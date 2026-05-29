use std::fs;
use std::sync::{Arc, Mutex};

use chrono::{Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle};
use tauri_plugin_dialog::DialogExt;
use timetrack_core::{
    merge_consecutive_activities,
    parser::terminal::{hook_install_script, hook_state_path, read_terminal_hook_state},
    parse_hh_mm, DayWorkHours,
};
use timetrack_monitor::{
    capture_snapshot, is_accessibility_trusted, open_accessibility_settings,
    request_accessibility_prompt,
};

use crate::export::{self, ExportFormat, ExportScope};
use crate::state::AppState;

#[derive(Serialize)]
pub struct ActivityDto {
    pub started_at: String,
    pub ended_at: Option<String>,
    pub app_name: String,
    pub subtitle: String,
    pub url: Option<String>,
    pub is_idle: bool,
}

#[derive(Serialize)]
pub struct DayWorkHoursDto {
    pub enabled: bool,
    pub start: String,
    pub end: String,
}

#[derive(Serialize)]
pub struct TrackerStatus {
    pub accessibility_granted: bool,
    pub tracking_paused: bool,
    pub work_hours_enabled: bool,
    pub work_hours_active: bool,
    pub work_hours_today_label: String,
    pub work_hours_week: Vec<DayWorkHoursDto>,
    pub total_today_label: String,
    pub app_binary_path: String,
    pub tracking_error: Option<String>,
    pub update_available: Option<String>,
}

fn day_work_hours_dto(day: &DayWorkHours) -> DayWorkHoursDto {
    DayWorkHoursDto {
        enabled: day.enabled,
        start: format_minutes(day.start_minutes),
        end: format_minutes(day.end_minutes),
    }
}

fn format_minutes(total: u16) -> String {
    format!("{:02}:{:02}", total / 60, total % 60)
}

fn build_tracker_status(
    guard: &AppState,
    day: NaiveDate,
    now: chrono::DateTime<Utc>,
) -> Result<TrackerStatus, String> {
    let total = guard
        .db
        .total_duration_for_day(day, now)
        .map_err(|e| e.to_string())?;

    Ok(TrackerStatus {
        accessibility_granted: accessibility_effective(),
        tracking_paused: guard.settings.tracking_paused,
        work_hours_enabled: guard.settings.work_hours.enabled,
        work_hours_active: guard.settings.work_hours.is_active_now(),
        work_hours_today_label: guard.settings.work_hours.today_label(),
        work_hours_week: guard
            .settings
            .work_hours
            .days
            .iter()
            .map(day_work_hours_dto)
            .collect(),
        total_today_label: format_duration(total),
        app_binary_path: current_binary_path(),
        tracking_error: None,
        update_available: guard.pending_update_version.clone(),
    })
}

fn accessibility_effective() -> bool {
    if is_accessibility_trusted() {
        return true;
    }

    match timetrack_monitor::capture_snapshot() {
        Ok(snapshot) => {
            !snapshot.window_title.is_empty()
                || snapshot.url.is_some()
                || snapshot.page_title.is_some()
        }
        Err(_) => false,
    }
}

fn current_binary_path() -> String {
    std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_default()
}

fn format_duration(secs: i64) -> String {
    if secs < 60 {
        return format!("{secs}s");
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins} Min");
    }
    let hours = mins / 60;
    let rem = mins % 60;
    if rem == 0 {
        format!("{hours}h")
    } else {
        format!("{hours}h {rem}m")
    }
}

fn build_subtitle(activity: &timetrack_core::Activity) -> String {
    if activity.is_idle {
        return "Inaktiv".into();
    }

    if let Some(project) = &activity.context.project {
        let mut parts = vec![project.clone()];
        if let Some(file) = &activity.context.file {
            parts.push(file.clone());
        }
        return parts.join(" · ");
    }

    if let Some(branch) = &activity.context.git_branch {
        return format!("Branch: {branch}");
    }

    if let Some(page_title) = &activity.context.page_title {
        if !page_title.is_empty() {
            return page_title.clone();
        }
    }

    if let Some(url) = &activity.context.url {
        return url.clone();
    }

    if !activity.window_title.is_empty() {
        return activity.window_title.clone();
    }

    activity.app_name.clone()
}

#[tauri::command]
pub fn get_activities(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    day: Option<String>,
) -> Result<Vec<ActivityDto>, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    let day = parse_day(day)?;

    let activities = merge_consecutive_activities(
        guard
            .db
            .activities_for_day(day)
            .map_err(|e| e.to_string())?,
    );

    Ok(activities
        .into_iter()
        .map(|a| ActivityDto {
            started_at: a.started_at.to_rfc3339(),
            ended_at: a.ended_at.map(|t| t.to_rfc3339()),
            app_name: a.app_name.clone(),
            subtitle: build_subtitle(&a),
            url: a.context.url.clone(),
            is_idle: a.is_idle,
        })
        .collect())
}

#[tauri::command]
pub fn get_tracker_status(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    day: Option<String>,
) -> Result<TrackerStatus, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    let day = parse_day(day)?;
    build_tracker_status(&guard, day, Utc::now())
}

#[tauri::command]
pub fn request_accessibility() -> bool {
    request_accessibility_prompt()
}

#[tauri::command]
pub fn open_accessibility_settings_cmd() -> Result<(), String> {
    open_accessibility_settings();
    Ok(())
}

#[derive(Deserialize)]
pub struct SetWorkScheduleDay {
    pub enabled: bool,
    pub start: String,
    pub end: String,
}

#[tauri::command]
pub fn set_work_schedule(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    enabled: bool,
    days: Vec<SetWorkScheduleDay>,
) -> Result<TrackerStatus, String> {
    if days.len() != 7 {
        return Err("Es müssen genau 7 Wochentage angegeben werden.".into());
    }

    let mut schedule = std::array::from_fn(|_| DayWorkHours::default());
    for (index, day) in days.into_iter().enumerate() {
        let start_minutes = parse_hh_mm(&day.start).ok_or("Ungültige Startzeit (HH:MM)")?;
        let end_minutes = parse_hh_mm(&day.end).ok_or("Ungültige Endzeit (HH:MM)")?;
        schedule[index] = DayWorkHours {
            enabled: day.enabled,
            start_minutes,
            end_minutes,
        };
    }

    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.settings.work_hours.enabled = enabled;
    guard.settings.work_hours.days = schedule;
    guard.save_settings().map_err(|e| e.to_string())?;

    let day = Local::now().date_naive();
    build_tracker_status(&guard, day, Utc::now())
}

#[tauri::command]
pub fn set_tracking_paused(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    paused: bool,
) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.settings.tracking_paused = paused;
    guard.save_settings().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_all_data(state: tauri::State<'_, Arc<Mutex<AppState>>>) -> Result<u64, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    guard
        .db
        .delete_all()
        .map_err(|e| e.to_string())
        .map(|count| count as u64)
}

#[tauri::command]
pub fn delete_day_data(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    day: String,
) -> Result<u64, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    let day = parse_day(Some(day))?;
    let deleted = guard
        .db
        .delete_activities_for_day(day)
        .map_err(|e| e.to_string())?;
    Ok(deleted as u64)
}

#[tauri::command]
pub fn install_terminal_hook() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("home directory not found")?;
    let dir = home.join(".timetrack");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let hook_path = dir.join("hook.sh");
    fs::write(&hook_path, hook_install_script()).map_err(|e| e.to_string())?;

    Ok(format!(
        "Hook gespeichert unter {}.\n\n\
         Wichtig: In ~/.zshrc eintragen und Terminal neu starten:\n\n\
         source \"{}\"\n\n\
         Danach in Terminal einmal Enter drücken — die Datei \
         ~/.timetrack/terminal-state.jsonl sollte neue Zeilen bekommen.",
        hook_path.display(),
        hook_path.display()
    ))
}

#[derive(Serialize)]
pub struct TerminalHookStatus {
    pub hook_script_installed: bool,
    pub hook_script_path: String,
    pub shell_configured: bool,
    pub state_file_exists: bool,
    pub state_file_path: String,
    pub latest_cwd: Option<String>,
    pub latest_branch: Option<String>,
}

#[derive(Serialize)]
pub struct CapturePreview {
    pub accessibility_trusted: bool,
    pub frontmost_app: String,
    pub window_title: String,
    pub url: Option<String>,
}

fn shell_references_hook() -> bool {
    let Some(home) = dirs::home_dir() else {
        return false;
    };

    for name in [".zshrc", ".zprofile", ".bashrc", ".bash_profile"] {
        let path = home.join(name);
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        if content.contains("_timetrack_hook") || content.contains(".timetrack/hook.sh") {
            return true;
        }
    }

    false
}

#[tauri::command]
pub async fn export_activities(
    app: AppHandle,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    format: String,
    scope: String,
    day: Option<String>,
) -> Result<String, String> {
    let format = ExportFormat::try_from(format.as_str())?;
    let scope = ExportScope::try_from(scope.as_str())?;
    let export_day = if scope == ExportScope::Day {
        Some(parse_day(day)?)
    } else {
        None
    };

    let activities = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        match scope {
            ExportScope::Day => guard
                .db
                .activities_for_day(export_day.unwrap())
                .map_err(|e| e.to_string())?,
            ExportScope::All => guard.db.activities_all().map_err(|e| e.to_string())?,
        }
    };

    if activities.is_empty() {
        return Err("Keine Aktivitäten zum Exportieren.".into());
    }

    let now = Utc::now();
    let content = export::serialize_activities(&activities, format, now)?;
    let filename = export::default_filename(
        format,
        scope,
        export_day.unwrap_or_else(|| Local::now().date_naive()),
    );
    let (filter_name, extensions) = export::format_filter(format);
    let app_for_dialog = app.clone();

    let chosen_path = tauri::async_runtime::spawn_blocking(move || {
        app_for_dialog
            .dialog()
            .file()
            .set_title("Export speichern")
            .set_file_name(filename)
            .add_filter(filter_name, extensions)
            .blocking_save_file()
    })
    .await
    .map_err(|e| e.to_string())?;

    let Some(file_path) = chosen_path else {
        return Ok("Export abgebrochen.".into());
    };

    let path = file_path.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())?;

    Ok(format!(
        "{} Einträge exportiert nach {}.",
        activities.len(),
        path.display()
    ))
}

#[tauri::command]
pub fn get_terminal_hook_status() -> TerminalHookStatus {
    let home = dirs::home_dir().unwrap_or_default();
    let hook_script_path = home.join(".timetrack").join("hook.sh");
    let state_file_path = hook_state_path()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| home.join(".timetrack/terminal-state.jsonl").display().to_string());

    let hook = read_terminal_hook_state();

    TerminalHookStatus {
        hook_script_installed: hook_script_path.is_file(),
        hook_script_path: hook_script_path.display().to_string(),
        shell_configured: shell_references_hook(),
        state_file_exists: hook_state_path().is_some_and(|path| path.is_file()),
        state_file_path,
        latest_cwd: hook.as_ref().and_then(|ctx| ctx.cwd.clone()),
        latest_branch: hook.as_ref().and_then(|ctx| ctx.git_branch.clone()),
    }
}

#[tauri::command]
pub fn get_capture_preview() -> Result<CapturePreview, String> {
    let snapshot = capture_snapshot().map_err(|e| e.to_string())?;
    Ok(CapturePreview {
        accessibility_trusted: is_accessibility_trusted(),
        frontmost_app: snapshot.app_name,
        window_title: snapshot.window_title,
        url: snapshot.url,
    })
}

fn parse_day(day: Option<String>) -> Result<NaiveDate, String> {
    match day {
        Some(value) => NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|e| e.to_string()),
        None => Ok(Local::now().date_naive()),
    }
}

use std::fs;
use std::sync::{Arc, Mutex};

use chrono::{Local, NaiveDate, Utc};
use serde::Serialize;
use timetrack_core::{
    merge_consecutive_activities,
    parser::terminal::hook_install_script,
};
use timetrack_monitor::{
    is_accessibility_trusted, open_accessibility_settings, request_accessibility_prompt,
};

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
pub struct TrackerStatus {
    pub accessibility_granted: bool,
    pub tracking_paused: bool,
    pub total_today_label: String,
    pub app_binary_path: String,
}

fn accessibility_effective(db: &timetrack_core::Database) -> bool {
    if is_accessibility_trusted() {
        return true;
    }

    if db.has_rich_activity_context().unwrap_or(false) {
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
    let now = Utc::now();
    let total = guard
        .db
        .total_duration_for_day(day, now)
        .map_err(|e| e.to_string())?;

    Ok(TrackerStatus {
        accessibility_granted: accessibility_effective(&guard.db),
        tracking_paused: guard.settings.tracking_paused,
        total_today_label: format_duration(total),
        app_binary_path: current_binary_path(),
    })
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

#[tauri::command]
pub fn set_tracking_paused(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    paused: bool,
) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.settings.tracking_paused = paused;
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
        "Hook gespeichert unter {}. Füge diese Zeile in ~/.zshrc ein:\n\nsource \"{}\"",
        hook_path.display(),
        hook_path.display()
    ))
}

fn parse_day(day: Option<String>) -> Result<NaiveDate, String> {
    match day {
        Some(value) => NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|e| e.to_string()),
        None => Ok(Local::now().date_naive()),
    }
}

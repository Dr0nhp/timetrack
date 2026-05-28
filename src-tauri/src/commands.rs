use std::fs;
use std::sync::{Arc, Mutex};

use chrono::{Local, NaiveDate};
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
    pub id: i64,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_secs: i64,
    pub duration_label: String,
    pub app_name: String,
    pub window_title: String,
    pub url: Option<String>,
    pub page_title: Option<String>,
    pub project: Option<String>,
    pub file: Option<String>,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    pub is_idle: bool,
    pub subtitle: String,
}

#[derive(Serialize)]
pub struct TrackerStatus {
    pub accessibility_granted: bool,
    pub tracking_paused: bool,
    pub idle_timeout_secs: u64,
    pub total_today_secs: i64,
    pub total_today_label: String,
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
            id: a.id,
            started_at: a.started_at.to_rfc3339(),
            ended_at: a.ended_at.map(|t| t.to_rfc3339()),
            duration_secs: a.duration_secs,
            duration_label: format_duration(a.duration_secs),
            app_name: a.app_name.clone(),
            window_title: a.window_title.clone(),
            url: a.context.url.clone(),
            page_title: a.context.page_title.clone(),
            project: a.context.project.clone(),
            file: a.context.file.clone(),
            cwd: a.context.cwd.clone(),
            git_branch: a.context.git_branch.clone(),
            is_idle: a.is_idle,
            subtitle: build_subtitle(&a),
        })
        .collect())
}

#[tauri::command]
pub fn get_tracker_status(state: tauri::State<'_, Arc<Mutex<AppState>>>) -> Result<TrackerStatus, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    let today = Local::now().date_naive();
    let total = guard
        .db
        .total_duration_for_day(today)
        .map_err(|e| e.to_string())?;

    Ok(TrackerStatus {
        accessibility_granted: is_accessibility_trusted(),
        tracking_paused: guard.settings.tracking_paused,
        idle_timeout_secs: guard.settings.idle_timeout_secs,
        total_today_secs: total,
        total_today_label: format_duration(total),
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
pub fn delete_all_data(state: tauri::State<'_, Arc<Mutex<AppState>>>) -> Result<(), String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    guard.db.delete_all().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_terminal_hook_script() -> String {
    hook_install_script()
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

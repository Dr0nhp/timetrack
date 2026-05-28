mod commands;
mod state;
mod tracker;
mod update;

use std::sync::{Arc, Mutex};

use tauri::{
    menu::{Menu, MenuItem, MenuItemKind, HELP_SUBMENU_ID},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use tracing_subscriber::EnvFilter;

use crate::state::AppState;
use crate::tracker::TrackerHandle;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("timetrack=info".parse().unwrap()))
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let data_dir = dirs::data_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("timetrack");
            std::fs::create_dir_all(&data_dir).ok();

            let db_path = data_dir.join("timeline.db");
            let state = Arc::new(Mutex::new(AppState::new(db_path)?));
            let _tracker = TrackerHandle::start(Arc::clone(&state), app.handle().clone());

            app.manage(state);

            setup_tray(app)?;
            setup_app_menu(app)?;

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
            }

            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id.as_ref() == "check_updates" {
                let _ = app.emit("menu-check-updates", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_activities,
            commands::get_tracker_status,
            commands::request_accessibility,
            commands::open_accessibility_settings_cmd,
            commands::set_tracking_paused,
            commands::delete_all_data,
            commands::delete_day_data,
            update::check_for_updates,
            update::install_update,
            commands::install_terminal_hook,
        ])
        .run(tauri::generate_context!())
        .expect("error while running timetrack");
}

fn setup_app_menu(app: &tauri::App) -> tauri::Result<()> {
    let handle = app.handle();
    let menu = Menu::default(handle)?;
    let check_updates = MenuItem::with_id(
        handle,
        "check_updates",
        "Nach Updates suchen…",
        true,
        None::<&str>,
    )?;

    if let Some(MenuItemKind::Submenu(help_menu)) = menu.get(HELP_SUBMENU_ID) {
        help_menu.append(&check_updates)?;
        #[cfg(target_os = "macos")]
        help_menu.set_as_help_menu_for_nsapp()?;
    }

    app.set_menu(menu)?;
    Ok(())
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show_i = MenuItem::with_id(app, "show", "Timeline öffnen", true, None::<&str>)?;
    let pause_i = MenuItem::with_id(app, "pause", "Tracking pausieren", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Beenden", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &pause_i, &quit_i])?;

    let _tray = TrayIconBuilder::new()
        .icon(tauri::include_image!("icons/tray-icon.png"))
        .icon_as_template(true)
        .menu(&menu)
        .tooltip("TimeTrack")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "pause" => {
                if let Some(state) = app.try_state::<Arc<Mutex<AppState>>>() {
                    if let Ok(mut guard) = state.lock() {
                        guard.settings.tracking_paused = !guard.settings.tracking_paused;
                    }
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;
use tracing::{error, info};

#[derive(Serialize, Clone)]
pub struct UpdateProgress {
    pub phase: String,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub message: String,
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn emit_progress(app: &AppHandle, progress: UpdateProgress) {
    let _ = app.emit("update-progress", progress);
}

fn map_update_error(err: impl std::fmt::Display) -> String {
    let msg = err.to_string();
    if msg.contains("PermissionDenied") || msg.contains("administrator privileges") {
        format!(
            "Installation fehlgeschlagen (Rechte): {msg}\n\n\
             Ein macOS-Passwort-Dialog könnte im Hintergrund warten. \
             TimeTrack muss in /Applications installiert sein."
        )
    } else if msg.contains("signature") || msg.contains("Signature") {
        format!("Update-Signatur ungültig: {msg}")
    } else {
        msg
    }
}

#[cfg(target_os = "macos")]
fn is_installed_macos_app() -> bool {
    std::env::current_exe()
        .map(|path| path.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
fn is_installed_macos_app() -> bool {
    true
}

async fn show_dialog(app: AppHandle, title: String, message: String) {
    let _ = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .message(message)
            .title(title)
            .kind(MessageDialogKind::Info)
            .buttons(MessageDialogButtons::Ok)
            .blocking_show();
    })
    .await;
}

async fn confirm_dialog(app: AppHandle, title: String, message: String) -> bool {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .message(message)
            .title(title)
            .kind(MessageDialogKind::Info)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Installieren".into(),
                "Abbrechen".into(),
            ))
            .blocking_show()
    })
    .await
    .unwrap_or(false)
}

async fn show_error_dialog(app: AppHandle, message: String) {
    let _ = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .message(message)
            .title("Update fehlgeschlagen")
            .kind(MessageDialogKind::Error)
            .buttons(MessageDialogButtons::Ok)
            .blocking_show();
    })
    .await;
}

#[derive(Serialize)]
pub struct UpdateCheckResult {
    pub available: bool,
    pub current_version: String,
    pub version: Option<String>,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<UpdateCheckResult, String> {
    let current = app.package_info().version.to_string();

    let Some(update) = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(UpdateCheckResult {
            available: false,
            current_version: current,
            version: None,
            notes: None,
        });
    };

    Ok(UpdateCheckResult {
        available: true,
        current_version: current,
        version: Some(update.version),
        notes: update.body,
    })
}

pub async fn run_update_flow(app: AppHandle) {
    show_main_window(&app);

    let check = check_for_updates(app.clone()).await;
    if let Err(err) = check {
        show_error_dialog(app, err).await;
        return;
    }
    let result = check.unwrap();

    if !result.available {
        show_dialog(
            app,
            "Kein Update".into(),
            format!("TimeTrack {} ist aktuell.", result.current_version),
        )
        .await;
        return;
    }

    let version = result.version.unwrap_or_default();
    let notes = result
        .notes
        .map(|text| format!("\n\n{text}"))
        .unwrap_or_default();
    let confirmed = confirm_dialog(
        app.clone(),
        "Update verfügbar".into(),
        format!(
            "Update {version} ist verfügbar.{notes}\n\n\
             Jetzt herunterladen und installieren? Die App startet danach neu.\n\n\
             Hinweis: macOS kann danach einen Passwort-Dialog anzeigen."
        ),
    )
    .await;

    if !confirmed {
        return;
    }

    if !is_installed_macos_app() {
        show_dialog(
            app,
            "Dev-Modus".into(),
            "OTA-Updates funktionieren nur in der installierten TimeTrack.app \
             (z. B. in /Applications).\n\n\
             Mit `npm run tauri dev` kannst du die Update-Suche testen, \
             aber nicht installieren."
                .into(),
        )
        .await;
        return;
    }

    if let Err(err) = install_update(app.clone()).await {
        show_error_dialog(app, err).await;
    }
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    show_main_window(&app);
    emit_progress(
        &app,
        UpdateProgress {
            phase: "checking".into(),
            downloaded: 0,
            total: None,
            message: "Update wird vorbereitet…".into(),
        },
    );

    let Some(update) = app
        .updater()
        .map_err(map_update_error)?
        .check()
        .await
        .map_err(map_update_error)?
    else {
        return Err("Kein Update verfügbar".into());
    };

    info!(version = %update.version, "downloading update");
    emit_progress(
        &app,
        UpdateProgress {
            phase: "downloading".into(),
            downloaded: 0,
            total: None,
            message: format!("Lade Version {} herunter…", update.version),
        },
    );

    let app_for_progress = app.clone();
    let downloaded = AtomicU64::new(0);
    update
        .download_and_install(
            |chunk_len, total| {
                let downloaded_bytes =
                    downloaded.fetch_add(chunk_len as u64, Ordering::Relaxed) + chunk_len as u64;
                let pct = total.map(|t| {
                    if t > 0 {
                        ((downloaded_bytes as f64 / t as f64) * 100.0).round() as u32
                    } else {
                        0
                    }
                });
                let message = match pct {
                    Some(p) => format!("Download: {p}%"),
                    None => format!("Download: {} KB", downloaded_bytes / 1024),
                };
                emit_progress(
                    &app_for_progress,
                    UpdateProgress {
                        phase: "downloading".into(),
                        downloaded: downloaded_bytes,
                        total,
                        message,
                    },
                );
            },
            || {
                let downloaded_bytes = downloaded.load(Ordering::Relaxed);
                emit_progress(
                    &app_for_progress,
                    UpdateProgress {
                        phase: "installing".into(),
                        downloaded: downloaded_bytes,
                        total: None,
                        message: "Installiere Update… Falls ein macOS-Passwort-Dialog erscheint, \
                                  bitte bestätigen."
                            .into(),
                    },
                );
            },
        )
        .await
        .map_err(|e| {
            error!(error = %e, "update install failed");
            map_update_error(e)
        })?;

    info!("update installed, restarting app");
    emit_progress(
        &app,
        UpdateProgress {
            phase: "restarting".into(),
            downloaded: 0,
            total: None,
            message: "TimeTrack startet neu…".into(),
        },
    );

    app.restart();
}

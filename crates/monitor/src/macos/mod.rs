mod accessibility;
mod bundle_id;
mod workspace;

pub use accessibility::permissions;

use thiserror::Error;
use tracing;

use crate::snapshot::RawSnapshot;

#[derive(Debug, Error)]
pub enum MacMonitorError {
    #[error("no frontmost application")]
    NoFrontmostApp,
}

pub fn capture_snapshot() -> Result<RawSnapshot, MacMonitorError> {
    capture_snapshot_inner()
}

fn capture_snapshot_inner() -> Result<RawSnapshot, MacMonitorError> {
    let app = workspace::frontmost_app()?;
    let trusted = permissions::is_trusted();

    let mut window_title = String::new();
    let mut url = None;
    let mut page_title = None;

    if trusted {
        if let Some(window) = accessibility::focused_window_for_pid(app.pid) {
            window_title = window.title.clone().unwrap_or_default();
            page_title = window.title;
        }

        if !app.bundle_id.is_empty() {
            if let Some(browser) =
                accessibility::browser_info(&app.bundle_id, app.pid, &window_title)
            {
                url = browser.url;
                if browser.title.is_some() {
                    page_title = browser.title;
                }
            }
        }
    } else {
        tracing::debug!("accessibility not granted — window title and URL skipped");
    }

    Ok(RawSnapshot {
        app_name: app.name,
        app_bundle_id: app.bundle_id,
        window_title,
        url,
        page_title,
    })
}

pub fn idle_seconds() -> f64 {
    accessibility::idle::seconds_since_last_input()
}

mod accessibility;
mod workspace;

pub use accessibility::permissions;

use thiserror::Error;

use crate::snapshot::RawSnapshot;

#[derive(Debug, Error)]
pub enum MacMonitorError {
    #[error("no frontmost application")]
    NoFrontmostApp,
}

pub fn capture_snapshot() -> Result<RawSnapshot, MacMonitorError> {
    let app = workspace::frontmost_app()?;
    let trusted = permissions::is_trusted();

    let mut window_title = String::new();
    let mut url = None;
    let mut page_title = None;

    if trusted {
        if let Some(window) = accessibility::focused_window_for_pid(app.pid) {
            window_title = window.title.unwrap_or_default();
            page_title = window.title.clone();

            if let Some(browser) = accessibility::browser_info(&app.bundle_id, app.pid, &window_title) {
                url = browser.url;
                if browser.title.is_some() {
                    page_title = browser.title;
                }
            }
        }
    }

    Ok(RawSnapshot {
        app_name: app.name,
        app_bundle_id: app.bundle_id,
        window_title,
        url,
        page_title,
    })
}

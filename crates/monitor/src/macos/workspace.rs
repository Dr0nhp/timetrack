use core_foundation::array::CFArray;
use core_foundation::base::CFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::window::{
    kCGNullWindowID, kCGWindowLayer, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionOnScreenOnly, kCGWindowOwnerName, kCGWindowOwnerPID,
    CGWindowListCopyWindowInfo,
};
use objc2::rc::Retained;
use objc2_app_kit::{NSRunningApplication, NSWorkspace};
use objc2_foundation::{MainThreadMarker, NSString};

#[derive(Debug, Clone)]
pub struct FrontmostApp {
    pub name: String,
    pub bundle_id: String,
    pub pid: i32,
}

pub fn frontmost_app() -> Result<FrontmostApp, super::MacMonitorError> {
    if let Some(app) = frontmost_via_window_list() {
        return Ok(app);
    }

    frontmost_via_workspace().ok_or(super::MacMonitorError::NoFrontmostApp)
}

fn frontmost_via_window_list() -> Option<FrontmostApp> {
    unsafe {
        let windows_info = CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        );

        if windows_info.is_null() {
            return None;
        }

        let array: CFArray<CFDictionary<CFString, CFType>> =
            CFArray::wrap_under_create_rule(windows_info as _);

        let layer_key = CFString::wrap_under_get_rule(kCGWindowLayer);
        let pid_key = CFString::wrap_under_get_rule(kCGWindowOwnerPID);
        let name_key = CFString::wrap_under_get_rule(kCGWindowOwnerName);

        for window in array.iter() {
            let layer = dictionary_i64(window, &layer_key)?;
            if layer != 0 {
                continue;
            }

            let pid = dictionary_i32(window, &pid_key)?;
            if pid <= 0 {
                continue;
            }

            let owner_name = window
                .find(&name_key)
                .and_then(|v| v.downcast::<CFString>())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            if owner_name == "Window Server" || owner_name == "Dock" {
                continue;
            }

            return Some(FrontmostApp {
                name: owner_name,
                bundle_id: bundle_id_for_pid(pid).unwrap_or_default(),
                pid,
            });
        }

        None
    }
}

fn frontmost_via_workspace() -> Option<FrontmostApp> {
    let mtm = MainThreadMarker::new()?;
    let workspace = NSWorkspace::shared(mtm);
    let app: Retained<NSRunningApplication> = workspace.frontmostApplication()?;

    Some(FrontmostApp {
        name: app
            .localizedName()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Unknown".to_string()),
        bundle_id: app
            .bundleIdentifier()
            .map(|s| s.to_string())
            .unwrap_or_default(),
        pid: app.processIdentifier(),
    })
}

pub fn pid_for_bundle(bundle_id: &str) -> Option<i32> {
    let mtm = MainThreadMarker::new()?;
    let workspace = NSWorkspace::shared(mtm);
    let target = NSString::from_str(bundle_id);

    for app in workspace.runningApplications().iter() {
        if app.bundleIdentifier().as_ref() == Some(&target) {
            return Some(app.processIdentifier());
        }
    }

    None
}

fn bundle_id_for_pid(pid: i32) -> Option<String> {
    let mtm = MainThreadMarker::new()?;
    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
    app.bundleIdentifier().map(|s| s.to_string())
}

fn dictionary_i32(dict: &CFDictionary<CFString, CFType>, key: &CFString) -> Option<i32> {
    dict.find(key)
        .and_then(|v| v.downcast::<CFNumber>())
        .and_then(|n| n.to_i32())
}

fn dictionary_i64(dict: &CFDictionary<CFString, CFType>, key: &CFString) -> Option<i64> {
    dict.find(key)
        .and_then(|v| v.downcast::<CFNumber>())
        .and_then(|n| n.to_i64())
}

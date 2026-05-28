use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::window::{
    kCGNullWindowID, kCGWindowLayer, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionOnScreenOnly, kCGWindowOwnerName, kCGWindowOwnerPID,
    CGWindowListCopyWindowInfo,
};

use super::bundle_id;

#[derive(Debug, Clone)]
pub struct FrontmostApp {
    pub name: String,
    pub bundle_id: String,
    pub pid: i32,
}

/// Uses CGWindowList only — safe to call from the tracker background thread.
/// NSWorkspace must not be used off the main thread (can throw NSException → crash).
pub fn frontmost_app() -> Result<FrontmostApp, super::MacMonitorError> {
    frontmost_via_window_list().ok_or(super::MacMonitorError::NoFrontmostApp)
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
            let layer = dictionary_i64(&window, &layer_key)?;
            if layer != 0 {
                continue;
            }

            let pid = dictionary_i32(&window, &pid_key)?;
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
                name: owner_name.clone(),
                bundle_id: bundle_id::bundle_id_for_pid(pid, &owner_name),
                pid,
            });
        }

        None
    }
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

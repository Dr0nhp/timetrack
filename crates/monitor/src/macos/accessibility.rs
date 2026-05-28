use core_foundation::base::{CFGetTypeID, CFRelease, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringGetTypeID};
use core_foundation::url::{CFURL, CFURLGetTypeID};
use objc2::exception::catch as catch_objc_exception;
use std::panic::AssertUnwindSafe;

use accessibility_sys::{
    kAXErrorSuccess, kAXFocusedWindowAttribute, kAXTitleAttribute, kAXURLAttribute,
    AXUIElementCopyAttributeValue, AXUIElementCreateApplication, AXUIElementRef,
};

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BrowserInfo {
    pub url: Option<String>,
    pub title: Option<String>,
}

const BROWSER_BUNDLE_IDS: &[&str] = &[
    "com.google.Chrome",
    "com.apple.Safari",
    "org.mozilla.firefox",
    "com.brave.Browser",
    "com.microsoft.edgemac",
    "company.thebrowser.Browser",
    "com.operasoftware.Opera",
    "com.vivaldi.Vivaldi",
    "com.google.Chrome.canary",
];

const MAX_AX_DEPTH: usize = 8;

fn catch_ax<F, R>(label: &str, f: F) -> Option<R>
where
    F: FnOnce() -> Option<R>,
{
    match catch_objc_exception(AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(_) => {
            tracing::debug!("AX objc exception in {label}");
            None
        }
    }
}

/// Copy an AX attribute; `attribute` CFString must stay alive for the duration of the call.
unsafe fn copy_ax_attribute(element: AXUIElementRef, attribute: &str) -> Option<CFTypeRef> {
    let attr = CFString::new(attribute);
    let mut value: CFTypeRef = std::ptr::null_mut();
    if AXUIElementCopyAttributeValue(element, attr.as_concrete_TypeRef(), &mut value)
        != kAXErrorSuccess
        || value.is_null()
    {
        return None;
    }
    Some(value)
}

pub fn focused_window_for_pid(pid: i32) -> Option<WindowInfo> {
    catch_ax("focused_window", || focused_window_for_pid_inner(pid))
}

fn focused_window_for_pid_inner(pid: i32) -> Option<WindowInfo> {
    unsafe {
        let app_element = AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            return None;
        }

        let window = match copy_ax_attribute(app_element, kAXFocusedWindowAttribute) {
            Some(w) => w,
            None => {
                CFRelease(app_element as _);
                return None;
            }
        };

        let title = copy_string_attr(window as AXUIElementRef, kAXTitleAttribute);
        CFRelease(window as _);
        CFRelease(app_element as _);

        Some(WindowInfo { title })
    }
}

pub fn browser_info(bundle_id: &str, pid: i32, window_title: &str) -> Option<BrowserInfo> {
    if !BROWSER_BUNDLE_IDS.contains(&bundle_id) {
        return None;
    }

    catch_ax("browser_info", || {
        let url = if bundle_id == "com.apple.Safari" {
            safari_url(pid)
        } else {
            chromium_url(pid)
        }?;

        Some(BrowserInfo {
            url: Some(url),
            title: Some(window_title.to_string()),
        })
    })
}

fn focused_window_element_for_pid(pid: i32) -> Option<AXUIElementRef> {
    unsafe {
        let app_element = AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            return None;
        }

        let window = copy_ax_attribute(app_element, kAXFocusedWindowAttribute)?;
        CFRelease(app_element as _);
        Some(window as AXUIElementRef)
    }
}

fn safari_url(pid: i32) -> Option<String> {
    let window = focused_window_element_for_pid(pid)?;
    unsafe {
        // Safari exposes kAXURLAttribute as CFURL, not CFString.
        let url = copy_url_attr(window)
            .or_else(|| find_ax_url(window, MAX_AX_DEPTH));
        CFRelease(window as _);
        url
    }
}

fn chromium_url(pid: i32) -> Option<String> {
    let window = focused_window_element_for_pid(pid)?;
    unsafe {
        let url = copy_url_attr(window)
            .or_else(|| copy_string_attr(window, kAXURLAttribute))
            .filter(|value| !value.is_empty())
            .or_else(|| find_address_field_value(window, MAX_AX_DEPTH));
        CFRelease(window as _);
        url
    }
}

unsafe fn copy_url_attr(element: AXUIElementRef) -> Option<String> {
    let value = copy_ax_attribute(element, kAXURLAttribute)?;
    let result = cf_type_to_url_string(value);
    CFRelease(value as _);
    result.filter(|url| !url.is_empty())
}

unsafe fn copy_string_attr(element: AXUIElementRef, attr: &str) -> Option<String> {
    let value = copy_ax_attribute(element, attr)?;
    let result = cf_type_to_string(value);
    CFRelease(value as _);
    result.filter(|text| !text.is_empty())
}

unsafe fn cf_type_to_string(value: CFTypeRef) -> Option<String> {
    if CFGetTypeID(value) != CFStringGetTypeID() {
        return cf_type_to_url_string(value);
    }
    let cf_string = CFString::wrap_under_get_rule(value as _);
    Some(cf_string.to_string())
}

unsafe fn cf_type_to_url_string(value: CFTypeRef) -> Option<String> {
    if CFGetTypeID(value) == CFURLGetTypeID() {
        let cf_url = CFURL::wrap_under_get_rule(value as _);
        return Some(cf_url.get_string().to_string());
    }
    if CFGetTypeID(value) == CFStringGetTypeID() {
        let cf_string = CFString::wrap_under_get_rule(value as _);
        return Some(cf_string.to_string());
    }
    None
}

unsafe fn find_ax_url(element: AXUIElementRef, depth: usize) -> Option<String> {
    if depth == 0 {
        return None;
    }

    if let Some(url) = copy_url_attr(element).or_else(|| copy_string_attr(element, kAXURLAttribute)) {
        return Some(url);
    }

    let children = copy_ax_attribute(element, accessibility_sys::kAXChildrenAttribute)?;
    let array = core_foundation::array::CFArray::<*const std::ffi::c_void>::wrap_under_get_rule(
        children as _,
    );

    for child in array.iter() {
        if let Some(url) = find_ax_url(*child as AXUIElementRef, depth - 1) {
            CFRelease(children as _);
            return Some(url);
        }
    }

    CFRelease(children as _);
    None
}

unsafe fn find_address_field_value(element: AXUIElementRef, depth: usize) -> Option<String> {
    if depth == 0 {
        return None;
    }

    if let Some(role) = copy_ax_attribute(element, accessibility_sys::kAXRoleAttribute) {
        let role_str = cf_type_to_string(role);
        CFRelease(role as _);

        if role_str.as_deref() == Some("AXTextField") {
            if let Some(value) = copy_string_attr(element, accessibility_sys::kAXValueAttribute) {
                if looks_like_url(&value) {
                    return Some(value);
                }
            }
        }
    }

    let children = copy_ax_attribute(element, accessibility_sys::kAXChildrenAttribute)?;
    let array = core_foundation::array::CFArray::<*const std::ffi::c_void>::wrap_under_get_rule(
        children as _,
    );

    for child in array.iter() {
        if let Some(url) = find_address_field_value(*child as AXUIElementRef, depth - 1) {
            CFRelease(children as _);
            return Some(url);
        }
    }

    CFRelease(children as _);
    None
}

fn looks_like_url(value: &str) -> bool {
    value.starts_with("http://")
        || value.starts_with("https://")
        || (value.contains('.') && !value.contains(' '))
}

pub mod idle {
    use core_graphics::event_source::CGEventSourceStateID;

    /// kCGAnyInputEventType — matches any user input event.
    const ANY_INPUT_EVENT_TYPE: u32 = 0xFFFF_FFFF;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(
            stateID: CGEventSourceStateID,
            eventType: u32,
        ) -> f64;
    }

    pub fn seconds_since_last_input() -> f64 {
        unsafe {
            CGEventSourceSecondsSinceLastEventType(
                CGEventSourceStateID::CombinedSessionState,
                ANY_INPUT_EVENT_TYPE,
            )
        }
    }
}

pub mod permissions {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use std::process::Command;

    pub fn is_trusted() -> bool {
        unsafe { accessibility_sys::AXIsProcessTrusted() }
    }

    pub fn request_prompt() -> bool {
        unsafe {
            let key = CFString::new("AXTrustedCheckOptionPrompt");
            let value = CFBoolean::true_value();
            let options =
                CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);
            accessibility_sys::AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef())
        }
    }

    pub fn open_settings() {
        let _ = Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn();
    }
}

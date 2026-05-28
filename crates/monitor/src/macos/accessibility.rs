use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::event::CGEventType;
use core_graphics::event_source::{CGEventSourceSecondsSinceLastEventType, CGEventSourceStateID};

use accessibility_sys::{
    kAXErrorSuccess, kAXFocusedWindowAttribute, kAXTitleAttribute, kAXURLAttribute,
    AXUIElementCopyAttributeValue, AXUIElementCreateApplication,
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

pub fn focused_window_for_pid(pid: i32) -> Option<WindowInfo> {
    unsafe {
        let app_element = AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            return None;
        }

        let mut window: CFTypeRef = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            app_element,
            kAXFocusedWindowAttribute,
            &mut window,
        );

        if err != kAXErrorSuccess || window.is_null() {
            CFRelease(app_element as _);
            return None;
        }

        let title = copy_string_attr(window, kAXTitleAttribute);
        CFRelease(window as _);
        CFRelease(app_element as _);

        Some(WindowInfo { title })
    }
}

pub fn browser_info(bundle_id: &str, pid: i32, window_title: &str) -> Option<BrowserInfo> {
    if !BROWSER_BUNDLE_IDS.contains(&bundle_id) {
        return None;
    }

    if bundle_id == "com.apple.Safari" {
        return safari_url(pid).map(|url| BrowserInfo {
            url: Some(url),
            title: Some(window_title.to_string()),
        });
    }

    chromium_url(pid).map(|url| BrowserInfo {
        url: Some(url),
        title: Some(window_title.to_string()),
    })
}

fn safari_url(pid: i32) -> Option<String> {
    unsafe {
        let app_element = AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            return None;
        }

        let mut window: CFTypeRef = std::ptr::null_mut();
        if AXUIElementCopyAttributeValue(app_element, kAXFocusedWindowAttribute, &mut window)
            != kAXErrorSuccess
            || window.is_null()
        {
            CFRelease(app_element as _);
            return None;
        }

        let url = find_ax_url(window);
        CFRelease(window as _);
        CFRelease(app_element as _);
        url
    }
}

fn chromium_url(pid: i32) -> Option<String> {
    unsafe {
        let app_element = AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            return None;
        }

        let mut window: CFTypeRef = std::ptr::null_mut();
        if AXUIElementCopyAttributeValue(app_element, kAXFocusedWindowAttribute, &mut window)
            != kAXErrorSuccess
            || window.is_null()
        {
            CFRelease(app_element as _);
            return None;
        }

        let url = find_address_field_value(window);
        CFRelease(window as _);
        CFRelease(app_element as _);
        url
    }
}

unsafe fn copy_string_attr(element: CFTypeRef, attr: CFStringRef) -> Option<String> {
    let mut value: CFTypeRef = std::ptr::null_mut();
    if AXUIElementCopyAttributeValue(element, attr, &mut value) != kAXErrorSuccess
        || value.is_null()
    {
        return None;
    }

    let result = cf_type_to_string(value);
    CFRelease(value as _);
    result
}

unsafe fn cf_type_to_string(value: CFTypeRef) -> Option<String> {
    let cf_string: CFString = CFString::wrap_under_get_rule(value as _);
    Some(cf_string.to_string())
}

unsafe fn find_ax_url(element: CFTypeRef) -> Option<String> {
    if let Some(url) = copy_string_attr(element, kAXURLAttribute) {
        if !url.is_empty() {
            return Some(url);
        }
    }

    let mut children: CFTypeRef = std::ptr::null_mut();
    if accessibility_sys::AXUIElementCopyAttributeValue(
        element,
        accessibility_sys::kAXChildrenAttribute,
        &mut children,
    ) != kAXErrorSuccess
        || children.is_null()
    {
        return None;
    }

    let array = core_foundation::array::CFArray::<*const std::ffi::c_void>::wrap_under_get_rule(
        children as _,
    );

    for child in array.iter() {
        if let Some(url) = find_ax_url(*child as CFTypeRef) {
            CFRelease(children as _);
            return Some(url);
        }
    }

    CFRelease(children as _);
    None
}

unsafe fn find_address_field_value(element: CFTypeRef) -> Option<String> {
    let mut role: CFTypeRef = std::ptr::null_mut();
    if accessibility_sys::AXUIElementCopyAttributeValue(
        element,
        accessibility_sys::kAXRoleAttribute,
        &mut role,
    ) == kAXErrorSuccess
        && !role.is_null()
    {
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

    let mut children: CFTypeRef = std::ptr::null_mut();
    if accessibility_sys::AXUIElementCopyAttributeValue(
        element,
        accessibility_sys::kAXChildrenAttribute,
        &mut children,
    ) != kAXErrorSuccess
        || children.is_null()
    {
        return None;
    }

    let array = core_foundation::array::CFArray::<*const std::ffi::c_void>::wrap_under_get_rule(
        children as _,
    );

    for child in array.iter() {
        if let Some(url) = find_address_field_value(*child as CFTypeRef) {
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
    use super::*;

    pub fn seconds_since_last_input() -> f64 {
        unsafe {
            CGEventSourceSecondsSinceLastEventType(
                CGEventSourceStateID::CombinedSessionState,
                CGEventType::from(0xFFFF_FFFF),
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

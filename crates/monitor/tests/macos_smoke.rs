//! Live macOS smoke tests — run on a real Mac with optional Accessibility.

#[cfg(target_os = "macos")]
#[test]
fn capture_frontmost_app() {
    let snap = timetrack_monitor::capture_snapshot().expect("snapshot");
    assert!(!snap.app_name.is_empty(), "app name should not be empty");
    assert!(snap.app_bundle_id.is_empty() || !snap.app_bundle_id.is_empty());
    eprintln!(
        "app={} bundle={} title={:?} url={:?}",
        snap.app_name, snap.app_bundle_id, snap.window_title, snap.url
    );
}

#[cfg(target_os = "macos")]
#[test]
fn accessibility_status_is_readable() {
    let trusted = timetrack_monitor::is_accessibility_trusted();
    eprintln!("accessibility_granted={trusted}");
}

#[cfg(target_os = "macos")]
#[test]
fn idle_seconds_is_non_negative() {
    let secs = timetrack_monitor::idle_seconds();
    assert!(secs >= 0.0);
    eprintln!("idle_seconds={secs:.1}");
}

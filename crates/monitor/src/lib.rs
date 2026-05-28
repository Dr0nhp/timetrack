mod snapshot;

#[cfg(target_os = "macos")]
mod macos;

pub use snapshot::RawSnapshot;

#[derive(Debug, thiserror::Error)]
pub enum MonitorError {
    #[error("platform monitor is only available on macOS")]
    UnsupportedPlatform,
    #[error("monitor error: {0}")]
    Inner(String),
}

pub fn capture_snapshot() -> Result<RawSnapshot, MonitorError> {
    #[cfg(target_os = "macos")]
    {
        macos::capture_snapshot().map_err(|e| MonitorError::Inner(e.to_string()))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = ();
        Err(MonitorError::UnsupportedPlatform)
    }
}

pub fn is_accessibility_trusted() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::permissions::is_trusted()
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn request_accessibility_prompt() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::permissions::request_prompt()
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn idle_seconds() -> f64 {
    #[cfg(target_os = "macos")]
    {
        macos::idle_seconds()
    }

    #[cfg(not(target_os = "macos"))]
    {
        0.0
    }
}

pub fn open_accessibility_settings() {
    #[cfg(target_os = "macos")]
    {
        macos::permissions::open_settings();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_snapshot_stub_has_empty_optional_fields() {
        let snap = RawSnapshot::stub("Zed");
        assert_eq!(snap.app_name, "Zed");
        assert!(snap.app_bundle_id.is_empty());
        assert!(snap.url.is_none());
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn capture_snapshot_unsupported_off_macos() {
        let err = capture_snapshot().unwrap_err();
        assert!(matches!(err, MonitorError::UnsupportedPlatform));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn accessibility_not_trusted_off_macos() {
        assert!(!is_accessibility_trusted());
        assert!(!request_accessibility_prompt());
        assert_eq!(idle_seconds(), 0.0);
    }
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityContext {
    pub url: Option<String>,
    pub page_title: Option<String>,
    pub project: Option<String>,
    pub file: Option<String>,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
}

impl Default for ActivityContext {
    fn default() -> Self {
        Self {
            url: None,
            page_title: None,
            project: None,
            file: None,
            cwd: None,
            git_branch: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivitySnapshot {
    pub app_name: String,
    pub app_bundle_id: String,
    pub window_title: String,
    pub context: ActivityContext,
    pub is_idle: bool,
}

impl ActivitySnapshot {
    pub fn idle() -> Self {
        Self {
            app_name: "Idle".into(),
            app_bundle_id: String::new(),
            window_title: String::new(),
            context: ActivityContext::default(),
            is_idle: true,
        }
    }

    pub fn signature(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.app_name,
            self.app_bundle_id,
            self.window_title,
            self.context.url.as_deref().unwrap_or(""),
            self.context.page_title.as_deref().unwrap_or(""),
            self.context.project.as_deref().unwrap_or(""),
            self.context.file.as_deref().unwrap_or(""),
            self.context.git_branch.as_deref().unwrap_or(""),
            self.is_idle
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Activity {
    pub id: i64,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_secs: i64,
    pub app_name: String,
    pub app_bundle_id: String,
    pub window_title: String,
    pub context: ActivityContext,
    pub is_idle: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackerSettings {
    pub idle_timeout_secs: u64,
    pub poll_interval_ms: u64,
    pub tracking_paused: bool,
}

impl Default for TrackerSettings {
    fn default() -> Self {
        Self {
            idle_timeout_secs: 300,
            poll_interval_ms: 1500,
            tracking_paused: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_snapshot_has_expected_defaults() {
        let idle = ActivitySnapshot::idle();
        assert!(idle.is_idle);
        assert_eq!(idle.app_name, "Idle");
        assert!(idle.app_bundle_id.is_empty());
    }

    #[test]
    fn signature_changes_when_url_changes() {
        let mut snap = ActivitySnapshot {
            app_name: "Chrome".into(),
            app_bundle_id: "com.google.Chrome".into(),
            window_title: "Tab".into(),
            context: ActivityContext::default(),
            is_idle: false,
        };

        let sig1 = snap.signature();
        snap.context.url = Some("https://a.com".into());
        let sig2 = snap.signature();
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn signature_ignores_cwd_for_deduplication() {
        let mut a = ActivitySnapshot {
            app_name: "Terminal".into(),
            app_bundle_id: "com.apple.Terminal".into(),
            window_title: "shell".into(),
            context: ActivityContext {
                cwd: Some("/a".into()),
                git_branch: Some("main".into()),
                ..Default::default()
            },
            is_idle: false,
        };

        let sig1 = a.signature();
        a.context.cwd = Some("/b".into());
        assert_eq!(sig1, a.signature());
    }

    #[test]
    fn tracker_settings_defaults() {
        let settings = TrackerSettings::default();
        assert_eq!(settings.idle_timeout_secs, 300);
        assert_eq!(settings.poll_interval_ms, 1500);
        assert!(!settings.tracking_paused);
    }
}

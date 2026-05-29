use chrono::{DateTime, Timelike, Utc};
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

impl Activity {
    /// Same key as [`ActivitySnapshot::signature`] — used to merge timeline rows.
    pub fn grouping_key(&self) -> String {
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

    pub fn duration_secs_at(&self, now: DateTime<Utc>) -> i64 {
        match self.ended_at {
            Some(_) => self.duration_secs,
            None => (now - self.started_at).num_seconds().max(0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkHoursSettings {
    pub enabled: bool,
    /// Minutes since midnight in local time, inclusive start.
    pub start_minutes: u16,
    /// Minutes since midnight in local time, exclusive end.
    pub end_minutes: u16,
}

impl Default for WorkHoursSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            start_minutes: 9 * 60,
            end_minutes: 18 * 60,
        }
    }
}

impl WorkHoursSettings {
    pub fn is_active_now(&self) -> bool {
        if !self.enabled {
            return true;
        }

        let now = chrono::Local::now().time();
        let minutes = now.hour() as u16 * 60 + now.minute() as u16;

        if self.start_minutes <= self.end_minutes {
            minutes >= self.start_minutes && minutes < self.end_minutes
        } else {
            minutes >= self.start_minutes || minutes < self.end_minutes
        }
    }

    pub fn start_label(&self) -> String {
        format_minutes(self.start_minutes)
    }

    pub fn end_label(&self) -> String {
        format_minutes(self.end_minutes)
    }
}

pub fn parse_hh_mm(value: &str) -> Option<u16> {
    let (hour, minute) = value.split_once(':')?;
    let hour: u16 = hour.parse().ok()?;
    let minute: u16 = minute.parse().ok()?;
    if hour >= 24 || minute >= 60 {
        return None;
    }
    Some(hour * 60 + minute)
}

fn format_minutes(total: u16) -> String {
    format!("{:02}:{:02}", total / 60, total % 60)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackerSettings {
    pub idle_timeout_secs: u64,
    pub poll_interval_ms: u64,
    pub tracking_paused: bool,
    pub work_hours: WorkHoursSettings,
}

impl Default for TrackerSettings {
    fn default() -> Self {
        Self {
            idle_timeout_secs: 300,
            poll_interval_ms: 1500,
            tracking_paused: false,
            work_hours: WorkHoursSettings::default(),
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

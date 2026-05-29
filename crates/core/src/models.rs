use chrono::{DateTime, Datelike, Timelike, Utc};
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DayWorkHours {
    pub enabled: bool,
    pub start_minutes: u16,
    pub end_minutes: u16,
}

impl Default for DayWorkHours {
    fn default() -> Self {
        Self {
            enabled: true,
            start_minutes: 9 * 60,
            end_minutes: 18 * 60,
        }
    }
}

impl DayWorkHours {
    pub fn is_active_at(&self, minutes: u16) -> bool {
        if !self.enabled {
            return false;
        }

        if self.start_minutes <= self.end_minutes {
            minutes >= self.start_minutes && minutes < self.end_minutes
        } else {
            minutes >= self.start_minutes || minutes < self.end_minutes
        }
    }

    pub fn schedule_label(&self) -> String {
        if !self.enabled {
            return "Frei".into();
        }
        format!(
            "{}–{}",
            format_minutes(self.start_minutes),
            format_minutes(self.end_minutes)
        )
    }
}

fn default_week_days() -> [DayWorkHours; 7] {
    std::array::from_fn(|index| DayWorkHours {
        enabled: index < 5,
        ..DayWorkHours::default()
    })
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkHoursSettings {
    pub enabled: bool,
    pub days: [DayWorkHours; 7],
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WorkHoursSettingsRaw {
    Weekly {
        enabled: bool,
        days: [DayWorkHours; 7],
    },
    Legacy {
        enabled: bool,
        start_minutes: u16,
        end_minutes: u16,
    },
}

impl<'de> Deserialize<'de> for WorkHoursSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        WorkHoursSettingsRaw::deserialize(deserializer).map(Into::into)
    }
}

impl From<WorkHoursSettingsRaw> for WorkHoursSettings {
    fn from(raw: WorkHoursSettingsRaw) -> Self {
        match raw {
            WorkHoursSettingsRaw::Weekly { enabled, days } => Self { enabled, days },
            WorkHoursSettingsRaw::Legacy {
                enabled,
                start_minutes,
                end_minutes,
            } => {
                let day = DayWorkHours {
                    enabled: true,
                    start_minutes,
                    end_minutes,
                };
                Self {
                    enabled,
                    days: std::array::from_fn(|_| day.clone()),
                }
            }
        }
    }
}

impl Default for WorkHoursSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            days: default_week_days(),
        }
    }
}

impl WorkHoursSettings {
    fn weekday_index(weekday: chrono::Weekday) -> usize {
        weekday.num_days_from_monday() as usize
    }

    pub fn today_schedule(&self) -> &DayWorkHours {
        &self.days[Self::weekday_index(chrono::Local::now().weekday())]
    }

    pub fn is_active_now(&self) -> bool {
        if !self.enabled {
            return true;
        }

        let now = chrono::Local::now();
        let day = &self.days[Self::weekday_index(now.weekday())];
        let minutes = now.time().hour() as u16 * 60 + now.time().minute() as u16;
        day.is_active_at(minutes)
    }

    pub fn today_label(&self) -> String {
        self.today_schedule().schedule_label()
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
        assert!(!settings.work_hours.days[5].enabled);
        assert!(settings.work_hours.days[0].enabled);
    }

    #[test]
    fn day_work_hours_respects_enabled_flag() {
        let day = DayWorkHours {
            enabled: false,
            start_minutes: 9 * 60,
            end_minutes: 18 * 60,
        };
        assert!(!day.is_active_at(10 * 60));
    }

    #[test]
    fn legacy_work_hours_deserialize_to_weekly() {
        let raw: WorkHoursSettings = serde_json::from_str(
            r#"{"enabled":true,"start_minutes":540,"end_minutes":990}"#,
        )
        .unwrap();
        assert!(raw.enabled);
        assert_eq!(raw.days[0].start_minutes, 540);
        assert_eq!(raw.days[6].end_minutes, 990);
    }
}

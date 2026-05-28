use chrono::{DateTime, Utc};

use crate::models::ActivitySnapshot;

pub struct SegmentTracker {
    current_id: Option<i64>,
    last_snapshot: Option<ActivitySnapshot>,
    last_change_at: Option<DateTime<Utc>>,
}

impl SegmentTracker {
    pub fn new() -> Self {
        Self {
            current_id: None,
            last_snapshot: None,
            last_change_at: None,
        }
    }

    pub fn current_segment_id(&self) -> Option<i64> {
        self.current_id
    }

    pub fn last_snapshot(&self) -> Option<&ActivitySnapshot> {
        self.last_snapshot.as_ref()
    }

    pub fn should_start_new_segment(&self, snapshot: &ActivitySnapshot) -> bool {
        match &self.last_snapshot {
            None => true,
            Some(prev) => prev.signature() != snapshot.signature(),
        }
    }

    pub fn on_segment_opened(&mut self, id: i64, snapshot: ActivitySnapshot, at: DateTime<Utc>) {
        self.current_id = Some(id);
        self.last_snapshot = Some(snapshot);
        self.last_change_at = Some(at);
    }

    pub fn on_segment_closed(&mut self, at: DateTime<Utc>) -> Option<i64> {
        let id = self.current_id.take();
        self.last_change_at = Some(at);
        id
    }

    pub fn tick_same_segment(&mut self, at: DateTime<Utc>) {
        self.last_change_at = Some(at);
    }
}

impl Default for SegmentTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ActivityContext, ActivitySnapshot};

    fn sample(app: &str, title: &str) -> ActivitySnapshot {
        ActivitySnapshot {
            app_name: app.into(),
            app_bundle_id: format!("com.{app}"),
            window_title: title.into(),
            context: ActivityContext::default(),
            is_idle: false,
        }
    }

    #[test]
    fn new_segment_on_app_change() {
        let tracker = SegmentTracker::new();
        let a = sample("Zed", "main.rs");
        let b = sample("Chrome", "GitHub");

        assert!(tracker.should_start_new_segment(&a));
        assert!(!tracker.should_start_new_segment(&a));

        let mut tracker = tracker;
        tracker.on_segment_opened(1, a, Utc::now());
        assert!(tracker.should_start_new_segment(&b));
    }

    #[test]
    fn new_segment_on_url_change() {
        let mut tracker = SegmentTracker::new();
        let mut ctx = ActivityContext::default();
        ctx.url = Some("https://a.com".into());

        let snap1 = ActivitySnapshot {
            app_name: "Chrome".into(),
            app_bundle_id: "com.google.Chrome".into(),
            window_title: "A".into(),
            context: ctx.clone(),
            is_idle: false,
        };

        ctx.url = Some("https://b.com".into());
        let snap2 = ActivitySnapshot {
            context: ctx,
            ..snap1.clone()
        };

        tracker.on_segment_opened(1, snap1, Utc::now());
        assert!(tracker.should_start_new_segment(&snap2));
    }

    #[test]
    fn idle_snapshot_differs() {
        let tracker = SegmentTracker::new();
        let active = sample("Zed", "main.rs");
        let idle = ActivitySnapshot::idle();

        assert!(tracker.should_start_new_segment(&active));
        let mut tracker = tracker;
        tracker.on_segment_opened(1, active, Utc::now());
        assert!(tracker.should_start_new_segment(&idle));
    }

    #[test]
    fn no_new_segment_for_identical_snapshot() {
        let mut tracker = SegmentTracker::new();
        let snap = sample("Zed", "main.rs — app");
        tracker.on_segment_opened(1, snap.clone(), Utc::now());
        assert!(!tracker.should_start_new_segment(&snap));
    }

    #[test]
    fn new_segment_when_project_changes() {
        let mut tracker = SegmentTracker::new();
        let mut ctx = ActivityContext::default();
        ctx.project = Some("a".into());

        let snap1 = ActivitySnapshot {
            app_name: "Zed".into(),
            app_bundle_id: "dev.zed.Zed".into(),
            window_title: "main.rs — a".into(),
            context: ctx.clone(),
            is_idle: false,
        };

        ctx.project = Some("b".into());
        let snap2 = ActivitySnapshot {
            context: ctx,
            window_title: "main.rs — b".into(),
            ..snap1.clone()
        };

        tracker.on_segment_opened(1, snap1, Utc::now());
        assert!(tracker.should_start_new_segment(&snap2));
    }

    #[test]
    fn close_segment_clears_current_id() {
        let mut tracker = SegmentTracker::new();
        tracker.on_segment_opened(42, sample("Zed", "a"), Utc::now());
        assert_eq!(tracker.current_segment_id(), Some(42));

        let closed = tracker.on_segment_closed(Utc::now());
        assert_eq!(closed, Some(42));
        assert_eq!(tracker.current_segment_id(), None);
    }
}

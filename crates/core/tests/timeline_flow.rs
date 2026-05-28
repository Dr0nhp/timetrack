//! Integration tests for the full tracking flow (snapshot → enrich → segment → db).

use chrono::{TimeZone, Utc};
use timetrack_core::{
    parser::enrich_snapshot, ActivityContext, ActivitySnapshot, Database, SegmentTracker,
};

fn chrome_tab(url: &str, title: &str) -> ActivitySnapshot {
    ActivitySnapshot {
        app_name: "Google Chrome".into(),
        app_bundle_id: "com.google.Chrome".into(),
        window_title: title.into(),
        context: ActivityContext {
            url: Some(url.into()),
            page_title: Some(title.into()),
            ..Default::default()
        },
        is_idle: false,
    }
}

#[test]
fn full_workday_simulation() {
    let db = Database::open_in_memory().unwrap();
    let mut tracker = SegmentTracker::new();
    let base = Utc.with_ymd_and_hms(2026, 5, 28, 9, 0, 0).unwrap();

    let timeline = [
        (
            base,
            enrich_snapshot(ActivitySnapshot {
                app_name: "Zed".into(),
                app_bundle_id: "dev.zed.Zed".into(),
                window_title: "main.rs — timetrack".into(),
                context: ActivityContext::default(),
                is_idle: false,
            }),
        ),
        (
            base + chrono::Duration::minutes(45),
            enrich_snapshot(chrome_tab(
                "https://github.com/user/timetrack",
                "Pull Request",
            )),
        ),
        (
            base + chrono::Duration::hours(1),
            ActivitySnapshot::idle(),
        ),
    ];

    for (at, snapshot) in timeline {
        if tracker.should_start_new_segment(&snapshot) {
            if let Some(open_id) = tracker.on_segment_closed(at) {
                db.close_segment(open_id, at).unwrap();
            }
            let id = db.insert_segment(&snapshot, at).unwrap();
            tracker.on_segment_opened(id, snapshot, at);
        } else {
            tracker.tick_same_segment(at);
        }
    }

    let end = base + chrono::Duration::hours(2);
    if let Some(open_id) = tracker.on_segment_closed(end) {
        db.close_segment(open_id, end).unwrap();
    }

    let activities = db.activities_for_day(base.date_naive()).unwrap();
    assert_eq!(activities.len(), 3);

    assert_eq!(activities[0].app_name, "Zed");
    assert_eq!(activities[0].context.project.as_deref(), Some("timetrack"));
    assert_eq!(activities[0].duration_secs, 45 * 60);

    assert_eq!(activities[1].app_name, "Google Chrome");
    assert_eq!(
        activities[1].context.url.as_deref(),
        Some("https://github.com/user/timetrack")
    );
    assert_eq!(activities[1].duration_secs, 15 * 60);

    assert!(activities[2].is_idle);
    assert_eq!(activities[2].duration_secs, 60 * 60);
}

#[test]
fn same_app_different_urls_create_separate_segments() {
    let db = Database::open_in_memory().unwrap();
    let mut tracker = SegmentTracker::new();
    let t0 = Utc.with_ymd_and_hms(2026, 5, 28, 10, 0, 0).unwrap();
    let t1 = t0 + chrono::Duration::minutes(10);

    for (at, url) in [(t0, "https://a.com"), (t1, "https://b.com")] {
        let snapshot = enrich_snapshot(chrome_tab(url, "Tab"));
        if tracker.should_start_new_segment(&snapshot) {
            if let Some(id) = tracker.on_segment_closed(at) {
                db.close_segment(id, at).unwrap();
            }
            let id = db.insert_segment(&snapshot, at).unwrap();
            tracker.on_segment_opened(id, snapshot, at);
        }
    }

    if let Some(id) = tracker.on_segment_closed(t1 + chrono::Duration::minutes(5)) {
        db.close_segment(id, t1 + chrono::Duration::minutes(5))
            .unwrap();
    }

    assert_eq!(
        db.activities_for_day(t0.date_naive()).unwrap().len(),
        2
    );
}

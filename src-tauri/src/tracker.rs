use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::Utc;
use timetrack_core::{
    parser::enrich_snapshot, ActivityContext, ActivitySnapshot, SegmentTracker,
};
use timetrack_monitor::{capture_snapshot, idle_seconds};
use tracing::{error, warn};
use tauri::{AppHandle, Emitter};

use crate::state::AppState;

pub struct TrackerHandle {
    _thread: thread::JoinHandle<()>,
}

impl TrackerHandle {
    pub fn start(state: Arc<Mutex<AppState>>, app: AppHandle) -> Self {
        let thread = thread::spawn(move || tracker_loop(state, app));
        Self { _thread: thread }
    }
}

fn tracker_loop(state: Arc<Mutex<AppState>>, app: AppHandle) {
    let mut segments = SegmentTracker::new();

    loop {
        let (poll_ms, idle_timeout, paused, track_now) = {
            let guard = match state.lock() {
                Ok(g) => g,
                Err(_) => {
                    warn!("app state lock poisoned");
                    thread::sleep(Duration::from_millis(1500));
                    continue;
                }
            };
            (
                guard.settings.poll_interval_ms,
                guard.settings.idle_timeout_secs,
                guard.settings.tracking_paused,
                guard.settings.work_hours.is_active_now(),
            )
        };

        if paused {
            thread::sleep(Duration::from_millis(poll_ms));
            continue;
        }

        if !track_now {
            if let Err(err) = close_open_segment(&state, &mut segments, &app) {
                warn!("failed to close segment outside work hours: {err}");
            }
            thread::sleep(Duration::from_millis(poll_ms));
            continue;
        }

        match tick(&state, &mut segments, idle_timeout, &app) {
            Ok(()) => {}
            Err(err) => warn!("tracker tick failed: {err}"),
        }

        thread::sleep(Duration::from_millis(poll_ms));
    }
}

fn close_open_segment(
    state: &Arc<Mutex<AppState>>,
    segments: &mut SegmentTracker,
    app: &AppHandle,
) -> Result<(), String> {
    let now = Utc::now();
    let guard = state.lock().map_err(|e| e.to_string())?;

    if let Some(open_id) = segments.on_segment_closed(now) {
        guard
            .db
            .close_segment(open_id, now)
            .map_err(|e| e.to_string())?;
        drop(guard);
        let _ = app.emit("timeline-changed", ());
    }

    Ok(())
}

fn tick(
    state: &Arc<Mutex<AppState>>,
    segments: &mut SegmentTracker,
    idle_timeout_secs: u64,
    app: &AppHandle,
) -> Result<(), String> {
    let now = Utc::now();
    let snapshot = build_snapshot(idle_timeout_secs);

    if !segments.should_start_new_segment(&snapshot) {
        segments.tick_same_segment(now);
        return Ok(());
    }

    let guard = state.lock().map_err(|e| e.to_string())?;

    if let Some(open_id) = segments.on_segment_closed(now) {
        guard
            .db
            .close_segment(open_id, now)
            .map_err(|e| e.to_string())?;
    }

    let id = guard
        .db
        .insert_segment(&snapshot, now)
        .map_err(|e| e.to_string())?;
    segments.on_segment_opened(id, snapshot, now);

    drop(guard);
    let _ = app.emit("timeline-changed", ());

    Ok(())
}

fn build_snapshot(idle_timeout_secs: u64) -> ActivitySnapshot {
    let idle_secs = idle_seconds();
    if idle_secs >= idle_timeout_secs as f64 {
        return ActivitySnapshot::idle();
    }

    let raw = match capture_snapshot() {
        Ok(raw) => raw,
        Err(err) => {
            error!("capture failed: {err}");
            return ActivitySnapshot {
                app_name: "Unknown".into(),
                app_bundle_id: String::new(),
                window_title: String::new(),
                context: ActivityContext::default(),
                is_idle: false,
            };
        }
    };

    let mut snapshot = ActivitySnapshot {
        app_name: raw.app_name,
        app_bundle_id: raw.app_bundle_id,
        window_title: raw.window_title,
        context: ActivityContext {
            url: raw.url,
            page_title: raw.page_title,
            ..ActivityContext::default()
        },
        is_idle: false,
    };

    snapshot = enrich_snapshot(snapshot);
    snapshot
}

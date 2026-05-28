use std::path::Path;

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use rusqlite::{params, Connection};
use thiserror::Error;

use crate::models::{Activity, ActivityContext, ActivitySnapshot};

#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(String),
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init_schema()?;
        db.close_open_segments()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<(), DbError> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS activities (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                started_at      TEXT NOT NULL,
                ended_at        TEXT,
                duration_secs   INTEGER NOT NULL DEFAULT 0,
                app_name        TEXT NOT NULL,
                app_bundle_id   TEXT NOT NULL,
                window_title    TEXT NOT NULL DEFAULT '',
                url             TEXT,
                page_title      TEXT,
                project         TEXT,
                file            TEXT,
                cwd             TEXT,
                git_branch      TEXT,
                is_idle         INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_activities_started_at ON activities(started_at);
            CREATE INDEX IF NOT EXISTS idx_activities_app ON activities(app_name);
            ",
        )?;
        Ok(())
    }

    fn close_open_segments(&self) -> Result<(), DbError> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "
            UPDATE activities
            SET ended_at = ?1,
                duration_secs = CAST(
                    (strftime('%s', ?1) - strftime('%s', started_at)) AS INTEGER
                )
            WHERE ended_at IS NULL
            ",
            params![now],
        )?;
        Ok(())
    }

    pub fn insert_segment(
        &self,
        snapshot: &ActivitySnapshot,
        started_at: DateTime<Utc>,
    ) -> Result<i64, DbError> {
        self.conn.execute(
            "
            INSERT INTO activities (
                started_at, app_name, app_bundle_id, window_title,
                url, page_title, project, file, cwd, git_branch, is_idle
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
            params![
                started_at.to_rfc3339(),
                snapshot.app_name,
                snapshot.app_bundle_id,
                snapshot.window_title,
                snapshot.context.url,
                snapshot.context.page_title,
                snapshot.context.project,
                snapshot.context.file,
                snapshot.context.cwd,
                snapshot.context.git_branch,
                snapshot.is_idle as i64,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn close_segment(&self, id: i64, ended_at: DateTime<Utc>) -> Result<(), DbError> {
        self.conn.execute(
            "
            UPDATE activities
            SET ended_at = ?1,
                duration_secs = CAST(
                    (strftime('%s', ?1) - strftime('%s', started_at)) AS INTEGER
                )
            WHERE id = ?2
            ",
            params![ended_at.to_rfc3339(), id],
        )?;
        Ok(())
    }

    pub fn activities_for_day(&self, day: NaiveDate) -> Result<Vec<Activity>, DbError> {
        let (start, end) = local_day_bounds_utc(day)?;

        let mut stmt = self.conn.prepare(
            "
            SELECT id, started_at, ended_at, duration_secs, app_name, app_bundle_id,
                   window_title, url, page_title, project, file, cwd, git_branch, is_idle
            FROM activities
            WHERE started_at >= ?1 AND started_at < ?2
            ORDER BY started_at ASC
            ",
        )?;

        let rows = stmt.query_map(params![start.to_rfc3339(), end.to_rfc3339()], |row| {
            Ok(Activity {
                id: row.get(0)?,
                started_at: parse_ts_sql(row.get::<_, String>(1)?)?,
                ended_at: row
                    .get::<_, Option<String>>(2)?
                    .map(parse_ts_sql)
                    .transpose()?,
                duration_secs: row.get(3)?,
                app_name: row.get(4)?,
                app_bundle_id: row.get(5)?,
                window_title: row.get(6)?,
                context: ActivityContext {
                    url: row.get(7)?,
                    page_title: row.get(8)?,
                    project: row.get(9)?,
                    file: row.get(10)?,
                    cwd: row.get(11)?,
                    git_branch: row.get(12)?,
                },
                is_idle: row.get::<_, i64>(13)? != 0,
            })
        })?;

        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn delete_all(&self) -> Result<usize, DbError> {
        Ok(self.conn.execute("DELETE FROM activities", [])?)
    }

    pub fn delete_activities_for_day(&self, day: NaiveDate) -> Result<usize, DbError> {
        let (start, end) = local_day_bounds_utc(day)?;
        let deleted = self.conn.execute(
            "DELETE FROM activities WHERE started_at >= ?1 AND started_at < ?2",
            params![start.to_rfc3339(), end.to_rfc3339()],
        )?;
        Ok(deleted)
    }

    pub fn total_duration_for_day(
        &self,
        day: NaiveDate,
        now: DateTime<Utc>,
    ) -> Result<i64, DbError> {
        Ok(self
            .activities_for_day(day)?
            .iter()
            .map(|activity| activity.duration_secs_at(now))
            .sum())
    }

    pub fn has_rich_activity_context(&self) -> Result<bool, DbError> {
        let mut stmt = self.conn.prepare(
            "
            SELECT 1
            FROM activities
            WHERE window_title != '' OR url IS NOT NULL OR page_title IS NOT NULL
            LIMIT 1
            ",
        )?;
        let mut rows = stmt.query([])?;
        Ok(rows.next()?.is_some())
    }
}

fn local_day_bounds_utc(day: NaiveDate) -> Result<(DateTime<Utc>, DateTime<Utc>), DbError> {
    let start = local_naive_to_utc(day.and_hms_opt(0, 0, 0).unwrap())?;
    let end = local_naive_to_utc(
        day.succ_opt()
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap(),
    )?;
    Ok((start, end))
}

fn local_naive_to_utc(dt: NaiveDateTime) -> Result<DateTime<Utc>, DbError> {
    Local
        .from_local_datetime(&dt)
        .single()
        .ok_or_else(|| DbError::InvalidTimestamp(format!("ambiguous local time: {dt}")))
        .map(|local| local.with_timezone(&Utc))
}

fn parse_ts(value: String) -> Result<DateTime<Utc>, DbError> {
    DateTime::parse_from_rfc3339(&value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| DbError::InvalidTimestamp(value))
}

fn parse_ts_sql(value: String) -> Result<DateTime<Utc>, rusqlite::Error> {
    parse_ts(value).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(e),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ActivitySnapshot;
    use chrono::TimeZone;

    fn snapshot(app: &str, bundle: &str) -> ActivitySnapshot {
        ActivitySnapshot {
            app_name: app.into(),
            app_bundle_id: bundle.into(),
            window_title: String::new(),
            context: ActivityContext::default(),
            is_idle: false,
        }
    }

    #[test]
    fn inserts_and_queries_day() {
        let db = Database::open_in_memory().unwrap();
        let snap = ActivitySnapshot {
            app_name: "Zed".into(),
            app_bundle_id: "dev.zed.Zed".into(),
            window_title: "main.rs — timetrack".into(),
            context: ActivityContext {
                project: Some("timetrack".into()),
                file: Some("main.rs".into()),
                ..Default::default()
            },
            is_idle: false,
        };

        let started = Utc::now();
        let id = db.insert_segment(&snap, started).unwrap();
        db.close_segment(id, started + chrono::Duration::seconds(60))
            .unwrap();

        let day = Local::now().date_naive();
        let activities = db.activities_for_day(day).unwrap();
        assert_eq!(activities.len(), 1);
        assert_eq!(activities[0].app_name, "Zed");
        assert_eq!(activities[0].duration_secs, 60);
    }

    #[test]
    fn stores_browser_and_idle_fields() {
        let db = Database::open_in_memory().unwrap();
        let started = Utc.with_ymd_and_hms(2026, 5, 28, 14, 0, 0).unwrap();
        let day = started.with_timezone(&Local).date_naive();

        let browser = ActivitySnapshot {
            app_name: "Chrome".into(),
            app_bundle_id: "com.google.Chrome".into(),
            window_title: "GitHub".into(),
            context: ActivityContext {
                url: Some("https://github.com".into()),
                page_title: Some("GitHub".into()),
                ..Default::default()
            },
            is_idle: false,
        };
        let id = db.insert_segment(&browser, started).unwrap();
        db.close_segment(id, started + chrono::Duration::seconds(30))
            .unwrap();

        let idle = ActivitySnapshot::idle();
        let idle_id = db
            .insert_segment(&idle, started + chrono::Duration::seconds(30))
            .unwrap();
        db.close_segment(idle_id, started + chrono::Duration::seconds(90))
            .unwrap();

        let activities = db.activities_for_day(day).unwrap();
        assert_eq!(activities.len(), 2);
        assert_eq!(activities[0].context.url.as_deref(), Some("https://github.com"));
        assert!(activities[1].is_idle);
    }

    #[test]
    fn filters_activities_by_local_day() {
        let db = Database::open_in_memory().unwrap();
        let day1 = NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();
        let day2 = NaiveDate::from_ymd_opt(2026, 5, 28).unwrap();
        let day1_ts = Local
            .from_local_datetime(&day1.and_hms_opt(23, 30, 0).unwrap())
            .single()
            .unwrap()
            .with_timezone(&Utc);
        let day2_ts = Local
            .from_local_datetime(&day2.and_hms_opt(8, 0, 0).unwrap())
            .single()
            .unwrap()
            .with_timezone(&Utc);

        let id1 = db.insert_segment(&snapshot("Slack", "com.slack"), day1_ts).unwrap();
        db.close_segment(id1, day1_ts + chrono::Duration::minutes(10))
            .unwrap();

        let id2 = db.insert_segment(&snapshot("Zed", "dev.zed.Zed"), day2_ts).unwrap();
        db.close_segment(id2, day2_ts + chrono::Duration::minutes(5))
            .unwrap();

        assert_eq!(db.activities_for_day(day1).unwrap().len(), 1);
        assert_eq!(db.activities_for_day(day2).unwrap().len(), 1);
    }

    #[test]
    fn early_morning_belongs_to_local_today_not_yesterday() {
        let db = Database::open_in_memory().unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 5, 28).unwrap();
        let yesterday = today.pred_opt().unwrap();
        let started = Local
            .from_local_datetime(&today.and_hms_opt(0, 15, 0).unwrap())
            .single()
            .unwrap()
            .with_timezone(&Utc);

        let id = db
            .insert_segment(&snapshot("Zed", "dev.zed.Zed"), started)
            .unwrap();
        db.close_segment(id, started + chrono::Duration::minutes(30))
            .unwrap();

        assert_eq!(db.activities_for_day(today).unwrap().len(), 1);
        assert!(db.activities_for_day(yesterday).unwrap().is_empty());
    }

    #[test]
    fn total_duration_sums_closed_segments() {
        let db = Database::open_in_memory().unwrap();
        let start = Utc.with_ymd_and_hms(2026, 5, 28, 9, 0, 0).unwrap();

        let id1 = db.insert_segment(&snapshot("Zed", "dev.zed.Zed"), start).unwrap();
        db.close_segment(id1, start + chrono::Duration::seconds(100))
            .unwrap();

        let id2 = db
            .insert_segment(
                &snapshot("Chrome", "com.google.Chrome"),
                start + chrono::Duration::seconds(100),
            )
            .unwrap();
        db.close_segment(id2, start + chrono::Duration::seconds(250))
            .unwrap();

        assert_eq!(
            db.total_duration_for_day(
                start.with_timezone(&Local).date_naive(),
                start + chrono::Duration::seconds(250)
            )
            .unwrap(),
            250
        );
    }

    #[test]
    fn delete_all_removes_activities() {
        let db = Database::open_in_memory().unwrap();
        let start = Utc::now();
        let id = db.insert_segment(&snapshot("Zed", "dev.zed.Zed"), start).unwrap();
        db.close_segment(id, start + chrono::Duration::seconds(10))
            .unwrap();

        db.delete_all().unwrap();
        assert!(db
            .activities_for_day(start.with_timezone(&Local).date_naive())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn delete_activities_for_day_only_removes_matching_day() {
        let db = Database::open_in_memory().unwrap();
        let day1 = NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();
        let day2 = NaiveDate::from_ymd_opt(2026, 5, 28).unwrap();
        let day1_ts = Local
            .from_local_datetime(&day1.and_hms_opt(10, 0, 0).unwrap())
            .single()
            .unwrap()
            .with_timezone(&Utc);
        let day2_ts = Local
            .from_local_datetime(&day2.and_hms_opt(10, 0, 0).unwrap())
            .single()
            .unwrap()
            .with_timezone(&Utc);

        let id1 = db.insert_segment(&snapshot("Slack", "com.slack"), day1_ts).unwrap();
        db.close_segment(id1, day1_ts + chrono::Duration::minutes(5))
            .unwrap();

        let id2 = db.insert_segment(&snapshot("Zed", "dev.zed.Zed"), day2_ts).unwrap();
        db.close_segment(id2, day2_ts + chrono::Duration::minutes(5))
            .unwrap();

        let deleted = db.delete_activities_for_day(day1).unwrap();
        assert_eq!(deleted, 1);
        assert!(db.activities_for_day(day1).unwrap().is_empty());
        assert_eq!(db.activities_for_day(day2).unwrap().len(), 1);
    }

    #[test]
    fn close_open_segments_on_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("timeline.db");
        let start = Utc.with_ymd_and_hms(2026, 5, 28, 12, 0, 0).unwrap();

        {
            let db = Database::open(&path).unwrap();
            db.insert_segment(&snapshot("Zed", "dev.zed.Zed"), start)
                .unwrap();
        }

        let db = Database::open(&path).unwrap();
        let activities = db
            .activities_for_day(start.with_timezone(&Local).date_naive())
            .unwrap();
        assert_eq!(activities.len(), 1);
        assert!(activities[0].ended_at.is_some());
        assert!(activities[0].duration_secs >= 0);
    }
}

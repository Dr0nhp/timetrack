pub mod db;
pub mod models;
pub mod parser;
pub mod segment;

pub use db::{Database, DbError};
pub use models::{
    Activity, ActivityContext, ActivitySnapshot, DayWorkHours, TrackerSettings, WorkHoursSettings,
};
pub use models::parse_hh_mm;
pub use segment::{merge_consecutive_activities, SegmentTracker};

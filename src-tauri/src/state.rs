use std::path::PathBuf;
use std::sync::Mutex;

use timetrack_core::{Database, TrackerSettings};

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("database error: {0}")]
    Database(#[from] timetrack_core::DbError),
}

pub struct AppState {
    pub db: Database,
    pub settings: TrackerSettings,
    pub db_path: PathBuf,
}

impl AppState {
    pub fn new(db_path: PathBuf) -> Result<Self, StateError> {
        let db = Database::open(&db_path)?;
        Ok(Self {
            db,
            settings: TrackerSettings::default(),
            db_path,
        })
    }
}

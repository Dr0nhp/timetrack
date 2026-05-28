use std::path::PathBuf;

use timetrack_core::{Database, TrackerSettings};

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("database error: {0}")]
    Database(#[from] timetrack_core::DbError),
}

pub struct AppState {
    pub db: Database,
    pub settings: TrackerSettings,
}

impl AppState {
    pub fn new(db_path: PathBuf) -> Result<Self, StateError> {
        tracing::info!("database opened at {}", db_path.display());
        let db = Database::open(&db_path)?;
        Ok(Self {
            db,
            settings: TrackerSettings::default(),
        })
    }
}

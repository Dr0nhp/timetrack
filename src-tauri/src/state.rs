use std::fs;
use std::path::{Path, PathBuf};

use timetrack_core::{Database, TrackerSettings};

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("database error: {0}")]
    Database(#[from] timetrack_core::DbError),
    #[error("settings error: {0}")]
    Settings(String),
}

pub struct AppState {
    pub db: Database,
    pub settings: TrackerSettings,
    pub pending_update_version: Option<String>,
    settings_path: PathBuf,
}

impl AppState {
    pub fn new(db_path: PathBuf) -> Result<Self, StateError> {
        tracing::info!("database opened at {}", db_path.display());
        let db = Database::open(&db_path)?;
        let settings_path = settings_path_for_db(&db_path);
        let settings = load_settings(&settings_path).unwrap_or_default();

        Ok(Self {
            db,
            settings,
            pending_update_version: None,
            settings_path,
        })
    }

    pub fn save_settings(&self) -> Result<(), StateError> {
        let json = serde_json::to_string_pretty(&self.settings)
            .map_err(|e| StateError::Settings(e.to_string()))?;
        fs::write(&self.settings_path, json).map_err(|e| StateError::Settings(e.to_string()))
    }
}

fn settings_path_for_db(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("settings.json")
}

fn load_settings(path: &Path) -> Result<TrackerSettings, StateError> {
    let raw = fs::read_to_string(path).map_err(|e| StateError::Settings(e.to_string()))?;
    serde_json::from_str(&raw).map_err(|e| StateError::Settings(e.to_string()))
}

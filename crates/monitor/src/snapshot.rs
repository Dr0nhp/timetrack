use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSnapshot {
    pub app_name: String,
    pub app_bundle_id: String,
    pub window_title: String,
    pub url: Option<String>,
    pub page_title: Option<String>,
}

impl RawSnapshot {
    pub fn stub(app_name: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
            app_bundle_id: String::new(),
            window_title: String::new(),
            url: None,
            page_title: None,
        }
    }
}

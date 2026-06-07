use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub active_profile: String,
    pub profiles: HashMap<String, String>,
    #[serde(default = "default_language")]
    pub active_language: String,
}

fn default_language() -> String {
    "vn".to_string()
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportResult {
    Success(PathBuf),
    AlreadyExists(PathBuf),
    SourceNotFound,
    Error(String),
}

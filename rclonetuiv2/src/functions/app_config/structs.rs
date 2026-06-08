use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub active_profile: String,
    pub profiles: HashMap<String, String>,
    #[serde(default = "default_language")]
    pub active_language: String,
    #[serde(default = "default_max_bandwidth")]
    pub max_bandwidth_bytes_per_sec: u64,
    #[serde(default = "default_retries")]
    pub retries: u32,
    #[serde(default = "default_cloud_timeout")]
    pub cloud_list_timeout_secs: u64,
    #[serde(default = "default_stats_refresh")]
    pub stats_refresh_ms: u64,
    #[serde(default = "default_services_scan")]
    pub services_scan_secs: u64,
    #[serde(default = "default_min_transfers")]
    pub min_transfers: u64,
    #[serde(default = "default_min_checkers")]
    pub min_checkers: u64,
    #[serde(default = "default_max_transfers")]
    pub max_transfers: u64,
    #[serde(default = "default_max_checkers")]
    pub max_checkers: u64,
    #[serde(default = "default_transfers_prior_fixed")]
    pub transfers_prior_fixed: Option<u64>,
    #[serde(default = "default_checkers_prior_fixed")]
    pub checkers_prior_fixed: Option<u64>,
}

fn default_language() -> String {
    "vn".to_string()
}

fn default_max_bandwidth() -> u64 {
    12_500_000
}

fn default_retries() -> u32 {
    3
}

fn default_cloud_timeout() -> u64 {
    15
}

fn default_stats_refresh() -> u64 {
    1500
}

fn default_services_scan() -> u64 {
    4
}

fn default_min_transfers() -> u64 {
    8
}

fn default_min_checkers() -> u64 {
    16
}

fn default_max_transfers() -> u64 {
    64
}

fn default_max_checkers() -> u64 {
    128
}

fn default_transfers_prior_fixed() -> Option<u64> {
    None
}

fn default_checkers_prior_fixed() -> Option<u64> {
    None
}


#[derive(Debug, Clone, PartialEq)]
pub enum ExportResult {
    Success(PathBuf),
    AlreadyExists(PathBuf),
    SourceNotFound,
    Error(String),
}

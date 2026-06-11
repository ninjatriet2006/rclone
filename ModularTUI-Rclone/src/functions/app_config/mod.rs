use std::collections::HashMap;

pub mod structs;
pub mod get_home_dir;
pub mod get_default_rclone_conf;
pub mod get_rclone_tui_conf;
pub mod load;
pub mod save;
pub mod export_profile;
pub mod config_parser;

pub use structs::{AppConfig, ExportResult};
pub use get_home_dir::get_home_dir;
pub use get_default_rclone_conf::get_default_rclone_conf;
pub use get_rclone_tui_conf::get_rclone_tui_conf;
pub use config_parser::{natural_cmp, reorder_ini_sections, save_sorted_remotes_to_ini};

impl Default for AppConfig {
    fn default() -> Self {
        let mut profiles = HashMap::new();
        let default_rclone_conf = get_default_rclone_conf();
        let rclone_tui_conf = get_rclone_tui_conf();

        profiles.insert("default".to_string(), default_rclone_conf);
        profiles.insert("rclone_tui".to_string(), rclone_tui_conf);

        AppConfig {
            active_profile: "default".to_string(),
            profiles,
            active_language: "vn".to_string(),
            max_bandwidth_bytes_per_sec: 12_500_000,
            retries: 3,
            cloud_list_timeout_secs: 15,
            stats_refresh_ms: 1500,
            services_scan_secs: 4,
            min_transfers: 8,
            min_checkers: 16,
            max_transfers: 64,
            max_checkers: 128,
            transfers_prior_fixed: None,
            checkers_prior_fixed: None,
            min_multiplier: 0.5,
            max_multiplier: 4.0,
        }
    }
}

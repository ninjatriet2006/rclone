use std::collections::HashMap;

pub mod structs;
pub mod get_home_dir;
pub mod get_default_rclone_conf;
pub mod get_rclone_tui_conf;
pub mod load;
pub mod save;
pub mod export_profile;

pub use structs::{AppConfig, ExportResult};
pub use get_home_dir::get_home_dir;
pub use get_default_rclone_conf::get_default_rclone_conf;
pub use get_rclone_tui_conf::get_rclone_tui_conf;

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
        }
    }
}

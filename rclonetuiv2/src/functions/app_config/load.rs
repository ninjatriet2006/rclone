use std::fs;
use std::path::PathBuf;
use crate::functions::app_config::structs::AppConfig;
use crate::functions::app_config::get_home_dir::get_home_dir;
use crate::functions::app_config::get_default_rclone_conf::get_default_rclone_conf;

impl AppConfig {
    pub fn config_dir() -> PathBuf {
        if cfg!(target_os = "windows") {
            if let Ok(appdata) = std::env::var("APPDATA") {
                PathBuf::from(appdata).join("rclone-tui")
            } else {
                PathBuf::from(get_home_dir()).join(".config").join("rclone-tui")
            }
        } else if cfg!(target_os = "macos") {
            PathBuf::from(get_home_dir())
                .join("Library")
                .join("Application Support")
                .join("rclone-tui")
        } else {
            let home = get_home_dir();
            PathBuf::from(home).join(".config").join("rclone-tui")
        }
    }

    pub fn config_file_path() -> PathBuf {
        Self::config_dir().join("app_config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_file_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                    return config;
                }
            }
        }

        let default_config = AppConfig::default();
        let _ = fs::create_dir_all(Self::config_dir());
        let _ = default_config.save();
        default_config
    }

    pub fn get_active_profile_path(&self) -> String {
        self.profiles
            .get(&self.active_profile)
            .cloned()
            .unwrap_or_else(|| get_default_rclone_conf())
    }
}

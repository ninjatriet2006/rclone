use std::fs;
use crate::functions::app_config::structs::AppConfig;

impl AppConfig {
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_file_path();
        let _ = fs::create_dir_all(Self::config_dir());
        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, content).map_err(|e| e.to_string())?;
        Ok(())
    }
}

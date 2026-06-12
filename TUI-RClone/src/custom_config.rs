use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use crate::app_config::{get_home_dir, AppConfig};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct TuiCustomConfig {
    pub default_local_dir: String,
    pub default_remote_dir: String,
    pub remote_export_default_file: String,
    pub profile_export_default_dir: String,
    pub db_file_path: String,
    pub features_cache_file_path: String,
    pub default_mount_dir: String,
}

impl Default for TuiCustomConfig {
    fn default() -> Self {
        let home = get_home_dir();
        let config_dir = AppConfig::config_dir().to_string_lossy().to_string();
        let user = std::env::var("USER").unwrap_or_else(|_| "bimatkeo".to_string());
        TuiCustomConfig {
            default_local_dir: home.clone(),
            default_remote_dir: "".to_string(),
            remote_export_default_file: Path::new(&home).join("Desktop").join("exported_remotes.conf").to_string_lossy().to_string(),
            profile_export_default_dir: Path::new(&home).join("Desktop").to_string_lossy().to_string(),
            db_file_path: Path::new(&config_dir).join("active_ops.db").to_string_lossy().to_string(),
            features_cache_file_path: Path::new(&config_dir).join("features_cache.json").to_string_lossy().to_string(),
            default_mount_dir: format!("/media/{}/DATA/Torrents", user),
        }
    }
}

impl TuiCustomConfig {
    pub fn config_file_path() -> PathBuf {
        AppConfig::config_dir().join("directory.yaml")
    }

    pub fn load() -> Self {
        let path = Self::config_file_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = serde_yaml::from_str::<TuiCustomConfig>(&content) {
                    return config;
                }
            }
        }
        let default_config = TuiCustomConfig::default();
        let _ = default_config.save_with_comments();
        default_config
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<(), String> {
        self.save_with_comments()
    }

    pub fn save_with_comments(&self) -> Result<(), String> {
        let path = Self::config_file_path();
        let _ = fs::create_dir_all(AppConfig::config_dir());
        let yaml_str = serde_yaml::to_string(self).map_err(|e| e.to_string())?;
        let with_comments = format!(
            "# Rclone TUI - Cấu hình thư mục và đường dẫn\n\
             # Hướng dẫn vai trò của từng tham số:\n\
             # - default_local_dir: Thư mục cục bộ mặc định hiển thị trong Explorer.\n\
             # - default_remote_dir: Thư mục con mặc định trên Cloud khi duyệt.\n\
             # - remote_export_default_file: Đường dẫn tệp cấu hình remote được xuất mặc định (Alt+X).\n\
             # - profile_export_default_dir: Thư mục mặc định dùng để xuất cấu hình Profiles.\n\
             # - db_file_path: Đường dẫn tệp cơ sở dữ liệu SQLite (active_ops.db).\n\
             # - features_cache_file_path: Đường dẫn tệp cache lưu tính năng remote (features_cache.json).\n\
             # - default_mount_dir: Thư mục mount cục bộ mặc định khi tạo mới dịch vụ mount (ví dụ: /media/user/DATA/Torrents).\n\n\
             {}",
            yaml_str
        );
        fs::write(path, with_comments).map_err(|e| e.to_string())?;
        Ok(())
    }
}

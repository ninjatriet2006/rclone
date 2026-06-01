use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn default_language() -> String {
    "vn".to_string()
}

pub fn get_home_dir() -> String {
    if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| "C:\\Users\\Default".to_string())
    } else {
        std::env::var("HOME").unwrap_or_else(|_| "/home/bimatkeo".to_string())
    }
}

pub fn get_default_rclone_conf() -> String {
    if cfg!(target_os = "windows") {
        if let Ok(appdata) = std::env::var("APPDATA") {
            Path::new(&appdata).join("rclone").join("rclone.conf").to_string_lossy().to_string()
        } else {
            Path::new(&get_home_dir()).join(".config").join("rclone").join("rclone.conf").to_string_lossy().to_string()
        }
    } else {
        let home = get_home_dir();
        format!("{}/.config/rclone/rclone.conf", home)
    }
}

pub fn get_rclone_tui_conf() -> String {
    if cfg!(target_os = "windows") {
        if let Ok(appdata) = std::env::var("APPDATA") {
            Path::new(&appdata).join("rclone-tui").join("rclone_tui.config").to_string_lossy().to_string()
        } else {
            Path::new(&get_home_dir()).join(".config").join("rclone-tui").join("rclone_tui.config").to_string_lossy().to_string()
        }
    } else if cfg!(target_os = "macos") {
        let home = get_home_dir();
        Path::new(&home)
            .join("Library")
            .join("Application Support")
            .join("rclone-tui")
            .join("rclone_tui.config")
            .to_string_lossy()
            .to_string()
    } else {
        let home = get_home_dir();
        format!("{}/.config/rclone-tui/rclone_tui.config", home)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub active_profile: String,
    pub profiles: HashMap<String, String>,
    #[serde(default = "default_language")]
    pub active_language: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportResult {
    Success(PathBuf),
    AlreadyExists(PathBuf),
    SourceNotFound,
    Error(String),
}

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

impl AppConfig {
    /// Lấy đường dẫn thư mục lưu trữ cấu hình của app: ~/.config/rclone-tui/ hoặc AppData\rclone-tui
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

    /// Lấy đường dẫn tệp app_config.json
    pub fn config_file_path() -> PathBuf {
        Self::config_dir().join("app_config.json")
    }

    /// Nạp cấu hình từ đĩa, nếu chưa có thì tạo mới cấu hình mặc định
    pub fn load() -> Self {
        let path = Self::config_file_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                    return config;
                }
            }
        }

        // Khởi tạo thư mục và ghi cấu hình mặc định nếu chưa có
        let default_config = AppConfig::default();
        let _ = fs::create_dir_all(Self::config_dir());
        let _ = default_config.save();
        default_config
    }

    /// Lưu cấu hình hiện tại xuống đĩa
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_file_path();
        let _ = fs::create_dir_all(Self::config_dir());
        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, content).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Lấy đường dẫn tệp config của profile đang kích hoạt
    pub fn get_active_profile_path(&self) -> String {
        self.profiles
            .get(&self.active_profile)
            .cloned()
            .unwrap_or_else(|| get_default_rclone_conf())
    }

    /// Xuất Profile cấu hình ra thư mục Downloads/Saved Profile
    pub fn export_profile(&self, profile_name: &str, force_overwrite: bool) -> ExportResult {
        let source_path_str = match self.profiles.get(profile_name) {
            Some(path) => path,
            None => return ExportResult::SourceNotFound,
        };

        let source_path = Path::new(source_path_str);
        if !source_path.exists() {
            return ExportResult::SourceNotFound;
        }

        let home = get_home_dir();
        let downloads_dir = PathBuf::from(home).join("Downloads").join("Saved Profile");

        // Tạo thư mục nếu chưa tồn tại (Giải quyết Bug 57, 91)
        if let Err(e) = fs::create_dir_all(&downloads_dir) {
            return ExportResult::Error(format!(
                "Không thể tạo thư mục Downloads/Saved Profile: {}",
                e
            ));
        }

        let dest_file = downloads_dir.join(format!("{}.conf", profile_name));

        // Kiểm tra trùng tên ghi đè (Giải quyết Bug 53)
        if dest_file.exists() && !force_overwrite {
            return ExportResult::AlreadyExists(dest_file);
        }

        // Sao chép tệp cấu hình
        if let Err(e) = fs::copy(source_path, &dest_file) {
            return ExportResult::Error(format!("Lỗi sao chép tệp: {}", e));
        }

        ExportResult::Success(dest_file)
    }
}

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

fn default_max_bandwidth() -> u64 {
    12_500_000 // 100 Mbps
}

fn default_retries() -> u32 {
    3
}

fn default_log_level() -> String {
    "NOTICE".to_string()
}

fn default_rclone_log_name() -> String {
    "rclone.log".to_string()
}

fn default_app_log_name() -> String {
    "app.log".to_string()
}

fn default_cloud_timeout() -> u64 {
    15
}

fn default_scan_concurrency() -> usize {
    8
}

fn default_stats_refresh() -> u64 {
    1500
}

fn default_services_scan() -> u64 {
    4
}

fn default_vfs_cache_mode() -> String {
    "writes".to_string()
}

fn default_dir_cache_time() -> String {
    "5m".to_string()
}

fn default_max_transfers() -> u64 {
    64
}

fn default_max_checkers() -> u64 {
    128
}

fn default_min_transfers() -> u64 {
    8
}

fn default_min_checkers() -> u64 {
    16
}

fn default_transfers_prior_fixed() -> Option<u64> {
    None
}

fn default_checkers_prior_fixed() -> Option<u64> {
    None
}

fn default_min_multiplier() -> f64 {
    0.5
}

fn default_max_multiplier() -> f64 {
    4.0
}

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
    #[serde(default = "default_log_level")]
    pub rclone_log_level: String,
    #[serde(default = "default_rclone_log_name")]
    pub rclone_log_name: String,
    #[serde(default = "default_app_log_name")]
    pub app_log_name: String,
    #[serde(default = "default_cloud_timeout")]
    pub cloud_list_timeout_secs: u64,
    #[serde(default = "default_scan_concurrency")]
    pub scan_concurrency: usize,
    #[serde(default = "default_stats_refresh")]
    pub stats_refresh_ms: u64,
    #[serde(default = "default_services_scan")]
    pub services_scan_secs: u64,
    #[serde(default = "default_vfs_cache_mode")]
    pub default_vfs_cache_mode: String,
    #[serde(default = "default_dir_cache_time")]
    pub default_dir_cache_time: String,
    #[serde(default = "default_min_transfers")]
    pub min_transfers: u64,
    #[serde(default = "default_min_checkers")]
    pub min_checkers: u64,
    #[serde(default = "default_max_transfers")]
    pub max_transfers: u64,
    #[serde(default = "default_max_checkers")]
    pub max_checkers: u64,
    #[serde(default = "default_transfers_prior_fixed", alias = "transfers_override")]
    pub transfers_prior_fixed: Option<u64>,
    #[serde(default = "default_checkers_prior_fixed", alias = "checkers_override")]
    pub checkers_prior_fixed: Option<u64>,
    #[serde(default = "default_min_multiplier")]
    pub min_multiplier: f64,
    #[serde(default = "default_max_multiplier")]
    pub max_multiplier: f64,
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
            max_bandwidth_bytes_per_sec: default_max_bandwidth(),
            retries: default_retries(),
            rclone_log_level: default_log_level(),
            rclone_log_name: default_rclone_log_name(),
            app_log_name: default_app_log_name(),
            cloud_list_timeout_secs: default_cloud_timeout(),
            scan_concurrency: default_scan_concurrency(),
            stats_refresh_ms: default_stats_refresh(),
            services_scan_secs: default_services_scan(),
            default_vfs_cache_mode: default_vfs_cache_mode(),
            default_dir_cache_time: default_dir_cache_time(),
            min_transfers: default_min_transfers(),
            min_checkers: default_min_checkers(),
            max_transfers: default_max_transfers(),
            max_checkers: default_max_checkers(),
            transfers_prior_fixed: default_transfers_prior_fixed(),
            checkers_prior_fixed: default_checkers_prior_fixed(),
            min_multiplier: default_min_multiplier(),
            max_multiplier: default_max_multiplier(),
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

    /// Lấy đường dẫn tệp app_config.yaml
    pub fn config_file_path() -> PathBuf {
        Self::config_dir().join("app_config.yaml")
    }

    /// Nạp cấu hình từ đĩa, nếu chưa có thì tạo mới cấu hình mặc định. Hỗ trợ tự động chuyển đổi từ json cũ.
    pub fn load() -> Self {
        let yaml_path = Self::config_file_path();
        let json_path = Self::config_dir().join("app_config.json");

        // Di chuyển cấu hình từ JSON cũ sang YAML mới nếu có
        if !yaml_path.exists() && json_path.exists() {
            if let Ok(content) = fs::read_to_string(&json_path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                    let _ = config.save();
                    let _ = fs::remove_file(json_path);
                    return config;
                }
            }
        }

        if yaml_path.exists() {
            if let Ok(content) = fs::read_to_string(&yaml_path) {
                if let Ok(config) = serde_yaml::from_str::<AppConfig>(&content) {
                    // Tự động lưu lại cấu hình để bổ sung các khoá thiếu và loại bỏ khoá thừa
                    let _ = config.save();
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

    /// Lưu cấu hình hiện tại xuống đĩa dưới dạng YAML kèm theo chú thích rõ ràng
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_file_path();
        let _ = fs::create_dir_all(Self::config_dir());
        
        let yaml_str = serde_yaml::to_string(self).map_err(|e| e.to_string())?;
        
        let mut commented_yaml = String::new();
        commented_yaml.push_str("# ==================================================================\n");
        commented_yaml.push_str("# TỆP CẤU HÌNH RCLONE TUI REMODEL (Định dạng YAML)\n");
        commented_yaml.push_str("# Bạn có thể chỉnh sửa các giá trị bên dưới trực tiếp bằng trình soạn thảo văn bản.\n");
        commented_yaml.push_str("# ==================================================================\n\n");

        for line in yaml_str.lines() {
            if line.starts_with("active_profile:") {
                commented_yaml.push_str("# Tên profile cấu hình đang hoạt động (ví dụ: default)\n");
            } else if line.starts_with("profiles:") {
                commented_yaml.push_str("\n# Danh sách các profile cấu hình và đường dẫn đến tệp rclone.conf tương ứng\n");
            } else if line.starts_with("active_language:") {
                commented_yaml.push_str("\n# Ngôn ngữ giao diện hoạt động: \"vn\" (Tiếng Việt) hoặc \"en\" (Tiếng Anh)\n");
            } else if line.starts_with("max_bandwidth_bytes_per_sec:") {
                commented_yaml.push_str("\n# Giới hạn băng thông tối đa (Bytes/giây), mặc định 12500000 (~100 Mbps)\n");
            } else if line.starts_with("retries:") {
                commented_yaml.push_str("\n# Số lần thử lại tối đa cho các tác vụ RPC khi gặp lỗi kết nối\n");
            } else if line.starts_with("rclone_log_level:") {
                commented_yaml.push_str("\n# Mức độ ghi nhật ký rclone (DEBUG, INFO, NOTICE, ERROR)\n");
            } else if line.starts_with("rclone_log_name:") {
                commented_yaml.push_str("\n# Tên tệp nhật ký ghi log rclone (lưu trong thư mục config này)\n");
            } else if line.starts_with("app_log_name:") {
                commented_yaml.push_str("\n# Tên tệp nhật ký ghi log hệ thống của ứng dụng (mặc định: app.log)\n");
            } else if line.starts_with("cloud_list_timeout_secs:") {
                commented_yaml.push_str("\n# Thời gian chờ tối đa (giây) khi tải danh sách tệp từ Cloud trước khi báo timeout\n");
            } else if line.starts_with("scan_concurrency:") {
                commented_yaml.push_str("\n# Số luồng quét kiểm tra quyền/so khớp file song song khi Copy/Move\n");
            } else if line.starts_with("stats_refresh_ms:") {
                commented_yaml.push_str("\n# Tần suất cập nhật thông số tiến trình Monitor (mili-giây), mặc định 1500 ms\n");
            } else if line.starts_with("services_scan_secs:") {
                commented_yaml.push_str("\n# Thời gian định kỳ quét các dịch vụ ngầm và điểm Mount (giây)\n");
            } else if line.starts_with("default_vfs_cache_mode:") {
                commented_yaml.push_str("\n# Chế độ cache VFS mặc định cho các ổ đĩa Mount: off, minimal, reads, writes, full\n");
            } else if line.starts_with("default_dir_cache_time:") {
                commented_yaml.push_str("\n# Thời gian lưu bộ nhớ đệm thư mục mặc định cho các ổ đĩa Mount (ví dụ: 5m, 1h)\n");
            } else if line.starts_with("min_transfers:") {
                commented_yaml.push_str("\n# Giới hạn số lượng tệp truyền tải đồng thời tối thiểu của bộ tối ưu hóa luồng (mặc định: 8)\n");
            } else if line.starts_with("min_checkers:") {
                commented_yaml.push_str("\n# Giới hạn số lượng tệp kiểm tra so khớp đồng thời tối thiểu của bộ tối ưu hóa luồng (mặc định: 16)\n");
            } else if line.starts_with("max_transfers:") {
                commented_yaml.push_str("\n# Giới hạn số lượng tệp truyền tải đồng thời tối đa của bộ tối ưu hóa luồng (mặc định: 64)\n");
            } else if line.starts_with("max_checkers:") {
                commented_yaml.push_str("\n# Giới hạn số lượng tệp kiểm tra so khớp đồng thời tối đa của bộ tối ưu hóa luồng (mặc định: 128)\n");
            } else if line.starts_with("transfers_prior_fixed:") {
                commented_yaml.push_str("\n# Ưu tiên số tệp truyền tải đồng thời cố định (Ví dụ: 8). Đặt null để tự động tối ưu hóa.\n");
            } else if line.starts_with("checkers_prior_fixed:") {
                commented_yaml.push_str("\n# Ưu tiên số tệp kiểm tra đồng thời cố định (Ví dụ: 16). Đặt null để tự động tối ưu hóa.\n");
            } else if line.starts_with("min_multiplier:") {
                commented_yaml.push_str("\n# Hệ số nhân giới hạn luồng động tối thiểu (mặc định: 0.5)\n");
            } else if line.starts_with("max_multiplier:") {
                commented_yaml.push_str("\n# Hệ số nhân giới hạn luồng động tối đa (mặc định: 4.0)\n");
            }
            commented_yaml.push_str(line);
            commented_yaml.push_str("\n");
        }

        fs::write(path, commented_yaml).map_err(|e| e.to_string())?;
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

pub fn log_info(msg: &str) {
    let log_name = if let Ok(content) = fs::read_to_string(AppConfig::config_file_path()) {
        if let Ok(config) = serde_yaml::from_str::<AppConfig>(&content) {
            config.app_log_name
        } else {
            "app.log".to_string()
        }
    } else {
        "app.log".to_string()
    };
    let log_path = AppConfig::config_dir().join(log_name);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(&log_path)
    {
        use std::io::Write;
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "[{}] {}", secs, msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_and_save_config() {
        let config = AppConfig::load();
        println!("Loaded and saved config: {:?}", config);
    }
}


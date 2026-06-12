use std::fs;
use std::path::Path;
use std::path::PathBuf;
use crate::functions::app_config::structs::{AppConfig, ExportResult};
use crate::functions::app_config::get_home_dir::get_home_dir;

impl AppConfig {
    pub fn export_profile(&self, profile_name: &str, force_overwrite: bool) -> ExportResult {
        let source_path_str = match self.profiles.get(profile_name) {
            Some(path) => path,
            None => return ExportResult::SourceNotFound,
        };

        let source_path = Path::new(source_path_str);
        if !source_path.exists() {
            return ExportResult::SourceNotFound;
        }

        let downloads_dir = PathBuf::from(crate::functions::app_config::TuiCustomConfig::load().profile_export_default_dir);

        if let Err(e) = fs::create_dir_all(&downloads_dir) {
            return ExportResult::Error(format!(
                "Không thể tạo thư mục Downloads/Saved Profile: {}",
                e
            ));
        }

        let dest_file = downloads_dir.join(format!("{}.conf", profile_name));

        if dest_file.exists() && !force_overwrite {
            return ExportResult::AlreadyExists(dest_file);
        }

        if let Err(e) = fs::copy(source_path, &dest_file) {
            return ExportResult::Error(format!("Lỗi sao chép tệp: {}", e));
        }

        ExportResult::Success(dest_file)
    }
}

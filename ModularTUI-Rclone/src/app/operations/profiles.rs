use crate::app::{App, AppEvent, Screen, DeleteTarget, ScanState, MultiScanState};
use crate::functions::*;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

impl App {
    pub(crate) fn load_profile_list(&mut self) {
        let mut list = Vec::new();
        for (name, path) in &self.config.profiles {
            list.push((name.clone(), path.clone()));
        }
        list.sort_by(|a, b| a.0.cmp(&b.0));
        self.profile_state.profiles = list;
    }

pub(crate) fn execute_import_profile(&mut self, name: String, src: String, import_type: usize) {
        let dest_path = AppConfig::config_dir().join(format!("{}.config", name));

        if import_type == 1 {
            // Local path copy
            if Path::new(&src).exists() {
                if fs::copy(&src, &dest_path).is_ok() {
                    self.config
                        .profiles
                        .insert(name, dest_path.to_string_lossy().to_string());
                    let _ = self.config.save();
                    self.load_profile_list();
                    self.profile_state.wizard = ImportWizardState::None;
                } else {
                    self.profile_state.error_message =
                        Some("Lỗi sao chép tệp cấu hình.".to_string());
                }
            } else {
                self.profile_state.error_message =
                    Some("Đường dẫn local không tồn tại.".to_string());
            }
        } else {
            // URL Download (giả lập tải xuống nhanh bằng wget/curl)
            let output = Command::new("curl")
                .args(["-o", &dest_path.to_string_lossy(), &src])
                .output();

            if output.is_ok() && output.unwrap().status.success() {
                self.config
                    .profiles
                    .insert(name, dest_path.to_string_lossy().to_string());
                let _ = self.config.save();
                self.load_profile_list();
                self.profile_state.wizard = ImportWizardState::None;
            } else {
                self.profile_state.error_message =
                    Some("Tải cấu hình từ URL thất bại.".to_string());
            }
        }
    }

}

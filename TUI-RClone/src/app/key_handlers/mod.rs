mod connection;
mod explorer;
mod services;
mod misc;

use crate::rclone;
use crossterm::event::{KeyEvent, KeyCode};
use std::process::Command;

use crate::app::{
    App, AppEvent, Screen, DeleteTarget
};

impl App {

    /// Xử lý phím bấm phân loại theo từng Screen
    pub(crate) async fn handle_key_event(
        &mut self,
        key: KeyEvent,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        if let Some(target) = self.delete_confirm.clone() {
            match key.code {
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.delete_confirm = None;
                }
                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.delete_confirm = None;
                    match target {
                        DeleteTarget::Connection(remote_name) => {
                            let param = serde_json::json!({"name": remote_name}).to_string();
                            let _ = rclone::rpc("config/delete", &param);
                            self.load_remotes(tx.clone()).await;
                        }
                        DeleteTarget::FileExplorer(item_name) => {
                            let pane = self.explorer_state.get_active_pane();
                            let is_dir = pane.items.iter()
                                .find(|item| item.name == item_name)
                                .map(|item| item.is_dir)
                                .unwrap_or(false);

                            let remote = pane.remote.clone();
                            let pane_path = pane.path.clone();
                            let pane_type = self.explorer_state.active_pane.clone();
                            let tx_op = tx.clone();

                            let target = if is_dir {
                                if remote.is_empty() {
                                    std::path::PathBuf::from(&pane_path)
                                        .join(&item_name)
                                        .to_string_lossy()
                                        .to_string()
                                } else {
                                    let clean_remote = remote.trim_end_matches(':');
                                    let clean_path = if pane_path.starts_with('/') {
                                        pane_path.clone()
                                    } else {
                                        format!("/{}", pane_path)
                                    };
                                    if clean_path.ends_with('/') {
                                        format!("{}:{}{}", clean_remote, clean_path, item_name)
                                    } else {
                                        format!("{}:{}/{}", clean_remote, clean_path, item_name)
                                    }
                                }
                            } else {
                                let fs = if remote.is_empty() {
                                    pane_path.clone()
                                } else {
                                    let clean_remote = remote.trim_end_matches(':');
                                    if pane_path.is_empty() {
                                        format!("{}:", clean_remote)
                                    } else {
                                        format!("{}:{}", clean_remote, pane_path)
                                    }
                                };
                                if fs.ends_with(':') {
                                    format!("{}{}", fs, item_name)
                                } else {
                                    format!("{}/{}", fs, item_name)
                                }
                            };

                            let op_id = format!("del_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                            let op = crate::app::ActiveOperation {
                                id: op_id.clone(),
                                action_type: "delete".to_string(),
                                src: target.clone(),
                                dest: String::new(),
                                items: Vec::new(),
                                is_dir,
                                use_checksum: false,
                                is_copy: false,
                                completed_items: Some(Vec::new()),
                                tasks: None,
                            };
                            crate::app::save_active_operation(&op);

                            let item_name_clone = item_name.clone();
                            tokio::spawn(async move {
                                let (op_name, method, param) = if is_dir {
                                    (
                                        "xóa thư mục (purge)".to_string(),
                                        "operations/purge".to_string(),
                                        serde_json::json!({
                                            "fs": target,
                                            "remote": "",
                                        })
                                    )
                                } else {
                                    let fs = if remote.is_empty() {
                                        pane_path
                                    } else {
                                        let clean_remote = remote.trim_end_matches(':');
                                        if pane_path.is_empty() {
                                            format!("{}:", clean_remote)
                                        } else {
                                            format!("{}:{}", clean_remote, pane_path)
                                        }
                                    };
                                    (
                                        "xóa tệp (deletefile)".to_string(),
                                        "operations/deletefile".to_string(),
                                        serde_json::json!({
                                            "fs": fs,
                                            "remote": item_name_clone,
                                        })
                                    )
                                };

                                let op_res = rclone::rpc_async(method, param.to_string()).await;
                                crate::app::remove_active_operation(&op_id);
                                let res = match op_res {
                                    Ok(r) if r.status == 200 => Ok(()),
                                    Ok(r) => Err(format!("Mã lỗi: {}", r.status)),
                                    Err(e) => Err(e),
                                };
                                let _ = tx_op.send(AppEvent::ExplorerOperationFinished {
                                    pane: pane_type,
                                    op_name,
                                    result: res,
                                });
                            });
                        }
                        DeleteTarget::FileExplorerMultiple(item_names) => {
                            let pane_type = self.explorer_state.active_pane.clone();
                            let pane = self.explorer_state.get_active_pane_mut();
                            pane.selected_names.clear();
                            pane.shift_anchor = None;
                            pane.shift_active = false;
                            pane.alt_anchor = None;
                            pane.alt_active = false;
                            let remote = pane.remote.clone();
                            let pane_path = pane.path.clone();
                            let tx_op = tx.clone();
                            
                            let items_with_type: Vec<(String, bool)> = item_names
                                .into_iter()
                                .map(|name| {
                                    let is_dir = pane.items.iter()
                                        .find(|item| item.name == name)
                                        .map(|item| item.is_dir)
                                        .unwrap_or(false);
                                    (name, is_dir)
                                })
                                .collect();

                            let mut paths = Vec::new();
                            for (item_name, is_dir) in &items_with_type {
                                let target = if *is_dir {
                                    if remote.is_empty() {
                                        std::path::PathBuf::from(&pane_path)
                                            .join(item_name)
                                            .to_string_lossy()
                                            .to_string()
                                    } else {
                                        let clean_remote = remote.trim_end_matches(':');
                                        let clean_path = if pane_path.starts_with('/') {
                                            pane_path.clone()
                                        } else {
                                            format!("/{}", pane_path)
                                        };
                                        if clean_path.ends_with('/') {
                                            format!("{}:{}{}", clean_remote, clean_path, item_name)
                                        } else {
                                            format!("{}:{}/{}", clean_remote, clean_path, item_name)
                                        }
                                    }
                                } else {
                                    let fs = if remote.is_empty() {
                                        pane_path.clone()
                                    } else {
                                        let clean_remote = remote.trim_end_matches(':');
                                        if pane_path.is_empty() {
                                            format!("{}:", clean_remote)
                                        } else {
                                            format!("{}:{}", clean_remote, pane_path)
                                        }
                                    };
                                    if fs.ends_with(':') {
                                        format!("{}{}", fs, item_name)
                                    } else {
                                        format!("{}/{}", fs, item_name)
                                    }
                                };
                                paths.push(target);
                            }

                            let op_id = format!("del_multi_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                            let op = crate::app::ActiveOperation {
                                id: op_id.clone(),
                                action_type: "delete_multiple".to_string(),
                                src: String::new(),
                                dest: String::new(),
                                items: paths,
                                is_dir: false,
                                use_checksum: false,
                                is_copy: false,
                                completed_items: Some(Vec::new()),
                                tasks: None,
                            };
                            crate::app::save_active_operation(&op);

                            tokio::spawn(async move {
                                let mut last_err = None;
                                for (item_name, is_dir) in items_with_type {
                                    let (method, param) = if is_dir {
                                        let target = if remote.is_empty() {
                                            std::path::PathBuf::from(&pane_path)
                                                .join(&item_name)
                                                .to_string_lossy()
                                                .to_string()
                                        } else {
                                            let clean_remote = remote.trim_end_matches(':');
                                            let clean_path = if pane_path.starts_with('/') {
                                                pane_path.clone()
                                            } else {
                                                format!("/{}", pane_path)
                                            };
                                            if clean_path.ends_with('/') {
                                                format!("{}:{}{}", clean_remote, clean_path, item_name)
                                            } else {
                                                format!("{}:{}/{}", clean_remote, clean_path, item_name)
                                            }
                                        };
                                        (
                                            "operations/purge".to_string(),
                                            serde_json::json!({
                                                "fs": target,
                                                "remote": "",
                                            })
                                        )
                                    } else {
                                        let fs = if remote.is_empty() {
                                            pane_path.clone()
                                        } else {
                                            let clean_remote = remote.trim_end_matches(':');
                                            if pane_path.is_empty() {
                                                format!("{}:", clean_remote)
                                            } else {
                                                format!("{}:{}", clean_remote, pane_path)
                                            }
                                        };
                                        (
                                            "operations/deletefile".to_string(),
                                            serde_json::json!({
                                                "fs": fs,
                                                "remote": item_name,
                                            })
                                        )
                                    };

                                    let op_res = rclone::rpc_async(method, param.to_string()).await;
                                    match op_res {
                                        Ok(r) if r.status == 200 => {}
                                        Ok(r) => last_err = Some(format!("Mã lỗi: {}", r.status)),
                                        Err(e) => last_err = Some(e),
                                    }
                                }
                                
                                crate::app::remove_active_operation(&op_id);
                                let res = match last_err {
                                    None => Ok(()),
                                    Some(e) => Err(e),
                                };
                                let _ = tx_op.send(AppEvent::ExplorerOperationFinished {
                                    pane: pane_type,
                                    op_name: "xóa nhiều mục".to_string(),
                                    result: res,
                                });
                            });
                        }
                        DeleteTarget::Service(idx) => {
                            if idx < self.services_state.active_services.len() {
                                let service = &self.services_state.active_services[idx];
                                // Diệt tiến trình
                                #[cfg(unix)]
                                {
                                    let _ = Command::new("kill").arg(service.pid.to_string()).status();
                                    if service.service_type_str == "Mount" {
                                        let _ = Command::new("fusermount")
                                            .args(["-uz", &service.path])
                                            .status();
                                    }
                                }
                                #[cfg(not(unix))]
                                {
                                    let _ = Command::new("taskkill")
                                        .args(["/F", "/PID", &service.pid.to_string()])
                                        .status();
                                }
                                std::thread::sleep(std::time::Duration::from_millis(100));
                                self.scan_running_services();
                                self.services_state.selected_active_idx = 0;
                            }
                        }
                        DeleteTarget::SystemdService(idx) => {
                            if idx < self.services_state.systemd_services.len() {
                                let service = self.services_state.systemd_services[idx].clone();
                                
                                // Stop và disable
                                if service.is_user {
                                    let _ = Command::new("systemctl")
                                        .args(["--user", "stop", &service.name])
                                        .status();
                                    let _ = Command::new("systemctl")
                                        .args(["--user", "disable", &service.name])
                                        .status();
                                } else {
                                    let _ = Command::new("pkexec")
                                        .args(["systemctl", "stop", &service.name])
                                        .status();
                                    let _ = Command::new("pkexec")
                                        .args(["systemctl", "disable", &service.name])
                                        .status();
                                }

                                // Xóa file cấu hình dịch vụ
                                let res = if service.is_user {
                                    std::fs::remove_file(&service.file_path)
                                } else {
                                    Command::new("pkexec")
                                        .args(["rm", "-f", &service.file_path])
                                        .status()
                                        .map(|_| ())
                                        .map_err(|e| e)
                                };

                                // Reload daemon
                                if service.is_user {
                                    let _ = Command::new("systemctl")
                                        .args(["--user", "daemon-reload"])
                                        .status();
                                } else {
                                    let _ = Command::new("pkexec")
                                        .args(["systemctl", "daemon-reload"])
                                        .status();
                                }

                                match res {
                                    Ok(_) => {
                                        self.services_state.info_message = Some(format!(
                                            "Đã dừng, tắt và xóa thành công dịch vụ '{}'!",
                                            service.name
                                        ));
                                    }
                                    Err(e) => {
                                        self.services_state.error_message = Some(format!(
                                            "Lỗi khi xóa tệp dịch vụ: {}", e
                                        ));
                                    }
                                }

                                self.scan_systemd_services();
                                self.services_state.selected_systemd_idx = 0;
                            }
                        }
                    }
                }
                _ => {}
            }
            return;
        }

        // ESC hủy popups thông báo chung (chặn tất cả phím khác khi đang hiện thông báo)
        if self.connection_state.error_message.is_some()
            || self.connection_state.info_message.is_some()
            || self.explorer_state.notification.is_some()
            || self.profile_state.error_message.is_some()
            || self.services_state.error_message.is_some()
            || self.services_state.info_message.is_some()
        {
            if key.code == KeyCode::Esc {
                self.connection_state.error_message = None;
                self.connection_state.info_message = None;
                self.explorer_state.notification = None;
                self.profile_state.error_message = None;
                self.services_state.error_message = None;
                self.services_state.info_message = None;
            }
            return;
        }

        match self.screen {
            Screen::MainMenu => self.handle_menu_key(key, tx).await,
            Screen::ConnectionManager => self.handle_connection_key(key, tx).await,
            Screen::FileExplorer => self.handle_explorer_key(key, tx).await,
            Screen::JobMonitor => self.handle_monitor_key(key, tx).await,
            Screen::ConfigProfileManager => self.handle_profile_key(key, tx).await,
            Screen::ServicesAndMounts => self.handle_services_key(key, tx).await,
            Screen::LanguageSelect => self.handle_language_key(key).await,
            Screen::DependencyManager => self.handle_dependency_key(key).await,
        }
    }
}

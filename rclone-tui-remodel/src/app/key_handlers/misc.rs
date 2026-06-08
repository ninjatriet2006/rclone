use crate::rclone;
use crate::ui;
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
use serde_json::json;
use std::fs;
use std::path::Path;
use std::process::Command;
use crate::app_config::{AppConfig, ExportResult};

use crate::app::{
    App, AppEvent, Screen
};

impl App {

    pub(crate) async fn handle_menu_key(
        &mut self,
        key: KeyEvent,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        match key.code {
            KeyCode::Up => self.menu_state.prev(),
            KeyCode::Down => self.menu_state.next(),
            KeyCode::Enter => match self.menu_state.selected_idx {
                0 => {
                    self.screen = Screen::ConnectionManager;
                    self.load_remotes(tx.clone()).await;
                }
                1 => {
                    self.screen = Screen::FileExplorer;
                    // Giữ nguyên trạng thái explorer nếu đã có dữ liệu
                    if self.explorer_state.left_pane.items.is_empty() && self.explorer_state.right_pane.items.is_empty() {
                        // Chỉ khởi tạo lại nếu chưa từng mở File Explorer
                        self.explorer_state = ui::explorer::ExplorerState::new();
                    }
                    self.load_remotes(tx.clone()).await;
                    self.refresh_explorer_pane(ui::explorer::ActivePane::Left, tx.clone())
                        .await;
                    self.refresh_explorer_pane(ui::explorer::ActivePane::Right, tx.clone())
                        .await;
                }
                2 => {
                    self.screen = Screen::JobMonitor;
                    // Kích hoạt quét tức thời khi chuyển sang màn hình Job Monitor
                    self.last_stats_scan = std::time::Instant::now() - std::time::Duration::from_secs(10);
                }
                3 => {
                    self.screen = Screen::ConfigProfileManager;
                    self.load_profile_list();
                }
                4 => {
                    self.screen = Screen::ServicesAndMounts;
                    self.load_remotes(tx.clone()).await;
                    // Kích hoạt quét tức thời khi chuyển sang màn hình dịch vụ
                    self.scan_running_services();
                    self.scan_systemd_services();
                    self.last_services_scan = std::time::Instant::now();
                }
                5 => {
                    self.screen = Screen::LanguageSelect;
                    self.available_languages = crate::lang::get_available_languages();
                    self.selected_lang_idx = self
                        .available_languages
                        .iter()
                        .position(|l| l == &self.config.active_language)
                        .unwrap_or(0);
                }
                6 => {
                    self.screen = Screen::DependencyManager;
                }
                7 => {
                    self.should_exit = true;
                }
                _ => {}
            },
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.should_exit = true;
            }
            _ => {}
        }
    }
    pub(crate) async fn handle_language_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                if !self.available_languages.is_empty() {
                    if self.selected_lang_idx == 0 {
                        self.selected_lang_idx = self.available_languages.len() - 1;
                    } else {
                        self.selected_lang_idx -= 1;
                    }
                }
            }
            KeyCode::Down => {
                if !self.available_languages.is_empty() {
                    self.selected_lang_idx =
                        (self.selected_lang_idx + 1) % self.available_languages.len();
                }
            }
            KeyCode::Enter => {
                if let Some(lang) = self
                    .available_languages
                    .get(self.selected_lang_idx)
                    .cloned()
                {
                    self.config.active_language = lang.clone();
                    let _ = self.config.save();
                    crate::lang::load_translation(&lang);
                }
                self.screen = Screen::MainMenu;
            }
            KeyCode::Esc => {
                self.screen = Screen::MainMenu;
            }
            _ => {}
        }
    }

    pub(crate) async fn handle_monitor_key(
        &mut self,
        key: KeyEvent,
        _tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        if self.monitor_state.confirm_stop_job.is_some() {
            match key.code {
                KeyCode::Esc => {
                    self.monitor_state.confirm_stop_job = None;
                }
                KeyCode::Enter => {
                    if let Some(job) = self.monitor_state.confirm_stop_job.take() {
                        let op_res = if let Some(id) = job.job_id {
                            let param = json!({ "jobid": id }).to_string();
                            rclone::rpc("job/stop", &param)
                        } else {
                            let param = json!({ "group": job.name }).to_string();
                            rclone::rpc("job/stopgroup", &param)
                        };
                        let msg = match op_res {
                            Ok(_) => format!("Đã yêu cầu hủy bỏ tác vụ: {}", job.name),
                            Err(e) => format!("Lỗi khi hủy tác vụ: {}", e),
                        };
                        self.monitor_state.history.push(msg);
                    }
                }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Esc => {
                    self.screen = Screen::MainMenu;
                }
                KeyCode::Tab => {
                    self.monitor_state.active_pane = match self.monitor_state.active_pane {
                        ui::monitor::MonitorPane::ActiveJobs => ui::monitor::MonitorPane::PendingJobs,
                        ui::monitor::MonitorPane::PendingJobs => ui::monitor::MonitorPane::FailedFiles,
                        ui::monitor::MonitorPane::FailedFiles => ui::monitor::MonitorPane::ActiveJobs,
                    };
                }
                KeyCode::Left => {
                    if self.monitor_state.active_pane == ui::monitor::MonitorPane::ActiveJobs {
                        self.monitor_state.collapse_node();
                    }
                }
                KeyCode::Right => {
                    if self.monitor_state.active_pane == ui::monitor::MonitorPane::ActiveJobs {
                        self.monitor_state.expand_node();
                    }
                }
                KeyCode::Char(' ') => {
                    if self.monitor_state.active_pane == ui::monitor::MonitorPane::ActiveJobs {
                        self.monitor_state.toggle_expand();
                    }
                }
                KeyCode::Up => self.monitor_state.prev(),
                KeyCode::Down => self.monitor_state.next(),
                KeyCode::Delete | KeyCode::Char('d') | KeyCode::Char('D') => {
                    match self.monitor_state.active_pane {
                        ui::monitor::MonitorPane::ActiveJobs => {
                            if !self.monitor_state.visible_nodes.is_empty() {
                                if self.monitor_state.selected_node_idx < self.monitor_state.visible_nodes.len() {
                                    let node = &self.monitor_state.visible_nodes[self.monitor_state.selected_node_idx];
                                    let job_opt = self.monitor_state.active_jobs.iter()
                                        .find(|j| j.job_id == node.job_id || j.name == node.job_name)
                                        .cloned();
                                    if let Some(job) = job_opt {
                                        self.monitor_state.confirm_stop_job = Some(job);
                                    }
                                }
                            }
                        }
                        ui::monitor::MonitorPane::PendingJobs => {
                            if !self.monitor_state.pending_jobs.is_empty() {
                                if self.monitor_state.selected_pending_idx < self.monitor_state.pending_jobs.len() {
                                    let removed = self.monitor_state.pending_jobs.remove(self.monitor_state.selected_pending_idx);
                                    self.monitor_state.history.push(format!("Đã xóa tác vụ chờ: {}", removed.src));
                                    if self.monitor_state.selected_pending_idx >= self.monitor_state.pending_jobs.len() {
                                        self.monitor_state.selected_pending_idx = self.monitor_state.pending_jobs.len().saturating_sub(1);
                                    }
                                }
                            }
                        }
                        ui::monitor::MonitorPane::FailedFiles => {
                            if !self.monitor_state.failed_files.is_empty() {
                                if self.monitor_state.selected_failed_idx < self.monitor_state.failed_files.len() {
                                    let removed = self.monitor_state.failed_files.remove(self.monitor_state.selected_failed_idx);
                                    self.monitor_state.history.push(format!("Đã xóa file lỗi khỏi danh sách: {}", removed.src));
                                    if self.monitor_state.selected_failed_idx >= self.monitor_state.failed_files.len() {
                                        self.monitor_state.selected_failed_idx = self.monitor_state.failed_files.len().saturating_sub(1);
                                    }
                                }
                            }
                        }
                    }
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if self.monitor_state.active_pane == ui::monitor::MonitorPane::FailedFiles {
                        if !self.monitor_state.failed_files.is_empty() {
                            if self.monitor_state.selected_failed_idx < self.monitor_state.failed_files.len() {
                                let failed_item = self.monitor_state.failed_files.remove(self.monitor_state.selected_failed_idx);
                                if self.monitor_state.selected_failed_idx >= self.monitor_state.failed_files.len() {
                                    self.monitor_state.selected_failed_idx = self.monitor_state.failed_files.len().saturating_sub(1);
                                }

                                let tx_clone = _tx.clone();
                                let src_clone = failed_item.src.clone();
                                let dest_clone = failed_item.dest.clone();
                                let is_copy = failed_item.is_copy;

                                if dest_clone.is_empty() {
                                    // It was a delete operation!
                                    self.monitor_state.history.push(format!("Đang khởi động lại tác vụ xóa: {}", src_clone));
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                                    tokio::spawn(async move {
                                        let res = crate::app::run_rpc_job_async(
                                            "operations/purge".to_string(),
                                            json!({
                                                "fs": src_clone,
                                                "remote": "",
                                            }),
                                        ).await;
                                        let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                                            pane: ui::explorer::ActivePane::Left,
                                            op_name: "Xóa tệp/thư mục (Thử lại)".to_string(),
                                            result: res,
                                        });
                                    });
                                } else {
                                    // Check if there is an ActiveOperation in active_ops.json that matches this failed_item
                                    let ops = crate::app::load_active_operations();
                                    let matching_op = ops.into_iter().find(|op| {
                                        op.src == src_clone && op.dest == dest_clone
                                    });

                                     if let Some(op) = matching_op {
                                         if op.tasks.is_some() {
                                             let op_id = op.id.clone();
                                             let src_path = op.src.clone();
                                             let dest_path = op.dest.clone();
                                             let is_dir = op.is_dir;
                                             let use_checksum = op.use_checksum;
                                             let is_copy = op.is_copy;
                                             let tx_op = tx_clone.clone();

                                             crate::app::prepare_active_operation_for_resume(&op_id);

                                             self.explorer_state.popup = if is_copy {
                                                 ui::explorer::ExplorerPopup::CopyProgress {
                                                     src: src_path.clone(),
                                                     dest: dest_path.clone(),
                                                     pct: 0.0,
                                                     job_id: None,
                                                 }
                                             } else {
                                                 ui::explorer::ExplorerPopup::MoveProgress {
                                                     src: src_path.clone(),
                                                     dest: dest_path.clone(),
                                                     pct: 0.0,
                                                     job_id: None,
                                                 }
                                             };

                                             self.monitor_state.history.push(format!("Đang khởi động lại tác vụ sao chép bất đồng bộ: {}", src_path));

                                             tokio::spawn(async move {
                                                 crate::app::operations::start_async_checker_and_transfer(
                                                     op_id,
                                                     src_path,
                                                     dest_path,
                                                     is_dir,
                                                     use_checksum,
                                                     is_copy,
                                                     tx_op,
                                                 ).await;
                                             });
                                             return;
                                         }

                                         if !op.items.is_empty() {
                                            // It's a multi-file operation! Resume copying the remaining items.
                                            let op_id = op.id.clone();
                                            let items_to_copy = op.items.clone();
                                            let dest_full = op.dest.clone();
                                            let use_checksum = op.use_checksum;
                                            let is_copy_action = op.is_copy;
                                            let action_type = op.action_type.clone();

                                            let (dest_remote, dest_path) = if let Some(idx) = dest_full.find(':') {
                                                (dest_full[..idx].to_string(), dest_full[idx+1..].to_string())
                                            } else {
                                                (String::new(), dest_full.clone())
                                            };
                                            let dest_remote_clone = dest_remote.clone();
                                            let dest_path_clone = dest_path.clone();

                                            let (src_remote, src_path) = if let Some(idx) = op.src.find(':') {
                                                (op.src[..idx].to_string(), op.src[idx+1..].to_string())
                                            } else {
                                                (String::new(), op.src.clone())
                                            };

                                            self.monitor_state.history.push(format!("Đang khởi động lại tác vụ sao chép nhiều mục từ: {}", op.src));

                                            let tx_op = tx_clone.clone();
                                            let pane_type = self.explorer_state.active_pane.clone();

                                            tokio::spawn(async move {
                                                let mut last_err = None;
                                                let total_count = items_to_copy.len();
                                                for (idx, item_name) in items_to_copy.iter().enumerate() {
                                                    let item_src = if src_remote.is_empty() {
                                                        std::path::PathBuf::from(&src_path)
                                                            .join(item_name)
                                                            .to_string_lossy()
                                                            .to_string()
                                                    } else {
                                                        let clean_remote = src_remote.trim_end_matches(':');
                                                        let clean_path = if src_path.starts_with('/') {
                                                            src_path.clone()
                                                        } else {
                                                            format!("/{}", src_path)
                                                        };
                                                        if clean_path.ends_with('/') {
                                                            format!("{}:{}{}", clean_remote, clean_path, item_name)
                                                        } else {
                                                            format!("{}:{}/{}", clean_remote, clean_path, item_name)
                                                        }
                                                    };

                                                    let item_dest = if dest_remote_clone.is_empty() {
                                                        std::path::PathBuf::from(&dest_path_clone)
                                                            .join(item_name)
                                                            .to_string_lossy()
                                                            .to_string()
                                                    } else {
                                                        let clean_remote = dest_remote_clone.trim_end_matches(':');
                                                        let clean_path = if dest_path_clone.starts_with('/') {
                                                            dest_path_clone.clone()
                                                        } else {
                                                            format!("/{}", dest_path_clone)
                                                        };
                                                        if clean_path.ends_with('/') {
                                                            format!("{}:{}{}", clean_remote, clean_path, item_name)
                                                        } else {
                                                            format!("{}:{}/{}", clean_remote, clean_path, item_name)
                                                        }
                                                    };

                                                    let pct = ((idx as f64) / total_count as f64) * 100.0;
                                                    let progress_event = if action_type == "move" {
                                                        AppEvent::MoveProgress {
                                                            src: format!("({}/{}) {}", idx + 1, total_count, item_name),
                                                            dest: item_dest.clone(),
                                                            pct,
                                                            job_id: None,
                                                        }
                                                    } else {
                                                        AppEvent::CopyProgress {
                                                            src: format!("({}/{}) {}", idx + 1, total_count, item_name),
                                                            dest: item_dest.clone(),
                                                            pct,
                                                            job_id: None,
                                                        }
                                                    };
                                                    let _ = tx_op.send(progress_event);

                                                    let method = if action_type == "move" {
                                                        "sync/move".to_string()
                                                    } else {
                                                        "sync/copy".to_string()
                                                    };

                                                    let mut param = json!({
                                                        "srcFs": item_src,
                                                        "dstFs": item_dest,
                                                    });
                                                    if use_checksum {
                                                        if let Some(obj) = param.as_object_mut() {
                                                            obj.insert("_config".to_string(), json!({ "checksum": true }));
                                                        }
                                                    }

                                                    let res = crate::app::run_rpc_job_async_with_progress(
                                                        method,
                                                        param,
                                                        Some((item_src.clone(), item_dest.clone(), is_copy_action)),
                                                        Some(tx_op.clone()),
                                                        None,
                                                    ).await;

                                                    match res {
                                                        Ok(_) => {
                                                            crate::app::complete_item_in_active_operation(&op_id, item_name);
                                                        }
                                                        Err(e) => {
                                                            last_err = Some(e);
                                                        }
                                                    }
                                                }

                                                let progress_event_done = if action_type == "move" {
                                                    AppEvent::MoveProgress {
                                                        src: format!("({} mục)", total_count),
                                                        dest: String::new(),
                                                        pct: 100.0,
                                                        job_id: None,
                                                    }
                                                } else {
                                                    AppEvent::CopyProgress {
                                                        src: format!("({} mục)", total_count),
                                                        dest: String::new(),
                                                        pct: 100.0,
                                                        job_id: None,
                                                    }
                                                };
                                                let _ = tx_op.send(progress_event_done);

                                                crate::app::remove_active_operation(&op_id);

                                                let final_result = match last_err {
                                                    None => Ok(()),
                                                    Some(e) => Err(e),
                                                };

                                                let op_label = if action_type == "move" { "di chuyển nhiều mục" } else { "sao chép nhiều mục" };
                                                let _ = tx_op.send(AppEvent::ExplorerOperationFinished {
                                                    pane: pane_type,
                                                    op_name: op_label.to_string(),
                                                    result: final_result,
                                                });
                                            });
                                            return;
                                        }
                                    }

                                    let method = if is_copy {
                                        "sync/copy".to_string()
                                    } else {
                                        "sync/move".to_string()
                                    };
                                    let param = json!({
                                        "srcFs": src_clone,
                                        "dstFs": dest_clone,
                                    });

                                    self.monitor_state.history.push(format!("Đang khởi động lại tác vụ: {} -> {}", src_clone, dest_clone));

                                    tokio::spawn(async move {
                                        let res = crate::app::run_rpc_job_async_with_progress(
                                            method,
                                            param,
                                            Some((src_clone, dest_clone, is_copy)),
                                            Some(tx_clone.clone()),
                                            None,
                                        ).await;
                                        let op_name = if is_copy { "sao chép (copy)" } else { "di chuyển (move)" };
                                        let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                                            pane: ui::explorer::ActivePane::Left,
                                            op_name: op_name.to_string(),
                                            result: res,
                                        });
                                    });
                                }
                            }
                        }
                    }
                }
                KeyCode::Enter | KeyCode::Char('c') | KeyCode::Char('C') => {
                    if self.monitor_state.active_pane == ui::monitor::MonitorPane::PendingJobs {
                        if !self.monitor_state.pending_jobs.is_empty() {
                            if self.monitor_state.selected_pending_idx < self.monitor_state.pending_jobs.len() {
                                let job = self.monitor_state.pending_jobs.remove(self.monitor_state.selected_pending_idx);
                                if self.monitor_state.selected_pending_idx >= self.monitor_state.pending_jobs.len() {
                                    self.monitor_state.selected_pending_idx = self.monitor_state.pending_jobs.len().saturating_sub(1);
                                }

                                let (dest_remote, dest_path) = if let Some(idx) = job.dest.find(':') {
                                    (job.dest[..idx].to_string(), job.dest[idx+1..].to_string())
                                } else {
                                    (String::new(), job.dest.clone())
                                };

                                let mut options = Vec::new();
                                let mut actions = Vec::new();

                                options.push(crate::lang::translate("exp_permission_option_cancel"));
                                actions.push(ui::explorer::FallbackAction::PermissionCancel);

                                if let Some(ref items) = job.items {
                                    options.push(crate::lang::translate("exp_permission_option_as_much"));
                                    actions.push(ui::explorer::FallbackAction::MultiPermissionCopyAsMuchAsPossible {
                                        items: items.clone(),
                                        dest_remote: dest_remote.clone(),
                                        dest_path: dest_path.clone(),
                                        restricted_files: job.restricted_files.clone(),
                                        use_checksum: job.use_checksum,
                                    });

                                    options.push(crate::lang::translate("exp_permission_option_restricted"));
                                    actions.push(ui::explorer::FallbackAction::MultiPermissionRestrictedCopy {
                                        items: items.clone(),
                                        dest_remote: dest_remote.clone(),
                                        dest_path: dest_path.clone(),
                                        restricted_files: job.restricted_files.clone(),
                                        use_checksum: job.use_checksum,
                                    });
                                } else {
                                    options.push(crate::lang::translate("exp_permission_option_as_much"));
                                    actions.push(ui::explorer::FallbackAction::PermissionCopyAsMuchAsPossible {
                                        src: job.src.clone(),
                                        dest: job.dest.clone(),
                                        is_dir: job.is_dir,
                                        restricted_files: job.restricted_files.clone(),
                                        use_checksum: job.use_checksum,
                                    });

                                    options.push(crate::lang::translate("exp_permission_option_restricted"));
                                    actions.push(ui::explorer::FallbackAction::PermissionRestrictedCopy {
                                        src: job.src.clone(),
                                        dest: job.dest.clone(),
                                        is_dir: job.is_dir,
                                        restricted_files: job.restricted_files.clone(),
                                        use_checksum: job.use_checksum,
                                    });
                                }

                                self.explorer_state.popup = ui::explorer::ExplorerPopup::ConfirmFallback {
                                    title: format!("GIẢI QUYẾT TÁC VỤ SAO CHÉP CHỜ ({})", job.src),
                                    options,
                                    selected_idx: 0,
                                    actions,
                                    restricted_files: Some(job.restricted_files),
                                    restricted_scroll: 0,
                                    focus_files: false,
                                };
                                self.screen = Screen::FileExplorer;
                            }
                        }
                    } else if self.monitor_state.active_pane == ui::monitor::MonitorPane::ActiveJobs && key.code == KeyCode::Enter {
                        self.monitor_state.toggle_expand();
                    }
                }
                _ => {}
            }
        }
    }

    pub(crate) async fn handle_profile_key(
        &mut self,
        key: KeyEvent,
        _tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        if self.profile_state.export_popup != ui::profile::ExportPopupState::None {
            if key.code == KeyCode::Enter || key.code == KeyCode::Esc {
                if let ui::profile::ExportPopupState::ConfirmOverwrite { profile_name } =
                    &self.profile_state.export_popup
                {
                    if key.code == KeyCode::Enter {
                        // Xác nhận ghi đè export
                        let res = self.config.export_profile(profile_name, true);
                        if let ExportResult::Success(path) = res {
                            self.profile_state.export_popup =
                                ui::profile::ExportPopupState::Success {
                                    path: path.to_string_lossy().to_string(),
                                };
                        }
                    } else {
                        self.profile_state.export_popup = ui::profile::ExportPopupState::None;
                    }
                } else {
                    self.profile_state.export_popup = ui::profile::ExportPopupState::None;
                }
            }
            return;
        }

        let wizard = self.profile_state.wizard.clone();
        match wizard {
            ui::profile::ImportWizardState::None => {
                match key.code {
                    KeyCode::Esc => {
                        self.screen = Screen::MainMenu;
                    }
                    KeyCode::Up => self.profile_state.prev(),
                    KeyCode::Down => self.profile_state.next(),
                    KeyCode::Enter => {
                        // Chọn kích hoạt profile cấu hình nóng
                        if !self.profile_state.profiles.is_empty() {
                            let name = self.profile_state.profiles[self.profile_state.selected_idx]
                                .0
                                .clone();
                            self.config.active_profile = name;
                            let _ = self.config.save();

                            // Cập nhật biến môi trường cho Go core
                            let path = self.config.get_active_profile_path();
                            unsafe {
                                std::env::set_var("RCLONE_CONFIG", &path);
                            }
                            let _ =
                                rclone::rpc("config/setpath", &json!({"path": path}).to_string());
                        }
                    }
                    KeyCode::Char('x') | KeyCode::Char('X') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Xuất Profile
                        if !self.profile_state.profiles.is_empty() {
                            let name = self.profile_state.profiles[self.profile_state.selected_idx]
                                .0
                                .clone();
                            let res = self.config.export_profile(&name, false);
                            match res {
                                ExportResult::Success(path) => {
                                    self.profile_state.export_popup =
                                        ui::profile::ExportPopupState::Success {
                                            path: path.to_string_lossy().to_string(),
                                        };
                                }
                                ExportResult::AlreadyExists(_) => {
                                    self.profile_state.export_popup =
                                        ui::profile::ExportPopupState::ConfirmOverwrite {
                                            profile_name: name,
                                        };
                                }
                                ExportResult::SourceNotFound => {
                                    self.profile_state.error_message =
                                        Some("Không tìm thấy tệp cấu hình nguồn.".to_string());
                                }
                                ExportResult::Error(e) => {
                                    self.profile_state.error_message = Some(e);
                                }
                            }
                        }
                    }
                    KeyCode::Insert => {
                        // Thêm Profile mới
                        self.profile_state.wizard =
                            ui::profile::ImportWizardState::InputProfileName {
                                input_buffer: String::new(),
                            };
                    }
                    _ => {}
                }
            }
            ui::profile::ImportWizardState::InputProfileName { mut input_buffer } => {
                match key.code {
                    KeyCode::Esc => {
                        self.profile_state.wizard = ui::profile::ImportWizardState::None;
                    }
                    KeyCode::Char(c) => {
                        input_buffer.push(c);
                        self.profile_state.wizard =
                            ui::profile::ImportWizardState::InputProfileName { input_buffer };
                    }
                    KeyCode::Backspace => {
                        input_buffer.pop();
                        self.profile_state.wizard =
                            ui::profile::ImportWizardState::InputProfileName { input_buffer };
                    }
                    KeyCode::Enter => {
                        let name = input_buffer.trim().to_string();
                        if !name.is_empty() {
                            self.profile_state.wizard =
                                ui::profile::ImportWizardState::SelectImportType {
                                    profile_name: name,
                                    selected_idx: 0,
                                };
                        }
                    }
                    _ => {}
                }
            }
            ui::profile::ImportWizardState::SelectImportType {
                profile_name,
                mut selected_idx,
            } => match key.code {
                KeyCode::Esc => {
                    self.profile_state.wizard = ui::profile::ImportWizardState::None;
                }
                KeyCode::Up | KeyCode::Down | KeyCode::Tab => {
                    selected_idx = if selected_idx == 0 { 1 } else { 0 };
                    self.profile_state.wizard = ui::profile::ImportWizardState::SelectImportType {
                        profile_name,
                        selected_idx,
                    };
                }
                KeyCode::Enter => {
                    self.profile_state.wizard = ui::profile::ImportWizardState::InputSource {
                        profile_name: profile_name.clone(),
                        import_type: selected_idx,
                        input_buffer: String::new(),
                    };
                }
                _ => {}
            },
            ui::profile::ImportWizardState::InputSource {
                profile_name,
                import_type,
                mut input_buffer,
            } => match key.code {
                KeyCode::Esc => {
                    self.profile_state.wizard = ui::profile::ImportWizardState::None;
                }
                KeyCode::Char(c) => {
                    input_buffer.push(c);
                    self.profile_state.wizard = ui::profile::ImportWizardState::InputSource {
                        profile_name,
                        import_type,
                        input_buffer,
                    };
                }
                KeyCode::Backspace => {
                    input_buffer.pop();
                    self.profile_state.wizard = ui::profile::ImportWizardState::InputSource {
                        profile_name,
                        import_type,
                        input_buffer,
                    };
                }
                KeyCode::Enter => {
                    let src = input_buffer.trim().to_string();
                    if !src.is_empty() {
                        let already_exists = self.config.profiles.contains_key(&profile_name);
                        if already_exists {
                            self.profile_state.wizard =
                                ui::profile::ImportWizardState::ConfirmImportOverwrite {
                                    profile_name: profile_name.clone(),
                                    source_path_or_url: src,
                                    import_type,
                                };
                        } else {
                            self.execute_import_profile(profile_name.clone(), src, import_type);
                        }
                    }
                }
                _ => {}
            },
            ui::profile::ImportWizardState::ConfirmImportOverwrite {
                profile_name,
                source_path_or_url,
                import_type,
            } => match key.code {
                KeyCode::Esc => {
                    self.profile_state.wizard = ui::profile::ImportWizardState::None;
                }
                KeyCode::Enter => {
                    self.execute_import_profile(
                        profile_name.clone(),
                        source_path_or_url.clone(),
                        import_type,
                    );
                }
                _ => {}
            },
        }
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
                    self.profile_state.wizard = ui::profile::ImportWizardState::None;
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
                self.profile_state.wizard = ui::profile::ImportWizardState::None;
            } else {
                self.profile_state.error_message =
                    Some("Tải cấu hình từ URL thất bại.".to_string());
            }
        }
    }

    pub(crate) async fn handle_dependency_key(&mut self, key: KeyEvent) {
        use std::io::Write;
        match key.code {
            KeyCode::Esc => {
                self.screen = Screen::MainMenu;
            }
            KeyCode::Up => {
                if self.selected_dependency_idx > 0 {
                    self.selected_dependency_idx -= 1;
                } else {
                    self.selected_dependency_idx = 1;
                }
            }
            KeyCode::Down => {
                if self.selected_dependency_idx < 1 {
                    self.selected_dependency_idx += 1;
                } else {
                    self.selected_dependency_idx = 0;
                }
            }
            KeyCode::Enter => {
                let idx = self.selected_dependency_idx;
                if idx == 0 {
                    // Cài đặt FUSE
                    #[cfg(all(unix, not(target_os = "macos")))]
                    {
                        let _ = crossterm::terminal::disable_raw_mode();
                        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
                        
                        println!("Đang chạy lệnh cập nhật và cài đặt fuse3...");
                        let status = std::process::Command::new("sudo").args(["apt-get", "update"]).status();
                        if status.is_ok() {
                            let status2 = std::process::Command::new("sudo")
                                .args(["apt-get", "install", "-y", "fuse3"])
                                .status();
                            if status2.is_ok() && status2.unwrap().success() {
                                println!("Cài đặt fuse3 thành công!");
                                self.fuse_installed = true;
                            } else {
                                println!("Lỗi: Cài đặt fuse3 thất bại.");
                            }
                        } else {
                            println!("Lỗi: Không thể chạy sudo.");
                        }
                        
                        println!("\nNhấn Enter để quay lại...");
                        let _ = std::io::stdout().flush();
                        let _ = std::io::stdin().read_line(&mut String::new());
                        
                        let _ = crossterm::terminal::enable_raw_mode();
                        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen);
                        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All));
                    }
                    #[cfg(target_os = "macos")]
                    {
                        let _ = crossterm::terminal::disable_raw_mode();
                        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
                        println!("------------------------------------------------------------------");
                        println!("Không phát hiện thư viện FUSE tương thích trên hệ thống.");
                        println!("FUSE là bắt buộc để sử dụng chức năng Mount ổ đĩa ảo trên macOS.");
                        println!("Để sử dụng chức năng này, vui lòng cài đặt một trong các lựa chọn sau:");
                        println!("1. Cài đặt macFUSE từ https://macfuse.io/ (Khuyên dùng)");
                        println!("   Hoặc cài đặt thông qua Homebrew: brew install --cask macfuse");
                        println!("   LƯU Ý: Với máy chip Apple Silicon (M1/M2/M3+), bạn cần vào");
                        println!("   chế độ Recovery Mode để bật nạp Kernel Extension bên thứ ba.");
                        println!("2. Sử dụng FUSE-T (Không cần Kernel Extension/Recovery Mode):");
                        println!("   Chi tiết xem tại: https://github.com/macos-fuse-t/fuse-t");
                        println!("------------------------------------------------------------------");
                        println!("\nNhấn Enter để quay lại...");
                        let _ = std::io::stdout().flush();
                        let _ = std::io::stdin().read_line(&mut String::new());
                        let _ = crossterm::terminal::enable_raw_mode();
                        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen);
                        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All));
                    }
                    #[cfg(windows)]
                    {
                        let _ = crossterm::terminal::disable_raw_mode();
                        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
                        println!("------------------------------------------------------------------");
                        println!("Không phát hiện tiện ích WinFsp (Windows File System Proxy) trên hệ thống.");
                        println!("WinFsp là bắt buộc đối với rclone để thực hiện chức năng Mount ổ đĩa ảo trên Windows.");
                        println!("Vui lòng tải và cài đặt WinFsp từ trang chủ: https://winfsp.dev/");
                        println!("------------------------------------------------------------------");
                        println!("\nNhấn Enter để quay lại...");
                        let _ = std::io::stdout().flush();
                        let _ = std::io::stdin().read_line(&mut String::new());
                        let _ = crossterm::terminal::enable_raw_mode();
                        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen);
                        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All));
                    }
                } else if idx == 1 {
                    // Cài đặt Filen CLI
                    let _ = crossterm::terminal::disable_raw_mode();
                    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
                    
                    println!("Đang chạy lệnh cài đặt Filen CLI...");
                    let status = std::process::Command::new("sh")
                        .arg("-c")
                        .arg("curl -sL https://filen.io/cli.sh | bash")
                        .status();
                        
                    if status.is_ok() && status.unwrap().success() {
                        println!("Cài đặt Filen CLI thành công!");
                        self.filen_cli_installed = true;
                    } else {
                        println!("Lỗi: Cài đặt Filen CLI thất bại.");
                    }
                    
                    println!("\nNhấn Enter để quay lại...");
                    let _ = std::io::stdout().flush();
                    let _ = std::io::stdin().read_line(&mut String::new());
                    
                    let _ = crossterm::terminal::enable_raw_mode();
                    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen);
                    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All));
                }
            }
            _ => {}
        }
    }
}

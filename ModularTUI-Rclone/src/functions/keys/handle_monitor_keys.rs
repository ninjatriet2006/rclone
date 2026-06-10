use crate::app::{App, AppEvent, Screen};
use crate::functions::*;
use crossterm::event::{KeyEvent, KeyCode};
use serde_json::json;

pub async fn handle_monitor_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
) {
    if app.monitor_state.confirm_stop_job.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.monitor_state.confirm_stop_job = None;
            }
            KeyCode::Enter => {
                if let Some(job) = app.monitor_state.confirm_stop_job.take() {
                    let op_res = if let Some(id) = job.job_id {
                        let param = json!({ "jobid": id }).to_string();
                        rpc("job/stop", &param)
                    } else {
                        let param = json!({ "group": job.name }).to_string();
                        rpc("job/stopgroup", &param)
                    };
                    let msg = match op_res {
                        Ok(_) => format!("Đã yêu cầu hủy bỏ tác vụ: {}", job.name),
                        Err(e) => format!("Lỗi khi hủy tác vụ: {}", e),
                    };
                    app.monitor_state.history.push(msg);
                }
            }
            _ => {}
        }
    } else {
        match key.code {
            KeyCode::Esc => {
                app.screen = Screen::MainMenu;
            }
            KeyCode::Tab => {
                app.monitor_state.active_pane = match app.monitor_state.active_pane {
                    MonitorPane::ActiveJobs => MonitorPane::PendingJobs,
                    MonitorPane::PendingJobs => MonitorPane::FailedFiles,
                    MonitorPane::FailedFiles => MonitorPane::ActiveJobs,
                };
            }
            KeyCode::Left => {
                if app.monitor_state.active_pane == MonitorPane::ActiveJobs {
                    app.monitor_state.collapse_node();
                }
            }
            KeyCode::Right => {
                if app.monitor_state.active_pane == MonitorPane::ActiveJobs {
                    app.monitor_state.expand_node();
                }
            }
            KeyCode::Char(' ') => {
                if app.monitor_state.active_pane == MonitorPane::ActiveJobs {
                    app.monitor_state.toggle_expand();
                }
            }
            KeyCode::Up => app.monitor_state.prev(),
            KeyCode::Down => app.monitor_state.next(),
            KeyCode::Delete | KeyCode::Char('d') | KeyCode::Char('D') => {
                match app.monitor_state.active_pane {
                    MonitorPane::ActiveJobs => {
                        if !app.monitor_state.visible_nodes.is_empty() {
                            if app.monitor_state.selected_node_idx < app.monitor_state.visible_nodes.len() {
                                let node = &app.monitor_state.visible_nodes[app.monitor_state.selected_node_idx];
                                let job_opt = app.monitor_state.active_jobs.iter()
                                    .find(|j| j.job_id == node.job_id || j.name == node.job_name)
                                    .cloned();
                                if let Some(job) = job_opt {
                                    app.monitor_state.confirm_stop_job = Some(job);
                                }
                            }
                        }
                    }
                    MonitorPane::PendingJobs => {
                        if !app.monitor_state.pending_jobs.is_empty() {
                            if app.monitor_state.selected_pending_idx < app.monitor_state.pending_jobs.len() {
                                let removed = app.monitor_state.pending_jobs.remove(app.monitor_state.selected_pending_idx);
                                app.monitor_state.history.push(format!("Đã xóa tác vụ chờ: {}", removed.src));
                                if app.monitor_state.selected_pending_idx >= app.monitor_state.pending_jobs.len() {
                                    app.monitor_state.selected_pending_idx = app.monitor_state.pending_jobs.len().saturating_sub(1);
                                }
                            }
                        }
                    }
                    MonitorPane::FailedFiles => {
                        if !app.monitor_state.failed_files.is_empty() {
                            if app.monitor_state.selected_failed_idx < app.monitor_state.failed_files.len() {
                                let removed = app.monitor_state.failed_files.remove(app.monitor_state.selected_failed_idx);
                                app.monitor_state.history.push(format!("Đã xóa file lỗi khỏi danh sách: {}", removed.src));
                                if app.monitor_state.selected_failed_idx >= app.monitor_state.failed_files.len() {
                                    app.monitor_state.selected_failed_idx = app.monitor_state.failed_files.len().saturating_sub(1);
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if app.monitor_state.active_pane == MonitorPane::FailedFiles {
                    if !app.monitor_state.failed_files.is_empty() {
                        if app.monitor_state.selected_failed_idx < app.monitor_state.failed_files.len() {
                            let failed_item = app.monitor_state.failed_files.remove(app.monitor_state.selected_failed_idx);
                            if app.monitor_state.selected_failed_idx >= app.monitor_state.failed_files.len() {
                                app.monitor_state.selected_failed_idx = app.monitor_state.failed_files.len().saturating_sub(1);
                            }

                            let tx_clone = tx.clone();
                            let src_clone = failed_item.src.clone();
                            let dest_clone = failed_item.dest.clone();
                            let is_copy = failed_item.is_copy;

                            if dest_clone.is_empty() {
                                // It was a delete operation!
                                app.monitor_state.history.push(format!("Đang khởi động lại tác vụ xóa: {}", src_clone));
                                app.explorer_state.popup = ExplorerPopup::None;
                                tokio::spawn(async move {
                                    let res = run_rpc_job_async(
                                        "operations/purge".to_string(),
                                        json!({
                                            "fs": src_clone,
                                            "remote": "",
                                        }),
                                    ).await;
                                    let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                                        pane: ActivePane::Left,
                                        op_name: "Xóa tệp/thư mục (Thử lại)".to_string(),
                                        result: res,
                                    });
                                });
                            } else {
                                // Check if there is an ActiveOperation in SQLite DB that matches this failed_item
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

                                        app.explorer_state.popup = if is_copy {
                                            ExplorerPopup::CopyProgress {
                                                src: src_path.clone(),
                                                dest: dest_path.clone(),
                                                pct: 0.0,
                                                job_id: None,
                                            }
                                        } else {
                                            ExplorerPopup::MoveProgress {
                                                src: src_path.clone(),
                                                dest: dest_path.clone(),
                                                pct: 0.0,
                                                job_id: None,
                                            }
                                        };

                                        app.monitor_state.history.push(format!("Đang khởi động lại tác vụ sao chép bất đồng bộ: {}", src_path));

                                        let skip_flag = app.skip_permission_precheck.clone();
                                        tokio::spawn(async move {
                                            crate::app::start_async_checker_and_transfer::start_async_checker_and_transfer(
                                                op_id,
                                                src_path,
                                                dest_path,
                                                is_dir,
                                                use_checksum,
                                                is_copy,
                                                None,
                                                skip_flag,
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

                                        app.monitor_state.history.push(format!("Đang khởi động lại tác vụ sao chép nhiều mục từ: {}", op.src));

                                        let tx_op = tx_clone.clone();
                                        let pane_type = app.explorer_state.active_pane.clone();

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

                                                let res = run_rpc_job_async_with_progress(
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

                                app.monitor_state.history.push(format!("Đang khởi động lại tác vụ: {} -> {}", src_clone, dest_clone));

                                tokio::spawn(async move {
                                    let res = run_rpc_job_async_with_progress(
                                        method,
                                        param,
                                        Some((src_clone, dest_clone, is_copy)),
                                        Some(tx_clone.clone()),
                                        None,
                                    ).await;
                                    let op_name = if is_copy { "sao chép (copy)" } else { "di chuyển (move)" };
                                    let _ = tx_clone.send(AppEvent::ExplorerOperationFinished {
                                        pane: ActivePane::Left,
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
                if app.monitor_state.active_pane == MonitorPane::PendingJobs {
                    if !app.monitor_state.pending_jobs.is_empty() {
                        if app.monitor_state.selected_pending_idx < app.monitor_state.pending_jobs.len() {
                            let job = app.monitor_state.pending_jobs.remove(app.monitor_state.selected_pending_idx);
                            if app.monitor_state.selected_pending_idx >= app.monitor_state.pending_jobs.len() {
                                app.monitor_state.selected_pending_idx = app.monitor_state.pending_jobs.len().saturating_sub(1);
                            }

                            let (dest_remote, dest_path) = if let Some(idx) = job.dest.find(':') {
                                (job.dest[..idx].to_string(), job.dest[idx+1..].to_string())
                            } else {
                                (String::new(), job.dest.clone())
                            };

                            let mut options = Vec::new();
                            let mut actions = Vec::new();

                            options.push(translate("exp_permission_option_cancel"));
                            actions.push(FallbackAction::PermissionCancel);

                            if let Some(ref items) = job.items {
                                options.push(translate("exp_permission_option_as_much"));
                                actions.push(FallbackAction::MultiPermissionCopyAsMuchAsPossible {
                                    items: items.clone(),
                                    dest_remote: dest_remote.clone(),
                                    dest_path: dest_path.clone(),
                                    restricted_files: job.restricted_files.clone(),
                                    use_checksum: job.use_checksum,
                                });

                                options.push(translate("exp_permission_option_restricted"));
                                actions.push(FallbackAction::MultiPermissionRestrictedCopy {
                                    items: items.clone(),
                                    dest_remote: dest_remote.clone(),
                                    dest_path: dest_path.clone(),
                                    restricted_files: job.restricted_files.clone(),
                                    use_checksum: job.use_checksum,
                                });
                            } else {
                                options.push(translate("exp_permission_option_as_much"));
                                actions.push(FallbackAction::PermissionCopyAsMuchAsPossible {
                                    src: job.src.clone(),
                                    dest: job.dest.clone(),
                                    is_dir: job.is_dir,
                                    restricted_files: job.restricted_files.clone(),
                                    use_checksum: job.use_checksum,
                                });

                                options.push(translate("exp_permission_option_restricted"));
                                actions.push(FallbackAction::PermissionRestrictedCopy {
                                    src: job.src.clone(),
                                    dest: job.dest.clone(),
                                    is_dir: job.is_dir,
                                    restricted_files: job.restricted_files.clone(),
                                    use_checksum: job.use_checksum,
                                });
                            }

                            app.explorer_state.popup = ExplorerPopup::ConfirmFallback {
                                title: format!("GIẢI QUYẾT TÁC VỤ SAO CHÉP CHỜ ({})", job.src),
                                options,
                                selected_idx: 0,
                                actions,
                                restricted_files: Some(job.restricted_files),
                                restricted_scroll: 0,
                                focus_files: false,
                            };
                            app.screen = Screen::FileExplorer;
                        }
                    }
                } else if app.monitor_state.active_pane == MonitorPane::ActiveJobs {
                    app.monitor_state.toggle_expand();
                }
            }
            KeyCode::Char('_') => {
                let rx_res = rpc_async("core/stats-reset".to_string(), "{}".to_string()).await;
                let msg = match rx_res {
                    Ok(_) => {
                        app.monitor_state.failed_files.clear();
                        app.monitor_state.selected_failed_idx = 0;
                        "Đã dọn dẹp danh sách các tác vụ hoàn thành (Clear completed).".to_string()
                    }
                    Err(e) => format!("Lỗi khi dọn dẹp tác vụ: {}", e),
                };
                app.monitor_state.history.push(msg);
            }
            _ => {}
        }
    }
}

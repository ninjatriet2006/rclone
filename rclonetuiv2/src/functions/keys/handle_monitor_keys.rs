use crate::app::{App, AppEvent, Screen};
use crate::functions::*;
use crossterm::event::{KeyEvent, KeyCode};
use serde_json::json;

pub async fn handle_monitor_keys(
    app: &mut App,
    key: KeyEvent,
    _tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
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
                    MonitorPane::PendingJobs => MonitorPane::ActiveJobs,
                };
            }
            KeyCode::Up => app.monitor_state.prev(),
            KeyCode::Down => app.monitor_state.next(),
            KeyCode::Delete | KeyCode::Char('d') | KeyCode::Char('D') => {
                match app.monitor_state.active_pane {
                    MonitorPane::ActiveJobs => {
                        if !app.monitor_state.active_jobs.is_empty() {
                            if app.monitor_state.selected_job_idx < app.monitor_state.active_jobs.len() {
                                let job = app.monitor_state.active_jobs[app.monitor_state.selected_job_idx].clone();
                                app.monitor_state.confirm_stop_job = Some(job);
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
                }
            }
            _ => {}
        }
    }
}

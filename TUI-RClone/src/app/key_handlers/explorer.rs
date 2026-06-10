use crate::rclone;
use crate::ui;
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
use serde_json::json;
use std::path::PathBuf;

use crate::app::{
    App, AppEvent, Screen, DeleteTarget, handle_input_key, run_rpc_job_async
};

impl App {

    pub(crate) async fn handle_explorer_key(
        &mut self,
        key: KeyEvent,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let popup = self.explorer_state.popup.clone();
        if popup != ui::explorer::ExplorerPopup::None {
            match popup {

                ui::explorer::ExplorerPopup::InputRename {
                    old_name,
                    mut input_buffer,
                    is_dir,
                } => {
                    let mut cursor = self.explorer_state.edit_cursor_idx;
                    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                        self.explorer_state.edit_cursor_idx = cursor;
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::InputRename {
                            old_name,
                            input_buffer,
                            is_dir,
                        };
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            }
                            KeyCode::Enter => {
                                let new_name = input_buffer.trim().to_string();
                                if !new_name.is_empty() && new_name != old_name {
                                    let pane = self.explorer_state.get_active_pane();
                                    let remote = pane.remote.clone();
                                    let parent_path = pane.path.clone();
                                    
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::None;

                                    let src = if remote.is_empty() {
                                        PathBuf::from(&parent_path).join(&old_name).to_string_lossy().to_string()
                                    } else {
                                        let clean_path = parent_path.trim_start_matches('/').trim_end_matches('/');
                                        if clean_path.is_empty() {
                                            format!("{}:/{}", remote.trim_end_matches(':'), old_name)
                                        } else {
                                            format!("{}:/{}/{}", remote.trim_end_matches(':'), clean_path, old_name)
                                        }
                                    };

                                    let dest = if remote.is_empty() {
                                        PathBuf::from(&parent_path).join(&new_name).to_string_lossy().to_string()
                                    } else {
                                        let clean_path = parent_path.trim_start_matches('/').trim_end_matches('/');
                                        if clean_path.is_empty() {
                                            format!("{}:/{}", remote.trim_end_matches(':'), new_name)
                                        } else {
                                            format!("{}:/{}/{}", remote.trim_end_matches(':'), clean_path, new_name)
                                        }
                                    };

                                    self.check_features_and_execute("rename", src, dest, is_dir, false, tx.clone());
                                } else {
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                                }
                            }
                            _ => {}
                        }
                    }
                }

                ui::explorer::ExplorerPopup::InputNewFolder { mut input_buffer } => {
                    let mut cursor = self.explorer_state.edit_cursor_idx;
                    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                        self.explorer_state.edit_cursor_idx = cursor;
                        self.explorer_state.popup =
                            ui::explorer::ExplorerPopup::InputNewFolder { input_buffer };
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            }
                            KeyCode::Enter => {
                                let folder_name = input_buffer.trim().to_string();
                                if !folder_name.is_empty() {
                                let pane = self.explorer_state.get_active_pane_mut();
                                let is_local = pane.remote.is_empty();
                                let target = if is_local {
                                    PathBuf::from(&pane.path)
                                        .join(&folder_name)
                                        .to_string_lossy()
                                        .to_string()
                                } else {
                                    String::new()
                                };

                                // Chèn thư mục mới một cách lạc quan vào danh sách UI
                                if !pane.items.iter().any(|item| item.name == folder_name) {
                                    pane.items.push(ui::explorer::FileItem {
                                        name: folder_name.clone(),
                                        size: 0,
                                        is_dir: true,
                                        mod_time: crate::lang::translate("exp_creating_placeholder"),
                                        id: None,
                                    });
                                    pane.items.sort_by(|a, b| {
                                        b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name))
                                    });
                                    if let Some(pos) = pane.items.iter().position(|item| item.name == folder_name) {
                                        pane.selected_idx = pos;
                                    }
                                }

                                let param = if is_local {
                                    json!({
                                        "fs": target,
                                        "remote": "",
                                    })
                                } else {
                                    let clean_path = pane.path.trim_start_matches('/').trim_end_matches('/');
                                    let parent_fs = if clean_path.is_empty() {
                                        format!("{}:", pane.remote.trim_end_matches(':'))
                                    } else {
                                        format!("{}:/{}", pane.remote.trim_end_matches(':'), clean_path)
                                    };
                                    json!({
                                        "fs": parent_fs,
                                        "remote": folder_name.clone(),
                                    })
                                }
                                .to_string();

                                let tx_op = tx.clone();
                                let pane_type = self.explorer_state.active_pane.clone();
                                tokio::spawn(async move {
                                    let res = if is_local {
                                        if std::fs::create_dir_all(&target).is_ok() {
                                            Ok(())
                                        } else {
                                            match std::process::Command::new("pkexec")
                                                .args(&["mkdir", "-p", &target])
                                                .status()
                                            {
                                                Ok(s) if s.success() => Ok(()),
                                                Ok(s) => Err(format!("Quyền root bị từ chối hoặc thất bại (exit: {})", s)),
                                                Err(e) => Err(format!("Lỗi chạy pkexec: {}", e)),
                                            }
                                        }
                                    } else {
                                        let op_res = rclone::rpc_async("operations/mkdir".to_string(), param).await;
                                        match op_res {
                                            Ok(r) if r.status == 200 => Ok(()),
                                            Ok(r) => Err(format!("Mã lỗi: {}", r.status)),
                                            Err(e) => Err(e),
                                        }
                                    };
                                    let _ = tx_op.send(AppEvent::ExplorerOperationFinished {
                                        pane: pane_type,
                                        op_name: "tạo thư mục (mkdir)".to_string(),
                                        result: res,
                                    });
                                });
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            }
                        }
                        _ => {}
                    }
                }
              }
                ui::explorer::ExplorerPopup::SelectRemote {
                    remotes,
                    mut selected_idx,
                } => match key.code {
                    KeyCode::Esc => {
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = remotes.len() - 1;
                        } else {
                            selected_idx -= 1;
                        }
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::SelectRemote {
                            remotes,
                            selected_idx,
                        };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % remotes.len();
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::SelectRemote {
                            remotes,
                            selected_idx,
                        };
                    }
                    KeyCode::Enter => {
                        let chosen = remotes[selected_idx].clone();
                        if chosen == crate::lang::translate("exp_add_shared_link_option") {
                            self.explorer_state.edit_cursor_idx = 0;
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::InputSharedLink {
                                input_buffer: String::new(),
                            };
                        } else {
                            let active_pane = self.explorer_state.get_active_pane_mut();
                            if chosen == "[Local System]" {
                                active_pane.remote = String::new();
                                active_pane.path = crate::app_config::get_home_dir();
                            } else {
                                active_pane.remote = chosen;
                                active_pane.path = String::new();
                            }
                            active_pane.items.clear();
                            active_pane.selected_idx = 0;
                            active_pane.scroll_offset = 0;
                            active_pane.selected_names.clear();
                            active_pane.shift_anchor = None;
                            active_pane.shift_active = false;
                            active_pane.alt_anchor = None;
                            active_pane.alt_active = false;
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;

                            let p_type = self.explorer_state.active_pane.clone();
                            self.refresh_explorer_pane(p_type, tx.clone()).await;
                        }
                    }
                    _ => {}
                },
                ui::explorer::ExplorerPopup::ConfirmFallback {
                    title,
                    options,
                    mut selected_idx,
                    actions,
                    restricted_files,
                    mut restricted_scroll,
                    mut focus_files,
                } => {
                    if (key.code == KeyCode::Char('c') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL))
                        || key.code == KeyCode::Esc
                    {
                        if let Some(ref files) = restricted_files {
                            let mut src = String::new();
                            let mut dest = String::new();
                            let mut is_dir = false;
                            let mut items = None;
                            let mut use_checksum = false;

                            for act in &actions {
                                match act {
                                    ui::explorer::FallbackAction::PermissionCopyAsMuchAsPossible { src: s, dest: d, is_dir: id, use_checksum: uc, .. } => {
                                        src = s.clone();
                                        dest = d.clone();
                                        is_dir = *id;
                                        use_checksum = *uc;
                                        break;
                                    }
                                    ui::explorer::FallbackAction::MultiPermissionCopyAsMuchAsPossible { items: its, dest_remote, dest_path, use_checksum: uc, .. } => {
                                        src = format!("({} mục)", its.len());
                                        dest = if dest_remote.is_empty() { dest_path.clone() } else { format!("{}:{}", dest_remote, dest_path) };
                                        is_dir = true;
                                        items = Some(its.clone());
                                        use_checksum = *uc;
                                        break;
                                    }
                                    _ => {}
                                }
                            }

                            if !src.is_empty() {
                                self.monitor_state.pending_jobs.push(ui::monitor::PendingCopyJob {
                                    src,
                                    dest,
                                    is_dir,
                                    total_files: 0,
                                    restricted_files: files.clone(),
                                    status: "Scanned (Has Restrictions)".to_string(),
                                    items,
                                    use_checksum,
                                });
                                self.monitor_state.history.push("Đã chuyển tác vụ có file restricted vào hàng chờ".to_string());
                            }
                        }
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                    } else {
                        match key.code {
                            KeyCode::Tab => {
                                if restricted_files.is_some() {
                                    focus_files = !focus_files;
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::ConfirmFallback {
                                        title,
                                        options,
                                        selected_idx,
                                        actions,
                                        restricted_files,
                                        restricted_scroll,
                                        focus_files,
                                    };
                                }
                            }
                            KeyCode::Up => {
                                if focus_files {
                                    if restricted_scroll > 0 {
                                        restricted_scroll -= 1;
                                    }
                                } else {
                                    if selected_idx == 0 {
                                        selected_idx = options.len() - 1;
                                    } else {
                                        selected_idx -= 1;
                                    }
                                }
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::ConfirmFallback {
                                    title,
                                    options,
                                    selected_idx,
                                    actions,
                                    restricted_files,
                                    restricted_scroll,
                                    focus_files,
                                };
                            }
                            KeyCode::Down => {
                                if focus_files {
                                    if let Some(ref files) = restricted_files {
                                        if restricted_scroll + 1 < files.len() {
                                            restricted_scroll += 1;
                                        }
                                    }
                                } else {
                                    selected_idx = (selected_idx + 1) % options.len();
                                }
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::ConfirmFallback {
                                    title,
                                    options,
                                    selected_idx,
                                    actions,
                                    restricted_files,
                                    restricted_scroll,
                                    focus_files,
                                };
                            }
                            KeyCode::Enter => {
                                if !focus_files {
                                    let selected_action = actions[selected_idx].clone();
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                                    self.execute_fallback_action(selected_action, tx.clone()).await;
                                }
                            }
                            _ => {}
                        }
                    }
                },
                ui::explorer::ExplorerPopup::PermissionScanning { src, dest, is_dir, items, use_checksum, .. } => {
                    if key.code == KeyCode::Char('c') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                        self.skip_permission_precheck.store(true, std::sync::atomic::Ordering::Relaxed);
                        self.monitor_state.pending_jobs.push(ui::monitor::PendingCopyJob {
                            src,
                            dest,
                            is_dir,
                            total_files: 0,
                            restricted_files: Vec::new(),
                            status: "Bypassed".to_string(),
                            items,
                            use_checksum,
                        });
                        self.monitor_state.history.push("Đã bỏ qua quét quyền sở hữu và chuyển tác vụ vào hàng chờ".to_string());
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                    } else if key.code == KeyCode::Esc {
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                    }
                }
                ui::explorer::ExplorerPopup::CopyProgress { job_id, .. }
                | ui::explorer::ExplorerPopup::MoveProgress { job_id, .. } => {
                    if (key.code == KeyCode::Char('c') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL))
                        || key.code == KeyCode::Esc
                    {
                        if let Some(id) = job_id {
                            let param = json!({ "jobid": id }).to_string();
                            let _ = rclone::rpc("job/stop", &param);
                        }
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                    }
                }
                ui::explorer::ExplorerPopup::SpecialActionsMenu { mut selected_idx } => {
                    match key.code {
                        KeyCode::Esc => {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        }
                        KeyCode::Up => {
                            selected_idx = if selected_idx == 0 { 9 } else { selected_idx - 1 };
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::SpecialActionsMenu { selected_idx };
                        }
                        KeyCode::Down => {
                            selected_idx = (selected_idx + 1) % 10;
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::SpecialActionsMenu { selected_idx };
                        }
                        KeyCode::Enter => {
                            if selected_idx == 9 {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            } else {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                                self.handle_special_action_selected(selected_idx, tx.clone()).await;
                            }
                        }
                        _ => {}
                    }
                }
                ui::explorer::ExplorerPopup::ViewFile { file_name, content, mut scroll_offset } => {
                    match key.code {
                        KeyCode::Esc => {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        }
                        KeyCode::Up => {
                            if scroll_offset > 0 {
                                scroll_offset -= 1;
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::ViewFile { file_name, content, scroll_offset };
                            }
                        }
                        KeyCode::Down => {
                            if scroll_offset + 1 < content.len() {
                                scroll_offset += 1;
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::ViewFile { file_name, content, scroll_offset };
                            }
                        }
                        _ => {}
                    }
                }
                ui::explorer::ExplorerPopup::ChecksumTypeSelect { mut selected_idx } => {
                    let hash_types = vec!["md5".to_string(), "sha1".to_string(), "sha256".to_string(), "crc32".to_string(), "xxhash".to_string()];
                    match key.code {
                        KeyCode::Esc => {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        }
                        KeyCode::Up => {
                            selected_idx = if selected_idx == 0 { hash_types.len() - 1 } else { selected_idx - 1 };
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::ChecksumTypeSelect { selected_idx };
                        }
                        KeyCode::Down => {
                            selected_idx = (selected_idx + 1) % hash_types.len();
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::ChecksumTypeSelect { selected_idx };
                        }
                        KeyCode::Enter => {
                            let hash_type = hash_types[selected_idx].clone();
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            self.execute_hashsum_file(hash_type, tx.clone()).await;
                        }
                        _ => {}
                    }
                }
                ui::explorer::ExplorerPopup::CryptdecodeForm { mut remote_input, mut encrypted_input, mut is_remote_focused, output_result } => {
                    let mut cursor = self.explorer_state.edit_cursor_idx;
                    let handled = if is_remote_focused {
                        handle_input_key(&key, &mut remote_input, &mut cursor)
                    } else {
                        handle_input_key(&key, &mut encrypted_input, &mut cursor)
                    };
                    if handled {
                        self.explorer_state.edit_cursor_idx = cursor;
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::CryptdecodeForm { remote_input, encrypted_input, is_remote_focused, output_result };
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            }
                            KeyCode::Tab => {
                                is_remote_focused = !is_remote_focused;
                                self.explorer_state.edit_cursor_idx = if is_remote_focused {
                                    remote_input.chars().count()
                                } else {
                                    encrypted_input.chars().count()
                                };
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::CryptdecodeForm { remote_input, encrypted_input, is_remote_focused, output_result };
                            }
                            KeyCode::Enter => {
                                self.execute_cryptdecode(remote_input.clone(), encrypted_input.clone(), tx.clone()).await;
                            }
                            _ => {}
                        }
                    }
                }
                ui::explorer::ExplorerPopup::DecompressModeSelect { archive_path, mut selected_idx } => {
                    match key.code {
                        KeyCode::Esc => {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        }
                        KeyCode::Up => {
                            selected_idx = if selected_idx == 0 { 2 } else { selected_idx - 1 };
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::DecompressModeSelect { archive_path, selected_idx };
                        }
                        KeyCode::Down => {
                            selected_idx = (selected_idx + 1) % 3;
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::DecompressModeSelect { archive_path, selected_idx };
                        }
                        KeyCode::Enter => {
                            self.handle_decompress_mode_selected(archive_path, selected_idx, tx.clone()).await;
                        }
                        _ => {}
                    }
                }
                ui::explorer::ExplorerPopup::DecompressPathInput { archive_path, mut selected_idx } => {
                    match key.code {
                        KeyCode::Esc => {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        }
                        KeyCode::Up | KeyCode::Down => {
                            selected_idx = (selected_idx + 1) % 2;
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::DecompressPathInput { archive_path, selected_idx };
                        }
                        KeyCode::Enter => {
                            if selected_idx == 0 {
                                let active_pane = self.explorer_state.get_active_pane();
                                let initial_path = if active_pane.remote.is_empty() {
                                    active_pane.path.clone()
                                } else {
                                    format!("{}:{}", active_pane.remote.trim_end_matches(':'), active_pane.path)
                                };
                                self.explorer_state.edit_cursor_idx = initial_path.chars().count();
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::DecompressPathManualInput {
                                    archive_path,
                                    input_buffer: initial_path,
                                };
                            } else {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::TuiExplorerSelector {
                                    archive_path,
                                    remote: String::new(),
                                    path: String::new(),
                                    items: Vec::new(),
                                    selected_idx: 0,
                                    scroll_offset: 0,
                                    loading: true,
                                };
                                self.refresh_tui_selector_list(tx.clone());
                            }
                        }
                        _ => {}
                    }
                }
                ui::explorer::ExplorerPopup::DecompressPathManualInput { archive_path, mut input_buffer } => {
                    let mut cursor = self.explorer_state.edit_cursor_idx;
                    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                        self.explorer_state.edit_cursor_idx = cursor;
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::DecompressPathManualInput { archive_path, input_buffer };
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            }
                            KeyCode::Enter => {
                                let dest_path = input_buffer.trim().to_string();
                                if !dest_path.is_empty() {
                                    self.execute_archive_decompress(archive_path, dest_path, tx.clone()).await;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                ui::explorer::ExplorerPopup::TuiExplorerSelector {
                    archive_path,
                    mut remote,
                    mut path,
                    items,
                    mut selected_idx,
                    mut scroll_offset,
                    loading,
                } => {
                    if loading {
                        if key.code == KeyCode::Esc {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        }
                        return;
                    }
                    match key.code {
                        KeyCode::Esc => {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        }
                        KeyCode::Up => {
                            if !items.is_empty() {
                                if selected_idx == 0 {
                                    selected_idx = items.len() - 1;
                                } else {
                                    selected_idx -= 1;
                                }
                                let term_h = crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                                let popup_h = term_h * 70 / 100;
                                let list_h = popup_h.saturating_sub(4);
                                scroll_offset = ui::update_scroll_offset(selected_idx, scroll_offset, list_h, items.len());
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::TuiExplorerSelector {
                                    archive_path, remote, path, items, selected_idx, scroll_offset, loading
                                };
                            }
                        }
                        KeyCode::Down => {
                            if !items.is_empty() {
                                selected_idx = (selected_idx + 1) % items.len();
                                let term_h = crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                                let popup_h = term_h * 70 / 100;
                                let list_h = popup_h.saturating_sub(4);
                                scroll_offset = ui::update_scroll_offset(selected_idx, scroll_offset, list_h, items.len());
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::TuiExplorerSelector {
                                    archive_path, remote, path, items, selected_idx, scroll_offset, loading
                                };
                            }
                        }
                        KeyCode::Enter => {
                            if !items.is_empty() {
                                let selected = items[selected_idx].clone();
                                if selected.name == "[Local System]" {
                                    remote = String::new();
                                    path = "/".to_string();
                                    selected_idx = 0;
                                    scroll_offset = 0;
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::TuiExplorerSelector {
                                        archive_path, remote, path, items: Vec::new(), selected_idx, scroll_offset, loading: true
                                    };
                                    self.refresh_tui_selector_list(tx.clone());
                                } else if selected.name.ends_with(':') {
                                    remote = selected.name.clone();
                                    path = String::new();
                                    selected_idx = 0;
                                    scroll_offset = 0;
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::TuiExplorerSelector {
                                        archive_path, remote, path, items: Vec::new(), selected_idx, scroll_offset, loading: true
                                    };
                                    self.refresh_tui_selector_list(tx.clone());
                                } else if selected.name == ".." {
                                    if !path.is_empty() && path != "/" {
                                        if let Some(idx) = path.rfind('/') {
                                            path = path[..idx].to_string();
                                        } else {
                                            path = String::new();
                                        }
                                        if path.is_empty() && remote.is_empty() {
                                            path = String::new();
                                            remote = String::new();
                                        }
                                    } else {
                                        path = String::new();
                                        remote = String::new();
                                    }
                                    selected_idx = 0;
                                    scroll_offset = 0;
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::TuiExplorerSelector {
                                        archive_path, remote, path, items: Vec::new(), selected_idx, scroll_offset, loading: true
                                    };
                                    self.refresh_tui_selector_list(tx.clone());
                                } else if selected.is_dir {
                                    if path == "/" {
                                        if remote.is_empty() {
                                            path = format!("/{}", selected.name);
                                        } else {
                                            path = selected.name;
                                        }
                                    } else if path.is_empty() {
                                        path = selected.name;
                                    } else {
                                        path = format!("{}/{}", path, selected.name);
                                    }
                                    selected_idx = 0;
                                    scroll_offset = 0;
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::TuiExplorerSelector {
                                        archive_path, remote, path, items: Vec::new(), selected_idx, scroll_offset, loading: true
                                    };
                                    self.refresh_tui_selector_list(tx.clone());
                                }
                            }
                        }
                        KeyCode::Insert => {
                            let dest_path = if remote.is_empty() {
                                path.clone()
                            } else {
                                let clean_remote = remote.trim_end_matches(':');
                                let clean_path = if path.starts_with('/') {
                                    path.clone()
                                } else {
                                    format!("/{}", path)
                                };
                                format!("{}:{}", clean_remote, clean_path)
                            };
                            if !dest_path.is_empty() {
                                self.execute_archive_decompress(archive_path, dest_path, tx.clone()).await;
                            }
                        }
                        _ => {}
                    }
                }
                ui::explorer::ExplorerPopup::SpecialActionMessage { .. } => {
                    if key.code == KeyCode::Esc || key.code == KeyCode::Enter {
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                    }
                }
                ui::explorer::ExplorerPopup::InputPasteRename { mut input_buffer } => {
                    let mut cursor = self.explorer_state.edit_cursor_idx;
                    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                        self.explorer_state.edit_cursor_idx = cursor;
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::InputPasteRename { input_buffer };
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            }
                            KeyCode::Enter => {
                                let new_name = input_buffer.trim().to_string();
                                if !new_name.is_empty() {
                                    if let Some(ref clipboard_item) = self.explorer_state.clipboard {
                                        let src = if clipboard_item.remote.is_empty() {
                                            PathBuf::from(&clipboard_item.path)
                                                .join(&clipboard_item.name)
                                                .to_string_lossy()
                                                .to_string()
                                        } else {
                                            format!("{}:{}/{}", clipboard_item.remote.trim_end_matches(':'), clipboard_item.path.trim_start_matches('/'), clipboard_item.name)
                                        };

                                        let dest_pane = self.explorer_state.get_active_pane();
                                        let dest = if dest_pane.remote.is_empty() {
                                            PathBuf::from(&dest_pane.path)
                                                .join(&new_name)
                                                .to_string_lossy()
                                                .to_string()
                                        } else {
                                            format!("{}:{}/{}", dest_pane.remote.trim_end_matches(':'), dest_pane.path.trim_start_matches('/'), new_name)
                                        };

                                        let is_dir = clipboard_item.is_dir;
                                        self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyModeSelect {
                                            src,
                                            dest,
                                            is_dir,
                                            is_multi: false,
                                            clipboard_items: None,
                                            action_type: "copy".to_string(),
                                            selected_idx: 0,
                                        };
                                    } else {
                                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                ui::explorer::ExplorerPopup::InputSharedLink { mut input_buffer } => {
                    let mut cursor = self.explorer_state.edit_cursor_idx;
                    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                        self.explorer_state.edit_cursor_idx = cursor;
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::InputSharedLink { input_buffer };
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            }
                            KeyCode::Enter => {
                                let link = input_buffer.trim().to_string();
                                if !link.is_empty() {
                                    let mut folder_id = link.clone();
                                    if link.contains('/') {
                                        if let Some(pos) = link.find("folders/") {
                                            let sub = &link[pos + 8..];
                                            folder_id = sub.split('?').next().unwrap_or(sub).to_string();
                                        } else if let Some(pos) = link.find("id=") {
                                            let sub = &link[pos + 3..];
                                            folder_id = sub.split('&').next().unwrap_or(sub).to_string();
                                        } else if let Some(pos) = link.find("/d/") {
                                            let sub = &link[pos + 3..];
                                            folder_id = sub.split('/').next().unwrap_or(sub).to_string();
                                        }
                                    }

                                    let mut drive_remotes = Vec::new();
                                    for remote in &self.connection_state.remotes {
                                        if let Some(r_type) = self.remote_types.get(remote) {
                                            if r_type == "drive" {
                                                drive_remotes.push(remote.clone());
                                            }
                                        }
                                    }

                                    if drive_remotes.is_empty() {
                                        self.explorer_state.notification = Some(("CẢNH BÁO".to_string(), "Không tìm thấy remote Google Drive nào được cấu hình trong hệ thống làm base credentials!".to_string()));
                                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                                    } else {
                                        self.explorer_state.popup = ui::explorer::ExplorerPopup::SelectBaseRemote {
                                            remotes: drive_remotes,
                                            selected_idx: 0,
                                            folder_id,
                                        };
                                    }
                                } else {
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                ui::explorer::ExplorerPopup::SelectBaseRemote {
                    remotes,
                    mut selected_idx,
                    folder_id,
                } => match key.code {
                    KeyCode::Esc => {
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = remotes.len() - 1;
                        } else {
                            selected_idx -= 1;
                        }
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::SelectBaseRemote {
                            remotes,
                            selected_idx,
                            folder_id,
                        };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % remotes.len();
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::SelectBaseRemote {
                            remotes,
                            selected_idx,
                            folder_id,
                        };
                    }
                    KeyCode::Enter => {
                        let base_remote = remotes[selected_idx].clone();
                        let active_pane = self.explorer_state.get_active_pane_mut();
                        active_pane.remote = format!("{},root_folder_id={}:", base_remote.trim_end_matches(':'), folder_id);
                        active_pane.path = String::new();
                        active_pane.items.clear();
                        active_pane.selected_idx = 0;
                        active_pane.scroll_offset = 0;
                        active_pane.selected_names.clear();
                        active_pane.shift_anchor = None;
                        active_pane.shift_active = false;
                        active_pane.alt_anchor = None;
                        active_pane.alt_active = false;
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;

                        let p_type = self.explorer_state.active_pane.clone();
                        self.refresh_explorer_pane(p_type, tx.clone()).await;
                    }
                    _ => {}
                }
                ui::explorer::ExplorerPopup::DedupeModeSelect {
                    mut by_hash,
                    mut selected_idx,
                } => match key.code {
                    KeyCode::Esc => {
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                    }
                    KeyCode::Up => {
                        selected_idx = if selected_idx == 0 { 6 } else { selected_idx - 1 };
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::DedupeModeSelect { by_hash, selected_idx };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % 7;
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::DedupeModeSelect { by_hash, selected_idx };
                    }
                    KeyCode::Char(' ') => {
                        by_hash = !by_hash;
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::DedupeModeSelect { by_hash, selected_idx };
                    }
                    KeyCode::Enter => {
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        let mode_str = match selected_idx {
                            0 => "rename",
                            1 => "newest",
                            2 => "oldest",
                            3 => "largest",
                            4 => "smallest",
                            5 => "first",
                            _ => "skip",
                        }.to_string();
                        self.execute_dedupe(mode_str, by_hash, tx.clone()).await;
                    }
                    _ => {}
                }
                ui::explorer::ExplorerPopup::MergeSimilarDestinationSelect { folders, mut selected_idx } => match key.code {
                    KeyCode::Esc => {
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                    }
                    KeyCode::Up => {
                        if !folders.is_empty() {
                            if selected_idx == 0 {
                                selected_idx = folders.len() - 1;
                            } else {
                                selected_idx -= 1;
                            }
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::MergeSimilarDestinationSelect { folders, selected_idx };
                        }
                    }
                    KeyCode::Down => {
                        if !folders.is_empty() {
                            selected_idx = (selected_idx + 1) % folders.len();
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::MergeSimilarDestinationSelect { folders, selected_idx };
                        }
                    }
                    KeyCode::Enter => {
                        if !folders.is_empty() {
                            let folders_count = folders.len();
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::MergeSimilarScanning { folders_count, scanned_count: 0 };
                            self.execute_merge_similar_scan(folders, selected_idx, tx.clone()).await;
                        }
                    }
                    _ => {}
                }
                ui::explorer::ExplorerPopup::MergeSimilarScanning { .. } => match key.code {
                    KeyCode::Esc => {
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                    }
                    _ => {}
                }
                ui::explorer::ExplorerPopup::MergeSimilarPreview {
                    summary_report,
                    tree_root,
                    mut expanded_paths,
                    mut selected_rel_path,
                    mut scroll_offset,
                    folders,
                    destination_idx,
                } => {
                    let term_h = crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                    let popup_h = term_h * 75 / 100;
                    let list_h = popup_h.saturating_sub(4);

                    fn find_node_by_path<'a>(node: &'a ui::explorer::TreeNode, path: &str) -> Option<&'a ui::explorer::TreeNode> {
                        if node.rel_path == path {
                            return Some(node);
                        }
                        for (_, child) in &node.children {
                            if let Some(n) = find_node_by_path(child, path) {
                                return Some(n);
                            }
                        }
                        None
                    }

                    let mut tree_lines = Vec::new();
                    ui::explorer::flatten_tree(
                        &tree_root,
                        "",
                        true,
                        true,
                        &expanded_paths,
                        &selected_rel_path,
                        &mut tree_lines,
                    );

                    let current_idx = tree_lines.iter().position(|(_, r, _)| r == &selected_rel_path).unwrap_or(0);

                    match key.code {
                        KeyCode::Esc => {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            return;
                        }
                        KeyCode::Up => {
                            if current_idx > 0 {
                                selected_rel_path = tree_lines[current_idx - 1].1.clone();
                            }
                        }
                        KeyCode::Down => {
                            if current_idx + 1 < tree_lines.len() {
                                selected_rel_path = tree_lines[current_idx + 1].1.clone();
                            }
                        }
                        KeyCode::Right => {
                            if let Some(node) = find_node_by_path(&tree_root, &selected_rel_path) {
                                if node.is_dir {
                                    if expanded_paths.contains(&selected_rel_path) {
                                        if let Some(first_child) = node.children.values().next() {
                                            selected_rel_path = first_child.rel_path.clone();
                                        }
                                    } else {
                                        expanded_paths.insert(selected_rel_path.clone());
                                    }
                                }
                            }
                        }
                        KeyCode::Left => {
                            if let Some(node) = find_node_by_path(&tree_root, &selected_rel_path) {
                                if node.is_dir && expanded_paths.contains(&selected_rel_path) {
                                    expanded_paths.remove(&selected_rel_path);
                                } else {
                                    if let Some(idx) = selected_rel_path.rfind('/') {
                                        selected_rel_path = selected_rel_path[..idx].to_string();
                                    } else {
                                        selected_rel_path = "".to_string();
                                    }
                                }
                            }
                        }
                        KeyCode::Char(' ') => {
                            if let Some(node) = find_node_by_path(&tree_root, &selected_rel_path) {
                                if node.is_dir {
                                    if expanded_paths.contains(&selected_rel_path) {
                                        expanded_paths.remove(&selected_rel_path);
                                    } else {
                                        expanded_paths.insert(selected_rel_path.clone());
                                    }
                                }
                            }
                        }
                        KeyCode::Enter => {
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                            self.execute_merge_similar(folders, destination_idx, tx.clone()).await;
                            return;
                        }
                        _ => {}
                    }

                    let mut new_tree_lines = Vec::new();
                    ui::explorer::flatten_tree(
                        &tree_root,
                        "",
                        true,
                        true,
                        &expanded_paths,
                        &selected_rel_path,
                        &mut new_tree_lines,
                    );
                    let new_idx = new_tree_lines.iter().position(|(_, r, _)| r == &selected_rel_path).unwrap_or(0);
                    let combined_idx = summary_report.len() + 1 + new_idx;
                    let total_len = summary_report.len() + 1 + new_tree_lines.len();
                    scroll_offset = ui::update_scroll_offset(combined_idx, scroll_offset, list_h, total_len);

                    self.explorer_state.popup = ui::explorer::ExplorerPopup::MergeSimilarPreview {
                        summary_report,
                        tree_root,
                        expanded_paths,
                        selected_rel_path,
                        scroll_offset,
                        folders,
                        destination_idx,
                    };
                }
                ui::explorer::ExplorerPopup::CopyModeSelect {
                    src,
                    dest,
                    is_dir,
                    is_multi,
                    clipboard_items,
                    action_type,
                    mut selected_idx,
                } => match key.code {
                    KeyCode::Esc => {
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                    }
                    KeyCode::Up => {
                        selected_idx = if selected_idx == 0 { 1 } else { selected_idx - 1 };
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyModeSelect {
                            src, dest, is_dir, is_multi, clipboard_items, action_type, selected_idx
                        };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % 2;
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyModeSelect {
                            src, dest, is_dir, is_multi, clipboard_items, action_type, selected_idx
                        };
                    }
                    KeyCode::Enter => {
                        let use_checksum = selected_idx == 1;
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::None;
                        if action_type == "sync" {
                            tokio::spawn(async move {
                                let mut param = json!({
                                    "srcFs": src,
                                    "dstFs": dest,
                                });
                                if use_checksum {
                                    if let Some(obj) = param.as_object_mut() {
                                        obj.insert("_config".to_string(), json!({ "checksum": true }));
                                    }
                                }
                                let _ = run_rpc_job_async("sync/sync".to_string(), param).await;
                            });
                        } else {
                            if is_multi {
                                if let Some(items) = clipboard_items {
                                    let dest_full = dest.clone();
                                    self.explorer_state.popup = ui::explorer::ExplorerPopup::PermissionScanning {
                                        src: src.clone(),
                                        dest: dest_full.clone(),
                                        is_dir: true,
                                        scanned_count: 0,
                                        total_files: 0,
                                        restricted_count: 0,
                                        items: Some(items.clone()),
                                        use_checksum,
                                    };

                                    let op_id = format!("copy_multi_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                                    let op = crate::app::ActiveOperation {
                                        id: op_id.clone(),
                                        action_type: "copy".to_string(),
                                        src: if items.is_empty() {
                                            String::new()
                                        } else {
                                            let item = &items[0];
                                            if item.remote.is_empty() {
                                                item.path.clone()
                                            } else {
                                                format!("{}:{}", item.remote.trim_end_matches(':'), item.path)
                                            }
                                        },
                                        dest: dest_full.clone(),
                                        items: items.iter().map(|item| item.name.clone()).collect(),
                                        is_dir: true,
                                        use_checksum,
                                        is_copy: true,
                                        completed_items: Some(Vec::new()),
                                        tasks: Some(Vec::new()),
                                    };
                                    crate::app::save_active_operation(&op);

                                    let src_path = op.src.clone();
                                    let skip_flag = self.skip_permission_precheck.clone();
                                    let tx_check = tx.clone();
                                    let items_clone = items.clone();

                                    tokio::spawn(async move {
                                        crate::app::operations::start_async_checker_and_transfer(
                                            op_id,
                                            src_path,
                                            dest_full,
                                            true,
                                            use_checksum,
                                            true,
                                            Some(items_clone),
                                            skip_flag,
                                            tx_check,
                                        ).await;
                                    });
                                }
                            } else {
                                self.check_features_and_execute("copy", src, dest, is_dir, use_checksum, tx.clone());
                            }
                        }
                    }
                    _ => {}
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Esc => {
                self.screen = Screen::MainMenu;
            }
            KeyCode::Tab => {
                self.explorer_state.toggle_pane();
            }
            KeyCode::Up => {
                self.explorer_state.get_active_pane_mut().prev();
            }
            KeyCode::Down => {
                self.explorer_state.get_active_pane_mut().next();
            }
            KeyCode::Enter => {
                // Vào thư mục con hoặc trở về thư mục cha nếu là ".."
                let active_pane = self.explorer_state.get_active_pane_mut();
                if !active_pane.items.is_empty() {
                    let selected = active_pane.items[active_pane.selected_idx].clone();
                    if selected.name == ".." {
                        let current_path = &active_pane.path;
                        if !current_path.is_empty() && current_path != "/" {
                            let parent = if let Some(idx) = current_path.rfind('/') {
                                current_path[..idx].to_string()
                            } else {
                                String::new()
                            };

                            active_pane.path = if parent.is_empty() && active_pane.remote.is_empty()
                            {
                                "/".to_string()
                            } else {
                                parent
                            };

                            active_pane.items.clear();
                            active_pane.selected_idx = 0;
                            active_pane.scroll_offset = 0;
                            active_pane.selected_names.clear();
                            active_pane.shift_anchor = None;
                            active_pane.shift_active = false;
                            active_pane.alt_anchor = None;
                            active_pane.alt_active = false;
                            let p_type = self.explorer_state.active_pane.clone();
                            self.refresh_explorer_pane(p_type, tx.clone()).await;
                        }
                    } else if selected.is_dir {
                        if active_pane.path == "/" {
                            if active_pane.remote.is_empty() {
                                active_pane.path = format!("/{}", selected.name);
                            } else {
                                active_pane.path = selected.name;
                            }
                        } else if active_pane.path.is_empty() {
                            active_pane.path = selected.name;
                        } else {
                            active_pane.path = format!("{}/{}", active_pane.path, selected.name);
                        }
                        active_pane.items.clear();
                        active_pane.selected_idx = 0;
                        active_pane.scroll_offset = 0;
                        active_pane.selected_names.clear();
                        active_pane.shift_anchor = None;
                        active_pane.shift_active = false;
                        active_pane.alt_anchor = None;
                        active_pane.alt_active = false;
                        let p_type = self.explorer_state.active_pane.clone();
                        self.refresh_explorer_pane(p_type, tx.clone()).await;
                    } else {
                        // Quick file preview (cat)
                        let name_lower = selected.name.to_lowercase();
                        let is_text = name_lower.ends_with(".txt")
                            || name_lower.ends_with(".log")
                            || name_lower.ends_with(".conf")
                            || name_lower.ends_with(".config")
                            || name_lower.ends_with(".json")
                            || name_lower.ends_with(".yaml")
                            || name_lower.ends_with(".yml")
                            || name_lower.ends_with(".toml")
                            || name_lower.ends_with(".sh")
                            || name_lower.ends_with(".py")
                            || name_lower.ends_with(".rs")
                            || name_lower.ends_with(".js")
                            || name_lower.ends_with(".ts")
                            || name_lower.ends_with(".md")
                            || name_lower.ends_with(".html")
                            || name_lower.ends_with(".css")
                            || name_lower.ends_with(".go")
                            || name_lower.ends_with(".c")
                            || name_lower.ends_with(".h")
                            || name_lower.ends_with(".cpp")
                            || name_lower.ends_with(".java")
                            || name_lower.ends_with(".properties")
                            || name_lower.ends_with(".env")
                            || name_lower.ends_with(".xml");
                        
                        if is_text && selected.size <= 2_000_000 {
                            let tx_clone = tx.clone();
                            let remote = active_pane.remote.clone();
                            let path = active_pane.path.clone();
                            let file_name = selected.name.clone();
                            
                            self.explorer_state.popup = ui::explorer::ExplorerPopup::SpecialActionMessage {
                                title: "Đang tải file...".to_string(),
                                message: "Vui lòng chờ...".to_string(),
                            };

                            tokio::spawn(async move {
                                let (parent_fs, filename) = if remote.is_empty() {
                                    (path.clone(), file_name.clone())
                                } else {
                                    let clean_remote = remote.trim_end_matches(':');
                                    let clean_path = if path.starts_with('/') {
                                        path.clone()
                                    } else {
                                        format!("/{}", path)
                                    };
                                    (format!("{}:{}", clean_remote, clean_path), file_name.clone())
                                };
                                let param = json!({
                                    "fs": parent_fs,
                                    "remote": filename,
                                }).to_string();
                                
                                let res = rclone::rpc_async("operations/cat".to_string(), param).await;
                                match res {
                                    Ok(rpc_res) if rpc_res.status == 200 => {
                                        let lines: Vec<String> = rpc_res.output.lines().map(|s| s.to_string()).collect();
                                        let _ = tx_clone.send(AppEvent::FileViewLoaded {
                                            file_name,
                                            result: Ok(lines),
                                        });
                                    }
                                    Ok(rpc_res) => {
                                        let _ = tx_clone.send(AppEvent::FileViewLoaded {
                                            file_name,
                                            result: Err(format!("Status {}: {}", rpc_res.status, rpc_res.output)),
                                        });
                                    }
                                    Err(e) => {
                                        let _ = tx_clone.send(AppEvent::FileViewLoaded {
                                            file_name,
                                            result: Err(e),
                                        });
                                    }
                                }
                            });
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                // Quay lại thư mục cha
                let active_pane = self.explorer_state.get_active_pane_mut();
                let current_path = &active_pane.path;
                if !current_path.is_empty() && current_path != "/" {
                    let parent = if let Some(idx) = current_path.rfind('/') {
                        current_path[..idx].to_string()
                    } else {
                        String::new()
                    };

                    active_pane.path = if parent.is_empty() && active_pane.remote.is_empty() {
                        "/".to_string()
                    } else {
                        parent
                    };

                    active_pane.items.clear();
                    active_pane.selected_idx = 0;
                    active_pane.scroll_offset = 0;
                    active_pane.selected_names.clear();
                    active_pane.shift_anchor = None;
                    active_pane.shift_active = false;
                    active_pane.alt_anchor = None;
                    active_pane.alt_active = false;
                    let p_type = self.explorer_state.active_pane.clone();
                    self.refresh_explorer_pane(p_type, tx.clone()).await;
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') if key.modifiers.contains(KeyModifiers::ALT) || (cfg!(target_os = "macos") && key.modifiers.contains(KeyModifiers::CONTROL)) => {
                // Ctrl+R: Mở danh sách remote để lựa chọn cho pane hiện tại
                let mut remotes = vec!["[Local System]".to_string()];
                remotes.extend(self.connection_state.remotes.clone());
                remotes.push(crate::lang::translate("exp_add_shared_link_option"));
                self.explorer_state.popup = ui::explorer::ExplorerPopup::SelectRemote {
                    remotes,
                    selected_idx: 0,
                };
            }
            KeyCode::Char('n') | KeyCode::Char('N') if key.modifiers.contains(KeyModifiers::ALT) || (cfg!(target_os = "macos") && key.modifiers.contains(KeyModifiers::CONTROL)) => {
                self.explorer_state.edit_cursor_idx = 0;
                self.explorer_state.popup = ui::explorer::ExplorerPopup::InputNewFolder {
                    input_buffer: String::new(),
                };
            }
            KeyCode::Char('y') | KeyCode::Char('Y') if key.modifiers.contains(KeyModifiers::ALT) || (cfg!(target_os = "macos") && key.modifiers.contains(KeyModifiers::CONTROL)) => {
                // Ctrl+Y: Đổi tên tệp/thư mục
                let (name, is_dir) = {
                    let pane = self.explorer_state.get_active_pane();
                    if !pane.items.is_empty() {
                        let item = &pane.items[pane.selected_idx];
                        if item.name != ".." {
                            (Some(item.name.clone()), Some(item.is_dir))
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    }
                };
                if let (Some(name), Some(is_dir)) = (name, is_dir) {
                    self.explorer_state.edit_cursor_idx = name.chars().count();
                    self.explorer_state.popup = ui::explorer::ExplorerPopup::InputRename {
                        old_name: name.clone(),
                        input_buffer: name,
                        is_dir,
                    };
                }
            }
            KeyCode::Delete => {
                let pane = self.explorer_state.get_active_pane();
                if !pane.items.is_empty() {
                    // Nếu có nhiều mục được chọn, xóa tất cả
                    if !pane.selected_names.is_empty() {
                        let names: Vec<String> = pane.selected_names.iter().cloned().collect();
                        self.delete_confirm = Some(DeleteTarget::FileExplorerMultiple(names));
                    } else {
                        let item_name = pane.items[pane.selected_idx].name.clone();
                        if item_name != ".." {
                            self.delete_confirm = Some(DeleteTarget::FileExplorer(item_name));
                        }
                    }
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+A: Chọn tất cả (ngoại trừ "..")
                let pane = self.explorer_state.get_active_pane_mut();
                if pane.selected_names.len() == pane.items.iter().filter(|i| i.name != "..").count() {
                    // Nếu đã chọn hết thì bỏ chọn tất cả
                    pane.selected_names.clear();
                } else {
                    pane.selected_names.clear();
                    for item in &pane.items {
                        if item.name != ".." {
                            pane.selected_names.insert(item.name.clone());
                        }
                    }
                }
            }

            KeyCode::Char('V') | KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::SHIFT) && !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) => {
                // Shift+V: Chọn theo vùng dạng bôi đen/loại bỏ (range selection toggle)
                let pane = self.explorer_state.get_active_pane_mut();
                if let Some(anchor) = pane.shift_anchor {
                    if anchor == pane.selected_idx {
                        // Lần nhấn thứ 3 hoặc nhấn tại chỗ: Hủy bỏ neo
                        pane.shift_anchor = None;
                        pane.shift_active = false;
                    } else if !pane.shift_active {
                        // Lần nhấn thứ 2: Toggle từ anchor đến vị trí hiện tại
                        let start = anchor.min(pane.selected_idx);
                        let end = anchor.max(pane.selected_idx);
                        for i in start..=end {
                            if i < pane.items.len() && pane.items[i].name != ".." {
                                let name = pane.items[i].name.clone();
                                if pane.selected_names.contains(&name) {
                                    pane.selected_names.remove(&name);
                                } else {
                                    pane.selected_names.insert(name);
                                }
                            }
                        }
                        pane.shift_active = true;
                    } else {
                        // Lần nhấn thứ 3: Hủy bỏ neo
                        pane.shift_anchor = None;
                        pane.shift_active = false;
                    }
                } else {
                    // Lần nhấn thứ 1: Đặt anchor
                    pane.shift_anchor = Some(pane.selected_idx);
                    pane.shift_active = false;
                }
            }
            KeyCode::Char('V') | KeyCode::Char('v') | KeyCode::Char('d') | KeyCode::Char('D')
                if ((key.code == KeyCode::Char('V') || key.code == KeyCode::Char('v'))
                    && key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::SHIFT))
                    || ((key.code == KeyCode::Char('d') || key.code == KeyCode::Char('D'))
                        && cfg!(target_os = "macos")
                        && key.modifiers.contains(KeyModifiers::CONTROL)) => {
                // Alt+V: Toggle chọn 1 mục đơn lẻ (không di chuyển dòng)
                let pane = self.explorer_state.get_active_pane_mut();
                if !pane.items.is_empty() {
                    let idx = pane.selected_idx;
                    let name = pane.items[idx].name.clone();
                    if name != ".." {
                        if pane.selected_names.contains(&name) {
                            pane.selected_names.remove(&name);
                        } else {
                            pane.selected_names.insert(name);
                        }
                    }
                }
            }
            KeyCode::Char('c') | KeyCode::Char('C') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+C: Sao chép vào clipboard nội bộ (hỗ trợ multi-select)
                let pane = self.explorer_state.get_active_pane();
                if !pane.items.is_empty() {
                    if !pane.selected_names.is_empty() {
                        // Multi-select: sao chép tất cả các mục đã chọn
                        let items: Vec<ui::explorer::ClipboardItem> = pane.items.iter()
                            .filter(|item| pane.selected_names.contains(&item.name))
                            .map(|item| ui::explorer::ClipboardItem {
                                remote: pane.remote.clone(),
                                path: pane.path.clone(),
                                name: item.name.clone(),
                                is_dir: item.is_dir,
                            })
                            .collect();
                        self.explorer_state.clipboard_items = Some(items);
                        self.explorer_state.clipboard = None;
                    } else {
                        // Single select
                        let item = &pane.items[pane.selected_idx];
                        if item.name != ".." {
                            self.explorer_state.clipboard = Some(ui::explorer::ClipboardItem {
                                remote: pane.remote.clone(),
                                path: pane.path.clone(),
                                name: item.name.clone(),
                                is_dir: item.is_dir,
                            });
                            self.explorer_state.clipboard_items = None;
                        }
                    }
                }
            }
            KeyCode::Char('o') | KeyCode::Char('O') if key.modifiers.contains(KeyModifiers::ALT) || (cfg!(target_os = "macos") && key.modifiers.contains(KeyModifiers::CONTROL)) => {
                let pane = self.explorer_state.get_active_pane();
                if !pane.items.is_empty() {
                    let selected = &pane.items[pane.selected_idx];
                    if selected.name != ".." {
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::SpecialActionsMenu {
                            selected_idx: 0,
                        };
                    }
                }
            }
            KeyCode::Char('v') | KeyCode::Char('V') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+V: Dán (hỗ trợ multi-select)
                if let Some(ref items) = self.explorer_state.clipboard_items {
                    if !items.is_empty() {
                        let dest_pane = self.explorer_state.get_active_pane();
                        let dest_remote = dest_pane.remote.clone();
                        let dest_path = dest_pane.path.clone();
                        let dest = if dest_remote.is_empty() { dest_path.clone() } else { format!("{}:{}", dest_remote, dest_path) };

                        self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyModeSelect {
                            src: format!("({} mục)", items.len()),
                            dest,
                            is_dir: true,
                            is_multi: true,
                            clipboard_items: Some(items.clone()),
                            action_type: "copy".to_string(),
                            selected_idx: 0,
                        };
                    }
                } else if let Some(ref clipboard_item) = self.explorer_state.clipboard {
                    self.explorer_state.edit_cursor_idx = clipboard_item.name.chars().count();
                    self.explorer_state.popup = ui::explorer::ExplorerPopup::InputPasteRename {
                        input_buffer: clipboard_item.name.clone(),
                    };
                }
            }
            KeyCode::Char('x') | KeyCode::Char('X') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+X: Di chuyển sang pane đối diện (hỗ trợ multi-select)
                let src_pane = self.explorer_state.get_active_pane();
                if !src_pane.items.is_empty() {
                    if !src_pane.selected_names.is_empty() {
                        // Multi-move: Di chuyển tất cả mục đã chọn
                        let items: Vec<(String, bool)> = src_pane.items.iter()
                            .filter(|item| src_pane.selected_names.contains(&item.name))
                            .map(|item| (item.name.clone(), item.is_dir))
                            .collect();
                        let src_remote = src_pane.remote.clone();
                        let src_path = src_pane.path.clone();

                        let dest_pane = match self.explorer_state.active_pane {
                            ui::explorer::ActivePane::Left => &self.explorer_state.right_pane,
                            ui::explorer::ActivePane::Right => &self.explorer_state.left_pane,
                        };
                        let dest_remote = dest_pane.remote.clone();
                        let dest_path = dest_pane.path.clone();
                        let pane_type = self.explorer_state.active_pane.clone();
                        let tx_op = tx.clone();

                        let dest_full = if dest_remote.is_empty() { dest_path.clone() } else { format!("{}:{}", dest_remote, dest_path) };
                        self.explorer_state.popup = ui::explorer::ExplorerPopup::MoveProgress {
                            src: format!("({} mục)", items.len()),
                            dest: dest_full.clone(),
                            pct: 0.0,
                            job_id: None,
                        };

                        // Xoá selection sau khi bắt đầu di chuyển
                        self.explorer_state.get_active_pane_mut().selected_names.clear();

                        let op_id = format!("multi_move_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                        let op = crate::app::ActiveOperation {
                            id: op_id.clone(),
                            action_type: "move".to_string(),
                            src: if src_remote.is_empty() { src_path.clone() } else { format!("{}:{}", src_remote.trim_end_matches(':'), src_path) },
                            dest: dest_full.clone(),
                            items: items.iter().map(|(item_name, _)| item_name.clone()).collect(),
                            is_dir: true,
                            use_checksum: false,
                            is_copy: false,
                            completed_items: Some(Vec::new()),
                            tasks: None,
                        };
                        crate::app::save_active_operation(&op);

                        tokio::spawn(async move {
                            let total = items.len();
                            let mut last_err = None;
                            for (idx, (item_name, is_dir)) in items.iter().enumerate() {
                                let src = if src_remote.is_empty() {
                                    PathBuf::from(&src_path)
                                        .join(item_name)
                                        .to_string_lossy()
                                        .to_string()
                                } else {
                                    format!("{}:{}/{}", src_remote.trim_end_matches(':'), src_path.trim_start_matches('/'), item_name)
                                };
                                let dest = if dest_remote.is_empty() {
                                    PathBuf::from(&dest_path)
                                        .join(item_name)
                                        .to_string_lossy()
                                        .to_string()
                                } else {
                                    format!("{}:{}/{}", dest_remote.trim_end_matches(':'), dest_path.trim_start_matches('/'), item_name)
                                };

                                let pct = ((idx as f64) / total as f64) * 100.0;
                                let _ = tx_op.send(AppEvent::MoveProgress {
                                    src: format!("({}/{}) {}", idx + 1, total, item_name),
                                    dest: dest.clone(),
                                    pct,
                                    job_id: None,
                                });

                                let method = if *is_dir {
                                    "sync/move"
                                } else {
                                    "operations/movefile"
                                };
                                let param = if *is_dir {
                                    json!({ "srcFs": src, "dstFs": dest })
                                } else {
                                    json!({ "srcFs": src.rsplit_once('/').map(|(p,_)| p).unwrap_or(&src), "srcRemote": item_name, "dstFs": dest.rsplit_once('/').map(|(p,_)| p).unwrap_or(&dest), "dstRemote": item_name })
                                };

                                let res = run_rpc_job_async(method.to_string(), param).await;
                                crate::app::complete_item_in_active_operation(&op_id, item_name);
                                if let Err(e) = res {
                                    last_err = Some(e);
                                }
                            }
                            let _ = tx_op.send(AppEvent::MoveProgress {
                                src: format!("({} mục)", total),
                                dest: String::new(),
                                pct: 100.0,
                                job_id: None,
                            });
                            let result = match last_err {
                                None => Ok(()),
                                Some(e) => Err(e),
                            };
                            let _ = tx_op.send(AppEvent::ExplorerOperationFinished {
                                pane: pane_type,
                                op_name: "di chuyển nhiều mục".to_string(),
                                result,
                            });
                        });
                    } else {
                        let item = &src_pane.items[src_pane.selected_idx];
                        if item.name == ".." {
                            return;
                        }
                        let is_dir = item.is_dir;
                        let src = if src_pane.remote.is_empty() {
                            PathBuf::from(&src_pane.path)
                                .join(&item.name)
                                .to_string_lossy()
                                .to_string()
                        } else {
                            format!("{}:{}/{}", src_pane.remote.trim_end_matches(':'), src_pane.path.trim_start_matches('/'), item.name)
                        };

                        let dest_pane = match self.explorer_state.active_pane {
                            ui::explorer::ActivePane::Left => &self.explorer_state.right_pane,
                            ui::explorer::ActivePane::Right => &self.explorer_state.left_pane,
                        };
                        let dest = if dest_pane.remote.is_empty() {
                            PathBuf::from(&dest_pane.path)
                                .join(&item.name)
                                .to_string_lossy()
                                .to_string()
                        } else {
                            format!("{}:{}/{}", dest_pane.remote.trim_end_matches(':'), dest_pane.path.trim_start_matches('/'), item.name)
                        };

                        self.check_features_and_execute("move", src, dest, is_dir, false, tx.clone());
                    }
                }
            }
            KeyCode::Char('t') | KeyCode::Char('T') if key.modifiers.contains(KeyModifiers::ALT) || (cfg!(target_os = "macos") && key.modifiers.contains(KeyModifiers::CONTROL)) => {
                // Alt+T: Đồng bộ
                let src_pane = match self.explorer_state.active_pane {
                    ui::explorer::ActivePane::Left => &self.explorer_state.left_pane,
                    ui::explorer::ActivePane::Right => &self.explorer_state.right_pane,
                };
                let dest_pane = match self.explorer_state.active_pane {
                    ui::explorer::ActivePane::Left => &self.explorer_state.right_pane,
                    ui::explorer::ActivePane::Right => &self.explorer_state.left_pane,
                };

                let src_fs = if src_pane.remote.is_empty() {
                    src_pane.path.clone()
                } else {
                    format!("{}:{}", src_pane.remote.trim_end_matches(':'), src_pane.path)
                };
                let dest_fs = if dest_pane.remote.is_empty() {
                    dest_pane.path.clone()
                } else {
                    format!("{}:{}", dest_pane.remote.trim_end_matches(':'), dest_pane.path)
                };

                self.explorer_state.popup = ui::explorer::ExplorerPopup::CopyModeSelect {
                    src: src_fs,
                    dest: dest_fs,
                    is_dir: true,
                    is_multi: false,
                    clipboard_items: None,
                    action_type: "sync".to_string(),
                    selected_idx: 0,
                };
            }
            KeyCode::Char(' ') => {
                // Space: Xoá clipboard và selection
                self.explorer_state.clipboard = None;
                self.explorer_state.clipboard_items = None;
                let pane = self.explorer_state.get_active_pane_mut();
                pane.selected_names.clear();
                pane.shift_anchor = None;
                pane.shift_active = false;
                pane.alt_anchor = None;
                pane.alt_active = false;
            }
            _ => {}
        }
    }
}

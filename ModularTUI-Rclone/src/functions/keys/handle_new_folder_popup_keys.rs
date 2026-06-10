use crossterm::event::{KeyEvent, KeyCode};
use crate::app::{App, AppEvent};
use crate::functions::*;
use std::path::PathBuf;
use serde_json::json;

pub fn handle_new_folder_popup_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    mut input_buffer: String,
) {
    let mut cursor = app.explorer_state.edit_cursor_idx;
    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
        app.explorer_state.edit_cursor_idx = cursor;
        app.explorer_state.popup = ExplorerPopup::InputNewFolder { input_buffer };
    } else {
        match key.code {
            KeyCode::Esc => {
                app.explorer_state.popup = ExplorerPopup::None;
            }
            KeyCode::Enter => {
                let folder_name = input_buffer.trim().to_string();
                if !folder_name.is_empty() {
                    let pane = app.explorer_state.get_active_pane_mut();
                    let is_local = pane.remote.is_empty();
                    let target = if is_local {
                        PathBuf::from(&pane.path)
                            .join(&folder_name)
                            .to_string_lossy()
                            .to_string()
                    } else {
                        String::new()
                    };

                    if !pane.items.iter().any(|item| item.name == folder_name) {
                        pane.items.push(FileItem {
                            name: folder_name.clone(),
                            size: 0,
                            is_dir: true,
                            mod_time: translate("exp_creating_placeholder"),
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
                    let pane_type = app.explorer_state.active_pane.clone();
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
                            let op_res = rpc_async("operations/mkdir".to_string(), param).await;
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
                    app.explorer_state.popup = ExplorerPopup::None;
                }
            }
            _ => {}
        }
    }
}

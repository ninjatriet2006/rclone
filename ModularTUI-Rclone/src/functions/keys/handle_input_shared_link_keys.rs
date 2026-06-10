use crossterm::event::{KeyEvent, KeyCode};
use crate::app::{App, AppEvent};
use crate::functions::*;

pub async fn handle_input_shared_link_keys(
    app: &mut App,
    key: KeyEvent,
    mut input_buffer: String,
) {
    let mut cursor = app.explorer_state.edit_cursor_idx;
    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
        app.explorer_state.edit_cursor_idx = cursor;
        app.explorer_state.popup = ExplorerPopup::InputSharedLink { input_buffer };
    } else {
        match key.code {
            KeyCode::Esc => {
                app.explorer_state.popup = ExplorerPopup::None;
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
                    for remote in &app.connection_state.remotes {
                        if let Some(r_type) = app.remote_types.get(remote) {
                            if r_type == "drive" {
                                drive_remotes.push(remote.clone());
                            }
                        }
                    }

                    if drive_remotes.is_empty() {
                        app.explorer_state.notification = Some((
                            "CẢNH BÁO".to_string(),
                            "Không tìm thấy remote Google Drive nào được cấu hình trong hệ thống làm base credentials!".to_string(),
                        ));
                        app.explorer_state.popup = ExplorerPopup::None;
                    } else {
                        app.explorer_state.popup = ExplorerPopup::SelectBaseRemote {
                            remotes: drive_remotes,
                            selected_idx: 0,
                            folder_id,
                        };
                    }
                } else {
                    app.explorer_state.popup = ExplorerPopup::None;
                }
            }
            _ => {}
        }
    }
}

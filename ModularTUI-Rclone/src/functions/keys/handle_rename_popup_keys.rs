use crossterm::event::{KeyEvent, KeyCode};
use crate::app::{App, AppEvent};
use crate::functions::*;
use std::path::PathBuf;

pub fn handle_rename_popup_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    old_name: String,
    mut input_buffer: String,
    is_dir: bool,
) {
    let mut cursor = app.explorer_state.edit_cursor_idx;
    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
        app.explorer_state.edit_cursor_idx = cursor;
        app.explorer_state.popup = ExplorerPopup::InputRename {
            old_name,
            input_buffer,
            is_dir,
        };
    } else {
        match key.code {
            KeyCode::Esc => {
                app.explorer_state.popup = ExplorerPopup::None;
            }
            KeyCode::Enter => {
                let new_name = input_buffer.trim().to_string();
                if !new_name.is_empty() && new_name != old_name {
                    let pane = app.explorer_state.get_active_pane();
                    let remote = pane.remote.clone();
                    let parent_path = pane.path.clone();
                    
                    app.explorer_state.popup = ExplorerPopup::None;

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

                    app.check_features_and_execute("rename", src, dest, is_dir, false, tx.clone());
                } else {
                    app.explorer_state.popup = ExplorerPopup::None;
                }
            }
            _ => {}
        }
    }
}

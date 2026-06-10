use crossterm::event::{KeyEvent, KeyCode};
use crate::app::{App, AppEvent};
use crate::functions::*;
use std::path::PathBuf;

pub async fn handle_input_paste_rename_keys(
    app: &mut App,
    key: KeyEvent,
    mut input_buffer: String,
) {
    let mut cursor = app.explorer_state.edit_cursor_idx;
    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
        app.explorer_state.edit_cursor_idx = cursor;
        app.explorer_state.popup = ExplorerPopup::InputPasteRename { input_buffer };
    } else {
        match key.code {
            KeyCode::Esc => {
                app.explorer_state.popup = ExplorerPopup::None;
            }
            KeyCode::Enter => {
                let new_name = input_buffer.trim().to_string();
                if !new_name.is_empty() {
                    if let Some(ref clipboard_item) = app.explorer_state.clipboard {
                        let src = if clipboard_item.remote.is_empty() {
                            PathBuf::from(&clipboard_item.path)
                                .join(&clipboard_item.name)
                                .to_string_lossy()
                                .to_string()
                        } else {
                            format!("{}:{}/{}", clipboard_item.remote.trim_end_matches(':'), clipboard_item.path.trim_start_matches('/'), clipboard_item.name)
                        };

                        let dest_pane = app.explorer_state.get_active_pane();
                        let dest = if dest_pane.remote.is_empty() {
                            PathBuf::from(&dest_pane.path)
                                .join(&new_name)
                                .to_string_lossy()
                                .to_string()
                        } else {
                            format!("{}:{}/{}", dest_pane.remote.trim_end_matches(':'), dest_pane.path.trim_start_matches('/'), new_name)
                        };

                        let is_dir = clipboard_item.is_dir;
                        app.explorer_state.popup = ExplorerPopup::CopyModeSelect {
                            src,
                            dest,
                            is_dir,
                            is_multi: false,
                            clipboard_items: None,
                            action_type: "copy".to_string(),
                            selected_idx: 0,
                        };
                    } else {
                        app.explorer_state.popup = ExplorerPopup::None;
                    }
                }
            }
            _ => {}
        }
    }
}

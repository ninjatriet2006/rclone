use crossterm::event::{KeyEvent, KeyCode};
use crate::app::{App, AppEvent};
use crate::functions::*;

pub async fn handle_cryptdecode_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    mut remote_input: String,
    mut encrypted_input: String,
    mut is_remote_focused: bool,
    output_result: Option<String>,
) {
    let mut cursor = app.explorer_state.edit_cursor_idx;
    let handled = if is_remote_focused {
        handle_input_key(&key, &mut remote_input, &mut cursor)
    } else {
        handle_input_key(&key, &mut encrypted_input, &mut cursor)
    };
    if handled {
        app.explorer_state.edit_cursor_idx = cursor;
        app.explorer_state.popup = ExplorerPopup::CryptdecodeForm { remote_input, encrypted_input, is_remote_focused, output_result };
    } else {
        match key.code {
            KeyCode::Esc => {
                app.explorer_state.popup = ExplorerPopup::None;
            }
            KeyCode::Tab => {
                is_remote_focused = !is_remote_focused;
                app.explorer_state.edit_cursor_idx = if is_remote_focused {
                    remote_input.chars().count()
                } else {
                    encrypted_input.chars().count()
                };
                app.explorer_state.popup = ExplorerPopup::CryptdecodeForm { remote_input, encrypted_input, is_remote_focused, output_result };
            }
            KeyCode::Enter => {
                app.execute_cryptdecode(remote_input.clone(), encrypted_input.clone(), tx.clone()).await;
            }
            _ => {}
        }
    }
}

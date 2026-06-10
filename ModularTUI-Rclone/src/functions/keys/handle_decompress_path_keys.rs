use crossterm::event::{KeyEvent, KeyCode};
use crate::app::{App, AppEvent};
use crate::functions::*;

pub async fn handle_decompress_path_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    archive_path: String,
    mut selected_idx: usize,
) {
    match key.code {
        KeyCode::Esc => {
            app.explorer_state.popup = ExplorerPopup::None;
        }
        KeyCode::Up | KeyCode::Down => {
            selected_idx = (selected_idx + 1) % 2;
            app.explorer_state.popup = ExplorerPopup::DecompressPathInput { archive_path, selected_idx };
        }
        KeyCode::Enter => {
            if selected_idx == 0 {
                let active_pane = app.explorer_state.get_active_pane();
                let initial_path = if active_pane.remote.is_empty() {
                    active_pane.path.clone()
                } else {
                    format!("{}:{}", active_pane.remote.trim_end_matches(':'), active_pane.path)
                };
                app.explorer_state.edit_cursor_idx = initial_path.chars().count();
                app.explorer_state.popup = ExplorerPopup::DecompressPathManualInput {
                    archive_path,
                    input_buffer: initial_path,
                };
            } else {
                app.explorer_state.popup = ExplorerPopup::TuiExplorerSelector {
                    archive_path,
                    remote: String::new(),
                    path: String::new(),
                    items: Vec::new(),
                    selected_idx: 0,
                    scroll_offset: 0,
                    loading: true,
                };
                app.refresh_tui_selector_list(tx.clone());
            }
        }
        _ => {}
    }
}

pub async fn handle_decompress_path_manual_input_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    archive_path: String,
    mut input_buffer: String,
) {
    let mut cursor = app.explorer_state.edit_cursor_idx;
    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
        app.explorer_state.edit_cursor_idx = cursor;
        app.explorer_state.popup = ExplorerPopup::DecompressPathManualInput { archive_path, input_buffer };
    } else {
        match key.code {
            KeyCode::Esc => {
                app.explorer_state.popup = ExplorerPopup::None;
            }
            KeyCode::Enter => {
                let dest_path = input_buffer.trim().to_string();
                if !dest_path.is_empty() {
                    app.execute_archive_decompress(archive_path, dest_path, tx.clone()).await;
                }
            }
            _ => {}
        }
    }
}

use crossterm::event::{KeyEvent, KeyCode};
use crate::app::{App, AppEvent};
use crate::functions::*;

pub async fn handle_decompress_mode_keys(
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
        KeyCode::Up => {
            selected_idx = if selected_idx == 0 { 2 } else { selected_idx - 1 };
            app.explorer_state.popup = ExplorerPopup::DecompressModeSelect { archive_path, selected_idx };
        }
        KeyCode::Down => {
            selected_idx = (selected_idx + 1) % 3;
            app.explorer_state.popup = ExplorerPopup::DecompressModeSelect { archive_path, selected_idx };
        }
        KeyCode::Enter => {
            app.handle_decompress_mode_selected(archive_path, selected_idx, tx.clone()).await;
        }
        _ => {}
    }
}

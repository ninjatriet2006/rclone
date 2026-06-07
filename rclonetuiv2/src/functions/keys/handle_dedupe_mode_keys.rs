use crossterm::event::{KeyEvent, KeyCode};
use crate::app::{App, AppEvent};
use crate::functions::*;

pub async fn handle_dedupe_mode_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    mut by_hash: bool,
    mut selected_idx: usize,
) {
    match key.code {
        KeyCode::Esc => {
            app.explorer_state.popup = ExplorerPopup::None;
        }
        KeyCode::Up => {
            selected_idx = if selected_idx == 0 { 6 } else { selected_idx - 1 };
            app.explorer_state.popup = ExplorerPopup::DedupeModeSelect { by_hash, selected_idx };
        }
        KeyCode::Down => {
            selected_idx = (selected_idx + 1) % 7;
            app.explorer_state.popup = ExplorerPopup::DedupeModeSelect { by_hash, selected_idx };
        }
        KeyCode::Char(' ') => {
            by_hash = !by_hash;
            app.explorer_state.popup = ExplorerPopup::DedupeModeSelect { by_hash, selected_idx };
        }
        KeyCode::Enter => {
            app.explorer_state.popup = ExplorerPopup::None;
            let mode_str = match selected_idx {
                0 => "rename",
                1 => "newest",
                2 => "oldest",
                3 => "largest",
                4 => "smallest",
                5 => "first",
                _ => "skip",
            }.to_string();
            app.execute_dedupe(mode_str, by_hash, tx.clone()).await;
        }
        _ => {}
    }
}

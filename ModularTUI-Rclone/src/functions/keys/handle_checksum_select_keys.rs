use crossterm::event::{KeyEvent, KeyCode};
use crate::app::{App, AppEvent};
use crate::functions::ExplorerPopup;

pub async fn handle_checksum_select_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    mut selected_idx: usize,
) {
    let hash_types = vec!["md5".to_string(), "sha1".to_string(), "sha256".to_string(), "crc32".to_string(), "xxhash".to_string()];
    match key.code {
        KeyCode::Esc => {
            app.explorer_state.popup = ExplorerPopup::None;
        }
        KeyCode::Up => {
            selected_idx = if selected_idx == 0 { hash_types.len() - 1 } else { selected_idx - 1 };
            app.explorer_state.popup = ExplorerPopup::ChecksumTypeSelect { selected_idx };
        }
        KeyCode::Down => {
            selected_idx = (selected_idx + 1) % hash_types.len();
            app.explorer_state.popup = ExplorerPopup::ChecksumTypeSelect { selected_idx };
        }
        KeyCode::Enter => {
            let hash_type = hash_types[selected_idx].clone();
            app.explorer_state.popup = ExplorerPopup::None;
            app.execute_hashsum_file(hash_type, tx.clone()).await;
        }
        _ => {}
    }
}

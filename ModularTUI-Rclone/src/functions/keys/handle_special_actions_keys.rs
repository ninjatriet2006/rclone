use crossterm::event::{KeyEvent, KeyCode};
use crate::app::{App, AppEvent};
use crate::functions::*;

pub async fn handle_special_actions_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    mut selected_idx: usize,
) {
    match key.code {
        KeyCode::Esc => {
            app.explorer_state.popup = ExplorerPopup::None;
        }
        KeyCode::Up => {
            selected_idx = if selected_idx == 0 { 9 } else { selected_idx - 1 };
            app.explorer_state.popup = ExplorerPopup::SpecialActionsMenu { selected_idx };
        }
        KeyCode::Down => {
            selected_idx = (selected_idx + 1) % 10;
            app.explorer_state.popup = ExplorerPopup::SpecialActionsMenu { selected_idx };
        }
        KeyCode::Enter => {
            if selected_idx == 9 {
                app.explorer_state.popup = ExplorerPopup::None;
            } else {
                app.explorer_state.popup = ExplorerPopup::None;
                app.handle_special_action_selected(selected_idx, tx.clone()).await;
            }
        }
        _ => {}
    }
}

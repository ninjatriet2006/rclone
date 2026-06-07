use crossterm::event::{KeyEvent, KeyCode};
use crate::app::{App, AppEvent};
use crate::functions::*;

pub async fn handle_select_base_remote_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    remotes: Vec<String>,
    mut selected_idx: usize,
    folder_id: String,
) {
    match key.code {
        KeyCode::Esc => {
            app.explorer_state.popup = ExplorerPopup::None;
        }
        KeyCode::Up => {
            if selected_idx == 0 {
                selected_idx = remotes.len() - 1;
            } else {
                selected_idx -= 1;
            }
            app.explorer_state.popup = ExplorerPopup::SelectBaseRemote {
                remotes,
                selected_idx,
                folder_id,
            };
        }
        KeyCode::Down => {
            selected_idx = (selected_idx + 1) % remotes.len();
            app.explorer_state.popup = ExplorerPopup::SelectBaseRemote {
                remotes,
                selected_idx,
                folder_id,
            };
        }
        KeyCode::Enter => {
            let base_remote = remotes[selected_idx].clone();
            let active_pane = app.explorer_state.get_active_pane_mut();
            active_pane.remote = format!("{},root_folder_id={}:", base_remote.trim_end_matches(':'), folder_id);
            active_pane.path = String::new();
            active_pane.items.clear();
            active_pane.selected_idx = 0;
            active_pane.scroll_offset = 0;
            active_pane.selected_names.clear();
            active_pane.shift_anchor = None;
            active_pane.shift_active = false;
            active_pane.alt_anchor = None;
            active_pane.alt_active = false;
            app.explorer_state.popup = ExplorerPopup::None;

            let p_type = app.explorer_state.active_pane.clone();
            app.refresh_explorer_pane(p_type, tx.clone()).await;
        }
        _ => {}
    }
}

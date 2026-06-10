use crossterm::event::{KeyEvent, KeyCode};
use crate::app::{App, AppEvent};
use crate::functions::*;

pub async fn handle_select_remote_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    remotes: Vec<String>,
    mut selected_idx: usize,
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
            app.explorer_state.popup = ExplorerPopup::SelectRemote {
                remotes,
                selected_idx,
            };
        }
        KeyCode::Down => {
            selected_idx = (selected_idx + 1) % remotes.len();
            app.explorer_state.popup = ExplorerPopup::SelectRemote {
                remotes,
                selected_idx,
            };
        }
        KeyCode::Enter => {
            let chosen = remotes[selected_idx].clone();
            if chosen == translate("exp_add_shared_link_option") {
                app.explorer_state.edit_cursor_idx = 0;
                app.explorer_state.popup = ExplorerPopup::InputSharedLink {
                    input_buffer: String::new(),
                };
            } else {
                let active_pane = app.explorer_state.get_active_pane_mut();
                if chosen == "[Local System]" {
                    active_pane.remote = String::new();
                    active_pane.path = get_home_dir();
                } else {
                    active_pane.remote = chosen;
                    active_pane.path = String::new();
                }
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
        }
        _ => {}
    }
}

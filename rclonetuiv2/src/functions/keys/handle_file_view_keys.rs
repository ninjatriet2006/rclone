use crossterm::event::{KeyEvent, KeyCode};
use crate::app::App;
use crate::functions::ExplorerPopup;

pub fn handle_file_view_keys(
    app: &mut App,
    key: KeyEvent,
    file_name: String,
    content: Vec<String>,
    mut scroll_offset: usize,
) {
    match key.code {
        KeyCode::Esc => {
            app.explorer_state.popup = ExplorerPopup::None;
        }
        KeyCode::Up => {
            if scroll_offset > 0 {
                scroll_offset -= 1;
                app.explorer_state.popup = ExplorerPopup::ViewFile { file_name, content, scroll_offset };
            }
        }
        KeyCode::Down => {
            if scroll_offset + 1 < content.len() {
                scroll_offset += 1;
                app.explorer_state.popup = ExplorerPopup::ViewFile { file_name, content, scroll_offset };
            }
        }
        _ => {}
    }
}

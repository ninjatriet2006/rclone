use crate::app::{App, Screen};
use crossterm::event::{KeyEvent, KeyCode};

pub async fn handle_language_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up => {
            if !app.available_languages.is_empty() {
                if app.selected_lang_idx == 0 {
                    app.selected_lang_idx = app.available_languages.len() - 1;
                } else {
                    app.selected_lang_idx -= 1;
                }
            }
        }
        KeyCode::Down => {
            if !app.available_languages.is_empty() {
                app.selected_lang_idx =
                    (app.selected_lang_idx + 1) % app.available_languages.len();
            }
        }
        KeyCode::Enter => {
            if let Some(lang) = app
                .available_languages
                .get(app.selected_lang_idx)
                .cloned()
            {
                app.config.active_language = lang.clone();
                let _ = app.config.save();
                crate::functions::load_translation(&lang);
            }
            app.screen = Screen::MainMenu;
        }
        KeyCode::Esc => {
            app.screen = Screen::MainMenu;
        }
        _ => {}
    }
}

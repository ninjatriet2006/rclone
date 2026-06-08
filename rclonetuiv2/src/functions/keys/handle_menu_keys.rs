use crate::app::{App, AppEvent, Screen};
use crossterm::event::{KeyEvent, KeyCode};

pub async fn handle_menu_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
) {
    match key.code {
        KeyCode::Up => app.menu_state.prev(),
        KeyCode::Down => app.menu_state.next(),
        KeyCode::Enter => match app.menu_state.selected_idx {
            0 => {
                app.screen = Screen::ConnectionManager;
                app.load_remotes(tx.clone()).await;
            }
            1 => {
                app.screen = Screen::FileExplorer;
                if app.explorer_state.left_pane.items.is_empty() && app.explorer_state.right_pane.items.is_empty() {
                    app.explorer_state = crate::functions::ExplorerState::new();
                }
                app.load_remotes(tx.clone()).await;
                app.refresh_explorer_pane(crate::functions::ActivePane::Left, tx.clone()).await;
                app.refresh_explorer_pane(crate::functions::ActivePane::Right, tx.clone()).await;
            }
            2 => {
                app.screen = Screen::JobMonitor;
                app.last_stats_scan = std::time::Instant::now() - std::time::Duration::from_secs(10);
            }
            3 => {
                app.screen = Screen::ConfigProfileManager;
                app.load_profile_list();
            }
            4 => {
                app.screen = Screen::ServicesAndMounts;
                app.load_remotes(tx.clone()).await;
                app.scan_running_services();
                app.scan_systemd_services();
                app.last_services_scan = std::time::Instant::now();
            }
            5 => {
                app.screen = Screen::LanguageSelect;
                app.available_languages = crate::functions::get_available_languages();
                app.selected_lang_idx = app
                    .available_languages
                    .iter()
                    .position(|l| l == &app.config.active_language)
                    .unwrap_or(0);
            }
            6 => {
                app.screen = Screen::DependencyManager;
                app.selected_dependency_idx = 0;
            }
            7 => {
                app.should_exit = true;
            }
            _ => {}
        },
        KeyCode::Char('q') | KeyCode::Char('Q') => {
            app.should_exit = true;
        }
        _ => {}
    }
}

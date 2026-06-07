use crossterm::event::{KeyEvent, KeyCode};
use crate::app::{App, AppEvent};
use crate::functions::*;

pub async fn handle_tui_selector_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    archive_path: String,
    mut remote: String,
    mut path: String,
    items: Vec<FileItem>,
    mut selected_idx: usize,
    mut scroll_offset: usize,
    loading: bool,
) {
    if loading {
        if key.code == KeyCode::Esc {
            app.explorer_state.popup = ExplorerPopup::None;
        }
        return;
    }
    match key.code {
        KeyCode::Esc => {
            app.explorer_state.popup = ExplorerPopup::None;
        }
        KeyCode::Up => {
            if !items.is_empty() {
                if selected_idx == 0 {
                    selected_idx = items.len() - 1;
                } else {
                    selected_idx -= 1;
                }
                let term_h = crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                let popup_h = term_h * 70 / 100;
                let list_h = popup_h.saturating_sub(4);
                scroll_offset = update_scroll_offset(selected_idx, scroll_offset, list_h, items.len());
                app.explorer_state.popup = ExplorerPopup::TuiExplorerSelector {
                    archive_path, remote, path, items, selected_idx, scroll_offset, loading
                };
            }
        }
        KeyCode::Down => {
            if !items.is_empty() {
                selected_idx = (selected_idx + 1) % items.len();
                let term_h = crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                let popup_h = term_h * 70 / 100;
                let list_h = popup_h.saturating_sub(4);
                scroll_offset = update_scroll_offset(selected_idx, scroll_offset, list_h, items.len());
                app.explorer_state.popup = ExplorerPopup::TuiExplorerSelector {
                    archive_path, remote, path, items, selected_idx, scroll_offset, loading
                };
            }
        }
        KeyCode::Enter => {
            if !items.is_empty() {
                let selected = items[selected_idx].clone();
                if selected.name == "[Local System]" {
                    remote = String::new();
                    path = "/".to_string();
                    selected_idx = 0;
                    scroll_offset = 0;
                    app.explorer_state.popup = ExplorerPopup::TuiExplorerSelector {
                        archive_path, remote, path, items: Vec::new(), selected_idx, scroll_offset, loading: true
                    };
                    app.refresh_tui_selector_list(tx.clone());
                } else if selected.name.ends_with(':') {
                    remote = selected.name.clone();
                    path = String::new();
                    selected_idx = 0;
                    scroll_offset = 0;
                    app.explorer_state.popup = ExplorerPopup::TuiExplorerSelector {
                        archive_path, remote, path, items: Vec::new(), selected_idx, scroll_offset, loading: true
                    };
                    app.refresh_tui_selector_list(tx.clone());
                } else if selected.name == ".." {
                    if !path.is_empty() && path != "/" {
                        if let Some(idx) = path.rfind('/') {
                            path = path[..idx].to_string();
                        } else {
                            path = String::new();
                        }
                        if path.is_empty() && remote.is_empty() {
                            path = String::new();
                            remote = String::new();
                        }
                    } else {
                        path = String::new();
                        remote = String::new();
                    }
                    selected_idx = 0;
                    scroll_offset = 0;
                    app.explorer_state.popup = ExplorerPopup::TuiExplorerSelector {
                        archive_path, remote, path, items: Vec::new(), selected_idx, scroll_offset, loading: true
                    };
                    app.refresh_tui_selector_list(tx.clone());
                } else if selected.is_dir {
                    if path == "/" {
                        if remote.is_empty() {
                            path = format!("/{}", selected.name);
                        } else {
                            path = selected.name;
                        }
                    } else if path.is_empty() {
                        path = selected.name;
                    } else {
                        path = format!("{}/{}", path, selected.name);
                    }
                    selected_idx = 0;
                    scroll_offset = 0;
                    app.explorer_state.popup = ExplorerPopup::TuiExplorerSelector {
                        archive_path, remote, path, items: Vec::new(), selected_idx, scroll_offset, loading: true
                    };
                    app.refresh_tui_selector_list(tx.clone());
                }
            }
        }
        KeyCode::Insert => {
            let dest_path = if remote.is_empty() {
                path.clone()
            } else {
                let clean_remote = remote.trim_end_matches(':');
                let clean_path = if path.starts_with('/') {
                    path.clone()
                } else {
                    format!("/{}", path)
                };
                format!("{}:{}", clean_remote, clean_path)
            };
            if !dest_path.is_empty() {
                app.execute_archive_decompress(archive_path, dest_path, tx.clone()).await;
            }
        }
        _ => {}
    }
}

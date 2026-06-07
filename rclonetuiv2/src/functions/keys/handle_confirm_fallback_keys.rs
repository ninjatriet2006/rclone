use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
use crate::app::{App, AppEvent};
use crate::functions::*;

pub async fn handle_confirm_fallback_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    title: String,
    options: Vec<String>,
    mut selected_idx: usize,
    actions: Vec<FallbackAction>,
    restricted_files: Option<Vec<String>>,
    mut restricted_scroll: usize,
    mut focus_files: bool,
) {
    if (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        || key.code == KeyCode::Esc
    {
        if let Some(ref files) = restricted_files {
            let mut src = String::new();
            let mut dest = String::new();
            let mut is_dir = false;
            let mut items = None;
            let mut use_checksum = false;

            for act in &actions {
                match act {
                    FallbackAction::PermissionCopyAsMuchAsPossible { src: s, dest: d, is_dir: id, use_checksum: uc, .. } => {
                        src = s.clone();
                        dest = d.clone();
                        is_dir = *id;
                        use_checksum = *uc;
                        break;
                    }
                    FallbackAction::MultiPermissionCopyAsMuchAsPossible { items: its, dest_remote, dest_path, use_checksum: uc, .. } => {
                        src = format!("({} mục)", its.len());
                        dest = if dest_remote.is_empty() { dest_path.clone() } else { format!("{}:{}", dest_remote, dest_path) };
                        is_dir = true;
                        items = Some(its.clone());
                        use_checksum = *uc;
                        break;
                    }
                    _ => {}
                }
            }

            if !src.is_empty() {
                app.monitor_state.pending_jobs.push(PendingCopyJob {
                    src,
                    dest,
                    is_dir,
                    total_files: 0,
                    restricted_files: files.clone(),
                    status: "Scanned (Has Restrictions)".to_string(),
                    items,
                    use_checksum,
                });
                app.monitor_state.history.push("Đã chuyển tác vụ có file restricted vào hàng chờ".to_string());
            }
        }
        app.explorer_state.popup = ExplorerPopup::None;
    } else {
        match key.code {
            KeyCode::Tab => {
                if restricted_files.is_some() {
                    focus_files = !focus_files;
                    app.explorer_state.popup = ExplorerPopup::ConfirmFallback {
                        title,
                        options,
                        selected_idx,
                        actions,
                        restricted_files,
                        restricted_scroll,
                        focus_files,
                    };
                }
            }
            KeyCode::Up => {
                if focus_files {
                    if restricted_scroll > 0 {
                        restricted_scroll -= 1;
                    }
                } else {
                    if selected_idx == 0 {
                        selected_idx = options.len() - 1;
                    } else {
                        selected_idx -= 1;
                    }
                }
                app.explorer_state.popup = ExplorerPopup::ConfirmFallback {
                    title,
                    options,
                    selected_idx,
                    actions,
                    restricted_files,
                    restricted_scroll,
                    focus_files,
                };
            }
            KeyCode::Down => {
                if focus_files {
                    if let Some(ref files) = restricted_files {
                        if restricted_scroll + 1 < files.len() {
                            restricted_scroll += 1;
                        }
                    }
                } else {
                    selected_idx = (selected_idx + 1) % options.len();
                }
                app.explorer_state.popup = ExplorerPopup::ConfirmFallback {
                    title,
                    options,
                    selected_idx,
                    actions,
                    restricted_files,
                    restricted_scroll,
                    focus_files,
                };
            }
            KeyCode::Enter => {
                if !focus_files {
                    let selected_action = actions[selected_idx].clone();
                    app.explorer_state.popup = ExplorerPopup::None;
                    app.execute_fallback_action(selected_action, tx.clone()).await;
                }
            }
            _ => {}
        }
    }
}

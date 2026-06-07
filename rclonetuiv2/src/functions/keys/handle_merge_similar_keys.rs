use crossterm::event::{KeyEvent, KeyCode};
use crate::app::{App, AppEvent};
use crate::functions::*;
use std::collections::HashSet;

pub async fn handle_merge_similar_destination_select_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    folders: Vec<FileItem>,
    mut selected_idx: usize,
) {
    match key.code {
        KeyCode::Esc => {
            app.explorer_state.popup = ExplorerPopup::None;
        }
        KeyCode::Up => {
            if !folders.is_empty() {
                if selected_idx == 0 {
                    selected_idx = folders.len() - 1;
                } else {
                    selected_idx -= 1;
                }
                app.explorer_state.popup = ExplorerPopup::MergeSimilarDestinationSelect { folders, selected_idx };
            }
        }
        KeyCode::Down => {
            if !folders.is_empty() {
                selected_idx = (selected_idx + 1) % folders.len();
                app.explorer_state.popup = ExplorerPopup::MergeSimilarDestinationSelect { folders, selected_idx };
            }
        }
        KeyCode::Enter => {
            if !folders.is_empty() {
                let folders_count = folders.len();
                app.explorer_state.popup = ExplorerPopup::MergeSimilarScanning { folders_count, scanned_count: 0 };
                app.execute_merge_similar_scan(folders, selected_idx, tx.clone()).await;
            }
        }
        _ => {}
    }
}

pub async fn handle_merge_similar_scanning_keys(
    app: &mut App,
    key: KeyEvent,
) {
    match key.code {
        KeyCode::Esc => {
            app.explorer_state.popup = ExplorerPopup::None;
        }
        _ => {}
    }
}

pub async fn handle_merge_similar_preview_keys(
    app: &mut App,
    key: KeyEvent,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    summary_report: Vec<String>,
    tree_root: TreeNode,
    mut expanded_paths: HashSet<String>,
    mut selected_rel_path: String,
    mut scroll_offset: usize,
    folders: Vec<FileItem>,
    destination_idx: usize,
) {
    let term_h = crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
    let popup_h = term_h * 75 / 100;
    let list_h = popup_h.saturating_sub(4);

    fn find_node_by_path<'a>(node: &'a TreeNode, path: &str) -> Option<&'a TreeNode> {
        if node.rel_path == path {
            return Some(node);
        }
        for (_, child) in &node.children {
            if let Some(n) = find_node_by_path(child, path) {
                return Some(n);
            }
        }
        None
    }

    let mut tree_lines = Vec::new();
    flatten_tree(
        &tree_root,
        "",
        true,
        true,
        &expanded_paths,
        &selected_rel_path,
        &mut tree_lines,
    );

    let current_idx = tree_lines.iter().position(|(_, r, _)| r == &selected_rel_path).unwrap_or(0);

    match key.code {
        KeyCode::Esc => {
            app.explorer_state.popup = ExplorerPopup::None;
            return;
        }
        KeyCode::Up => {
            if current_idx > 0 {
                selected_rel_path = tree_lines[current_idx - 1].1.clone();
            }
        }
        KeyCode::Down => {
            if current_idx + 1 < tree_lines.len() {
                selected_rel_path = tree_lines[current_idx + 1].1.clone();
            }
        }
        KeyCode::Right => {
            if let Some(node) = find_node_by_path(&tree_root, &selected_rel_path) {
                if node.is_dir {
                    if expanded_paths.contains(&selected_rel_path) {
                        if let Some(first_child) = node.children.values().next() {
                            selected_rel_path = first_child.rel_path.clone();
                        }
                    } else {
                        expanded_paths.insert(selected_rel_path.clone());
                    }
                }
            }
        }
        KeyCode::Left => {
            if let Some(node) = find_node_by_path(&tree_root, &selected_rel_path) {
                if node.is_dir && expanded_paths.contains(&selected_rel_path) {
                    expanded_paths.remove(&selected_rel_path);
                } else {
                    if let Some(idx) = selected_rel_path.rfind('/') {
                        selected_rel_path = selected_rel_path[..idx].to_string();
                    } else {
                        selected_rel_path = "".to_string();
                    }
                }
            }
        }
        KeyCode::Char(' ') => {
            if let Some(node) = find_node_by_path(&tree_root, &selected_rel_path) {
                if node.is_dir {
                    if expanded_paths.contains(&selected_rel_path) {
                        expanded_paths.remove(&selected_rel_path);
                    } else {
                        expanded_paths.insert(selected_rel_path.clone());
                    }
                }
            }
        }
        KeyCode::Enter => {
            app.explorer_state.popup = ExplorerPopup::None;
            app.execute_merge_similar(folders, destination_idx, tx.clone()).await;
            return;
        }
        _ => {}
    }

    let mut new_tree_lines = Vec::new();
    flatten_tree(
        &tree_root,
        "",
        true,
        true,
        &expanded_paths,
        &selected_rel_path,
        &mut new_tree_lines,
    );
    let new_idx = new_tree_lines.iter().position(|(_, r, _)| r == &selected_rel_path).unwrap_or(0);
    let combined_idx = summary_report.len() + 1 + new_idx;
    let total_len = summary_report.len() + 1 + new_tree_lines.len();
    scroll_offset = update_scroll_offset(combined_idx, scroll_offset, list_h, total_len);

    app.explorer_state.popup = ExplorerPopup::MergeSimilarPreview {
        summary_report,
        tree_root,
        expanded_paths,
        selected_rel_path,
        scroll_offset,
        folders,
        destination_idx,
    };
}

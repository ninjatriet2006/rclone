use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

#[derive(Debug, Clone, PartialEq)]
pub struct FileItem {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
    #[allow(dead_code)]
    pub mod_time: String,
    pub id: Option<String>,
}

pub struct ExplorerPane {
    pub remote: String, // Rỗng nghĩa là Local Path, ngược lại là tên Remote (ví dụ: "drive")
    pub path: String,   // Đường dẫn hiện tại (ví dụ: "Thư mục gốc" hoặc "folder/sub")
    pub items: Vec<FileItem>,
    pub selected_idx: usize,
    pub scroll_offset: usize,
    pub loading: bool,
    pub selected_names: std::collections::HashSet<String>,
    pub shift_anchor: Option<usize>,
    pub alt_anchor: Option<usize>,
    pub shift_active: bool,
    pub alt_active: bool,
}

impl ExplorerPane {
    pub fn new(remote: &str) -> Self {
        let config = crate::custom_config::TuiCustomConfig::load();
        ExplorerPane {
            remote: remote.to_string(),
            path: if remote.is_empty() {
                let local_dir = config.default_local_dir.trim();
                if local_dir.is_empty() {
                    crate::app_config::get_home_dir()
                } else {
                    local_dir.to_string()
                }
            } else {
                config.default_remote_dir.clone()
            },
            items: Vec::new(),
            selected_idx: 0,
            scroll_offset: 0,
            loading: false,
            selected_names: std::collections::HashSet::new(),
            shift_anchor: None,
            alt_anchor: None,
            shift_active: false,
            alt_active: false,
        }
    }

    pub fn next(&mut self) {
        if !self.items.is_empty() {
            self.selected_idx = (self.selected_idx + 1) % self.items.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.items.is_empty() {
            if self.selected_idx == 0 {
                self.selected_idx = self.items.len() - 1;
            } else {
                self.selected_idx -= 1;
            }
        }
    }

    pub fn adjust_scroll(&mut self, height: usize) {
        if self.items.is_empty() {
            self.scroll_offset = 0;
            return;
        }
        if self.selected_idx < self.scroll_offset {
            self.scroll_offset = self.selected_idx;
        } else if self.selected_idx >= self.scroll_offset + height {
            self.scroll_offset = self.selected_idx - height + 1;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActivePane {
    Left,
    Right,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum FallbackAction {
    MoveNative { src: String, dest: String },
    MoveCopyDelete { src: String, dest: String },
    MoveLocalTransfer { src: String, dest: String },
    CopyNative { src: String, dest: String, use_checksum: bool },
    CopyLocalTransfer { src: String, dest: String, use_checksum: bool },
    DeleteNative { target: String, is_dir: bool },
    DeleteIndividual { target: String },
    RenameCopyDelete { src: String, dest: String, is_dir: bool },
    RenameLocalTransfer { src: String, dest: String, is_dir: bool },
    CleanupCloud { fs: String },
    Rmdir { fs: String, remote: String },
    Rmdirs { fs: String, remote: String },
    Cancel,
    PermissionCancel,
    PermissionCopyAsMuchAsPossible { src: String, dest: String, is_dir: bool, restricted_files: Vec<String>, use_checksum: bool },
    PermissionRestrictedCopy { src: String, dest: String, is_dir: bool, restricted_files: Vec<String>, use_checksum: bool },
    MultiPermissionCopyAsMuchAsPossible { items: Vec<ClipboardItem>, dest_remote: String, dest_path: String, restricted_files: Vec<String>, use_checksum: bool },
    MultiPermissionRestrictedCopy { items: Vec<ClipboardItem>, dest_remote: String, dest_path: String, restricted_files: Vec<String>, use_checksum: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExplorerPopup {
    None,
    InputNewFolder {
        input_buffer: String,
    },
    CopyProgress {
        src: String,
        dest: String,
        pct: f64,
        job_id: Option<i64>,
    },
    MoveProgress {
        src: String,
        dest: String,
        pct: f64,
        job_id: Option<i64>,
    },
    SelectRemote {
        remotes: Vec<String>,
        selected_idx: usize,
    },
    ConfirmFallback {
        title: String,
        options: Vec<String>,
        selected_idx: usize,
        actions: Vec<FallbackAction>,
        restricted_files: Option<Vec<String>>,
        restricted_scroll: usize,
        focus_files: bool,
    },
    InputRename {
        old_name: String,
        input_buffer: String,
        is_dir: bool,
    },
    SpecialActionsMenu {
        selected_idx: usize,
    },
    ViewFile {
        file_name: String,
        content: Vec<String>,
        scroll_offset: usize,
    },
    ChecksumTypeSelect {
        selected_idx: usize,
    },
    CryptdecodeForm {
        remote_input: String,
        encrypted_input: String,
        is_remote_focused: bool,
        output_result: Option<String>,
    },
    DecompressModeSelect {
        archive_path: String,
        selected_idx: usize,
    },
    DecompressPathInput {
        archive_path: String,
        selected_idx: usize,
    },
    DecompressPathManualInput {
        archive_path: String,
        input_buffer: String,
    },
    TuiExplorerSelector {
        archive_path: String,
        remote: String,
        path: String,
        items: Vec<FileItem>,
        selected_idx: usize,
        scroll_offset: usize,
        loading: bool,
    },
    SpecialActionMessage {
        title: String,
        message: String,
    },
    InputPasteRename {
        input_buffer: String,
    },
    InputSharedLink {
        input_buffer: String,
    },
    SelectBaseRemote {
        remotes: Vec<String>,
        selected_idx: usize,
        folder_id: String,
    },
    PermissionScanning {
        src: String,
        dest: String,
        is_dir: bool,
        scanned_count: usize,
        total_files: usize,
        restricted_count: usize,
        items: Option<Vec<ClipboardItem>>,
        use_checksum: bool,
    },
    DedupeModeSelect {
        by_hash: bool,
        selected_idx: usize,
    },
    CopyModeSelect {
        src: String,
        dest: String,
        is_dir: bool,
        is_multi: bool,
        clipboard_items: Option<Vec<ClipboardItem>>,
        action_type: String, // "copy" or "sync"
        selected_idx: usize,
    },
    MergeSimilarDestinationSelect {
        folders: Vec<FileItem>,
        selected_idx: usize,
    },
    MergeSimilarScanning {
        folders_count: usize,
        scanned_count: usize,
    },
    MergeSimilarPreview {
        summary_report: Vec<String>,
        tree_root: TreeNode,
        expanded_paths: std::collections::HashSet<String>,
        selected_rel_path: String,
        scroll_offset: usize,
        folders: Vec<FileItem>,
        destination_idx: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    pub name: String,
    pub rel_path: String,
    pub is_dir: bool,
    pub action: Option<String>,
    pub children: std::collections::BTreeMap<String, TreeNode>,
}

impl TreeNode {
    pub fn new(name: String, rel_path: String, is_dir: bool) -> Self {
        TreeNode {
            name,
            rel_path,
            is_dir,
            action: None,
            children: std::collections::BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, path_parts: &[&str], is_dir: bool, action: Option<String>) {
        if path_parts.is_empty() {
            return;
        }
        let current_name = path_parts[0];
        let is_last = path_parts.len() == 1;

        let child_rel_path = if self.rel_path.is_empty() {
            current_name.to_string()
        } else {
            format!("{}/{}", self.rel_path, current_name)
        };

        let child = self.children.entry(current_name.to_string()).or_insert_with(|| {
            TreeNode::new(current_name.to_string(), child_rel_path, if is_last { is_dir } else { true })
        });

        if is_last {
            child.is_dir = is_dir;
            if action.is_some() {
                child.action = action;
            }
        } else {
            child.insert(&path_parts[1..], is_dir, action);
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClipboardItem {
    pub remote: String,
    pub path: String,
    pub name: String,
    pub is_dir: bool,
}

pub struct ExplorerState {
    pub left_pane: ExplorerPane,
    pub right_pane: ExplorerPane,
    pub active_pane: ActivePane,
    pub popup: ExplorerPopup,
    pub notification: Option<(String, String)>,
    pub clipboard: Option<ClipboardItem>,
    pub clipboard_items: Option<Vec<ClipboardItem>>,
    pub edit_cursor_idx: usize,
}

impl ExplorerState {
    pub fn new() -> Self {
        ExplorerState {
            left_pane: ExplorerPane::new(""),
            right_pane: ExplorerPane::new(""),
            active_pane: ActivePane::Left,
            popup: ExplorerPopup::None,
            notification: None,
            clipboard: None,
            clipboard_items: None,
            edit_cursor_idx: 0,
        }
    }

    pub fn get_active_pane(&self) -> &ExplorerPane {
        match self.active_pane {
            ActivePane::Left => &self.left_pane,
            ActivePane::Right => &self.right_pane,
        }
    }

    pub fn get_active_pane_mut(&mut self) -> &mut ExplorerPane {
        match self.active_pane {
            ActivePane::Left => &mut self.left_pane,
            ActivePane::Right => &mut self.right_pane,
        }
    }

    pub fn toggle_pane(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::Left => ActivePane::Right,
            ActivePane::Right => ActivePane::Left,
        };
    }
}

pub fn draw(state: &mut ExplorerState, frame: &mut Frame, area: Rect) {
    let help_text = crate::lang::translate("exp_help");
    let available_width = area.width.saturating_sub(2) as usize;
    let needed_lines = super::estimate_wrapped_lines(&help_text, available_width);

    let mut help_height = needed_lines.min(3);
    if help_height > 1 {
        let temp_help_bar_height = help_height + 2;
        let list_height = area.height.saturating_sub(3 + temp_help_bar_height as u16);
        let visible_files_height = list_height.saturating_sub(2);
        if visible_files_height <= 5 {
            help_height = 1;
        }
    }
    if help_height == 0 {
        help_height = 1;
    }
    let help_bar_height = help_height + 2;

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(3), // Console panel
            Constraint::Length(help_bar_height as u16), // Help bar
        ])
        .split(area);

    let panes_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_chunks[0]);

    // Vẽ Pane Trái
    draw_pane(
        frame,
        &mut state.left_pane,
        panes_chunks[0],
        state.active_pane == ActivePane::Left,
    );

    // Vẽ Pane Phải
    draw_pane(
        frame,
        &mut state.right_pane,
        panes_chunks[1],
        state.active_pane == ActivePane::Right,
    );

    // Console / Status Panel
    let console_text = if let Some(ref items) = state.clipboard_items {
        // Multi-select clipboard
        let count = items.len();
        let first_remote = items.first().map(|i| i.remote.clone()).unwrap_or_default();
        let label = if first_remote.is_empty() {
            crate::lang::translate("srv_local_system")
        } else {
            first_remote.trim_end_matches(':').to_string()
        };
        let msg = format!("📋 {} mục đã sao chép từ {}", count, label);
        Line::from(vec![
            Span::styled("⚡ ", Style::default().fg(Color::Yellow)),
            Span::styled(msg, Style::default().fg(Color::Black)),
        ])
    } else if let Some(ref item) = state.clipboard {
        let src_display = if item.remote.is_empty() {
            let local_prefix = crate::lang::translate("srv_local_system");
            if item.path == "/" || item.path.is_empty() {
                format!("{}:/{}", local_prefix, item.name)
            } else {
                format!("{}:{}/{}", local_prefix, item.path.trim_end_matches('/'), item.name)
            }
        } else {
            let clean_remote = item.remote.trim_end_matches(':');
            let clean_path = item.path.trim_start_matches('/');
            if clean_path.is_empty() {
                format!("{}:/{}", clean_remote, item.name)
            } else {
                format!("{}:{}/{}", clean_remote, clean_path.trim_end_matches('/'), item.name)
            }
        };

        let template = crate::lang::translate("exp_console_pending");
        let msg = template.replace("{}", &src_display);

        Line::from(vec![
            Span::styled("⚡ ", Style::default().fg(Color::Yellow)),
            Span::styled(msg, Style::default().fg(Color::Black)),
        ])
    } else {
        let empty_msg = crate::lang::translate("exp_console_empty");
        Line::from(vec![
            Span::styled("⚙️ ", Style::default().fg(Color::DarkGray)),
            Span::styled(empty_msg, Style::default().fg(Color::DarkGray)),
        ])
    };

    let console_paragraph = Paragraph::new(console_text)
        .block(
            Block::default()
                .title(Span::styled(
                    crate::lang::translate("exp_console_title"),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(console_paragraph, main_chunks[1]);

    // Help Bar
    let help_paragraph = Paragraph::new(super::parse_help_line(&help_text))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(help_paragraph, main_chunks[2]);

    // Vẽ Popups
    match &state.popup {
        ExplorerPopup::InputNewFolder { input_buffer } => {
            draw_input_new_folder(frame, input_buffer, state.edit_cursor_idx);
        }
        ExplorerPopup::CopyProgress { src, dest, pct, .. } => {
            let msg = crate::lang::translate("exp_copy_msg")
                .replacen("{}", src, 1)
                .replacen("{}", dest, 1)
                .replace("{:.1}", &format!("{:.1}", pct));
            super::draw_popup(frame, &crate::lang::translate("exp_copy_title"), &msg, 60, 35);
        }
        ExplorerPopup::MoveProgress { src, dest, pct, .. } => {
            let msg = crate::lang::translate("exp_move_msg")
                .replacen("{}", src, 1)
                .replacen("{}", dest, 1)
                .replace("{:.1}", &format!("{:.1}", pct));
            super::draw_popup(frame, &crate::lang::translate("exp_move_title"), &msg, 60, 35);
        }
        ExplorerPopup::SelectRemote {
            remotes,
            selected_idx,
        } => {
            draw_select_remote_popup(frame, remotes, *selected_idx);
        }
        ExplorerPopup::ConfirmFallback {
            title,
            options,
            selected_idx,
            restricted_files,
            restricted_scroll,
            focus_files,
            ..
        } => {
            draw_confirm_fallback_popup(
                frame,
                title,
                options,
                *selected_idx,
                restricted_files,
                *restricted_scroll,
                *focus_files,
            );
        }
        ExplorerPopup::InputRename { old_name, input_buffer, .. } => {
            draw_input_rename(frame, old_name, input_buffer, state.edit_cursor_idx);
        }
        ExplorerPopup::SpecialActionsMenu { selected_idx } => {
            let active_pane = if state.active_pane == ActivePane::Left { &state.left_pane } else { &state.right_pane };
            let is_trash_view = active_pane.remote.contains(",trashed_only=true");
            draw_special_actions_menu(frame, *selected_idx, is_trash_view);
        }
        ExplorerPopup::ViewFile { file_name, content, scroll_offset } => {
            draw_view_file(frame, file_name, content, *scroll_offset);
        }
        ExplorerPopup::ChecksumTypeSelect { selected_idx } => {
            draw_checksum_type_select(frame, *selected_idx);
        }
        ExplorerPopup::CryptdecodeForm { remote_input, encrypted_input, is_remote_focused, output_result } => {
            draw_cryptdecode_form(frame, remote_input, encrypted_input, *is_remote_focused, output_result.as_deref(), state.edit_cursor_idx);
        }
        ExplorerPopup::DecompressModeSelect { archive_path, selected_idx } => {
            draw_decompress_mode_select(frame, archive_path, *selected_idx);
        }
        ExplorerPopup::DecompressPathInput { archive_path, selected_idx } => {
            draw_decompress_path_input(frame, archive_path, *selected_idx);
        }
        ExplorerPopup::DecompressPathManualInput { archive_path, input_buffer } => {
            draw_decompress_path_manual_input(frame, archive_path, input_buffer, state.edit_cursor_idx);
        }
        ExplorerPopup::TuiExplorerSelector { archive_path, remote, path, items, selected_idx, scroll_offset, loading } => {
            draw_tui_explorer_selector(frame, archive_path, remote, path, items, *selected_idx, *scroll_offset, *loading);
        }
        ExplorerPopup::SpecialActionMessage { title, message } => {
            super::draw_popup(frame, title, message, 65, 40);
        }
        ExplorerPopup::InputPasteRename { input_buffer } => {
            draw_input_paste_rename(frame, input_buffer, state.edit_cursor_idx);
        }
        ExplorerPopup::InputSharedLink { input_buffer } => {
            draw_input_shared_link(frame, input_buffer, state.edit_cursor_idx);
        }
        ExplorerPopup::SelectBaseRemote { remotes, selected_idx, .. } => {
            draw_select_base_remote_popup(frame, remotes, *selected_idx);
        }
        ExplorerPopup::PermissionScanning { src, dest, is_dir: _, scanned_count, total_files, restricted_count, .. } => {
            draw_permission_scanning_popup(frame, src, dest, *scanned_count, *total_files, *restricted_count);
        }
        ExplorerPopup::DedupeModeSelect { by_hash, selected_idx } => {
            draw_dedupe_mode_select(frame, *selected_idx, *by_hash);
        }
        ExplorerPopup::CopyModeSelect { selected_idx, .. } => {
            draw_copy_mode_select(frame, *selected_idx);
        }
        ExplorerPopup::MergeSimilarDestinationSelect { folders, selected_idx } => {
            draw_merge_similar_destination_select(frame, folders, *selected_idx);
        }
        ExplorerPopup::MergeSimilarScanning { folders_count, scanned_count } => {
            draw_merge_similar_scanning(frame, *folders_count, *scanned_count);
        }
        ExplorerPopup::MergeSimilarPreview {
            summary_report,
            tree_root,
            expanded_paths,
            selected_rel_path,
            scroll_offset,
            ..
        } => {
            draw_merge_similar_preview(
                frame,
                summary_report,
                tree_root,
                expanded_paths,
                selected_rel_path,
                *scroll_offset,
            );
        }
        ExplorerPopup::None => {}
    }

    if let Some((ref title, ref msg)) = state.notification {
        super::draw_popup(frame, title, msg, 60, 30);
    }
}

fn draw_pane(frame: &mut Frame, pane: &mut ExplorerPane, area: Rect, is_active: bool) {
    let title_color = if is_active {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let border_color = if is_active {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let fs_label = if pane.remote.is_empty() {
        crate::lang::translate("srv_local_system")
    } else {
        pane.remote.clone()
    };

    let title = if pane.selected_names.is_empty() {
        format!(" {} : {} ", fs_label, pane.path)
    } else {
        format!(" {} : {} (Đã chọn: {}) ", fs_label, pane.path, pane.selected_names.len())
    };

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(title_color)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let height = area.height.saturating_sub(2) as usize;
    pane.adjust_scroll(height);

    let pane_width = area.width as usize;

    // Thiết kế adaptive tự động ẩn các cột dựa trên kích thước panel
    let (show_size, size_col_width) = if pane_width >= 40 {
        (true, 10)
    } else {
        (false, 0)
    };

    let total_cols_width = if show_size {
        size_col_width + 2 // 1 khoảng trống rộng 2 ký tự
    } else {
        0
    };

    let name_width = pane_width.saturating_sub(2 + total_cols_width); // trừ đi 2 ký tự biên block

    let items: Vec<ListItem> = if pane.items.is_empty() {
        if pane.loading {
            vec![ListItem::new(Line::from(vec![Span::styled(
                crate::lang::translate("exp_loading"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::ITALIC),
            )]))]
        } else {
            vec![ListItem::new(Line::from(vec![Span::styled(
                crate::lang::translate("exp_empty"),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )]))]
        }
    } else {
        pane.items
            .iter()
            .enumerate()
            .skip(pane.scroll_offset)
            .take(height)
            .map(|(i, item)| {
                let is_selected_item = pane.selected_names.contains(&item.name);
                let is_anchor = pane.shift_anchor == Some(i) || pane.alt_anchor == Some(i);

                let select_prefix = if item.name == ".." {
                    "  "
                } else if is_selected_item {
                    "✔ "
                } else {
                    "  "
                };

                let prefix = if item.name == ".." {
                    "📁 "
                } else if item.is_dir {
                    "📁 "
                } else {
                    "📄 "
                };

                // Cắt bớt / đệm khoảng trắng tên tệp dựa trên name_width động
                let display_name = format_display_name(&item.name);
                let raw_name = format!("{}{}{}", select_prefix, prefix, display_name);
                let display_name = if raw_name.chars().count() > name_width {
                    if name_width > 3 {
                        raw_name.chars().take(name_width - 3).collect::<String>() + "..."
                    } else {
                        raw_name.chars().take(name_width).collect::<String>()
                    }
                } else {
                    let pad = name_width.saturating_sub(raw_name.chars().count());
                    raw_name + &" ".repeat(pad)
                };

                let mut style = if i == pane.selected_idx && is_active {
                    if is_selected_item {
                        Style::default().fg(Color::Black).bg(Color::LightGreen).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
                    }
                } else if is_selected_item {
                    Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)
                } else if item.is_dir {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                if is_anchor {
                    style = style.add_modifier(Modifier::UNDERLINED);
                    if i != pane.selected_idx || !is_active {
                        style = style.fg(Color::LightMagenta);
                    }
                }

                // Sử dụng style thống nhất cho cả name_span để tránh xung đột màu
                let name_span = Span::styled(display_name, style);

                let mut spans = vec![name_span];

                if show_size {
                    let size_str = if item.is_dir {
                        "---".to_string()
                    } else if item.name == ".." {
                        "---".to_string()
                    } else {
                        super::format_size(item.size)
                    };
                    let size_formatted = if size_str.len() < size_col_width {
                        let pad = size_col_width - size_str.len();
                        " ".repeat(pad) + &size_str
                    } else {
                        size_str.chars().take(size_col_width).collect::<String>()
                    };
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(
                        size_formatted,
                        if i == pane.selected_idx && is_active {
                            style // Dùng cùng style với name khi đang được trỏ
                        } else {
                            Style::default().fg(Color::Magenta)
                        },
                    ));
                }

                let item_style = if i == pane.selected_idx && is_active {
                    if is_selected_item {
                        Style::default().bg(Color::LightGreen)
                    } else {
                        Style::default().bg(Color::Cyan)
                    }
                } else {
                    Style::default()
                };

                let line = Line::from(spans);
                ListItem::new(line).style(item_style)
            })
            .collect()
    };

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn draw_input_new_folder(frame: &mut Frame, input_buffer: &str, cursor_idx: usize) {
    let size = frame.size();
    let area = centered_rect(50, 25, size);
    frame.render_widget(Clear, area);

    let mut spans = vec![Span::styled("> ", Style::default().fg(Color::Cyan))];
    spans.extend(super::make_input_spans_with_cursor(input_buffer, cursor_idx, Color::White, Color::DarkGray));

    let text = vec![
        Line::from(crate::lang::translate("exp_new_folder_prompt")),
        Line::from(""),
        Line::from(spans),
    ];

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("exp_new_folder_title"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_select_remote_popup(frame: &mut Frame, remotes: &[String], selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(50, 45, size);
    frame.render_widget(Clear, area);

    let items: Vec<ListItem> = remotes
        .iter()
        .enumerate()
        .map(|(i, remote)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", remote)).style(style)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("exp_select_remote_title"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let height = area.height.saturating_sub(2) as usize;
    let range = super::calculate_scroll_range(selected_idx, items.len(), height);
    let visible_items: Vec<ListItem> = items.into_iter().skip(range.start).take(range.end - range.start).collect();

    let list = List::new(visible_items).block(block);
    frame.render_widget(list, area);
}

fn draw_confirm_fallback_popup(
    frame: &mut Frame,
    title: &str,
    options: &[String],
    selected_idx: usize,
    restricted_files: &Option<Vec<String>>,
    restricted_scroll: usize,
    focus_files: bool,
) {
    let size = frame.size();
    let area = if restricted_files.is_some() {
        centered_rect(75, 70, size)
    } else {
        centered_rect(65, 50, size)
    };
    frame.render_widget(Clear, area);

    let chunks = if restricted_files.is_some() {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Warning header
                Constraint::Min(6),    // Scrollable restricted files list
                Constraint::Length(6), // Options list
                Constraint::Length(2), // Help / Instructions footer
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Warning header
                Constraint::Min(4),    // Options list
            ])
            .split(area)
    };

    // Render warning title / header
    let header_text = if restricted_files.is_some() {
        Line::from(vec![
            Span::styled("⚠️ PHÁT HIỆN QUYỀN TRUY CẬP BỊ HẠN CHẾ (ACCESS RESTRICTED): ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw("Một số tệp tin không có quyền tải xuống!"),
        ])
    } else {
        Line::from(vec![
            Span::styled("⚠️ CẢNH BÁO: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Tính năng này không hỗ trợ trực tiếp bởi Remote!"),
        ])
    };
    
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::BOTTOM))
        .style(Style::default().fg(Color::White));
    frame.render_widget(header, chunks[0]);

    if let Some(files) = restricted_files {
        let files_border_color = if focus_files { Color::Cyan } else { Color::DarkGray };
        let files_title = format!(" DANH SÁCH FILE BỊ KHÓA / CHẶN TẢI ({}) ", files.len());
        let files_block = Block::default()
            .title(Span::styled(files_title, Style::default().fg(files_border_color).add_modifier(Modifier::BOLD)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(files_border_color));
        
        let files_height = chunks[1].height.saturating_sub(2) as usize;
        let range = super::calculate_scroll_range(restricted_scroll, files.len(), files_height);
        
        let file_items: Vec<ListItem> = files[range.clone()]
            .iter()
            .enumerate()
            .map(|(idx, f)| {
                let actual_idx = range.start + idx;
                let is_selected_file = actual_idx == restricted_scroll;
                
                let prefix = if is_selected_file && focus_files {
                    "👉 "
                } else {
                    "   "
                };
                
                let style = if is_selected_file && focus_files {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else if is_selected_file {
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                
                ListItem::new(Line::from(vec![
                    Span::styled(format!("  {:02}. ", actual_idx + 1), Style::default().fg(Color::DarkGray)),
                    Span::styled("🔒 ", Style::default().fg(Color::Red)),
                    Span::styled(prefix, Style::default().fg(Color::Yellow)),
                    Span::styled(f.clone(), style),
                ]))
            })
            .collect();
            
        let list = List::new(file_items).block(files_block);
        frame.render_widget(list, chunks[1]);

        let options_border_color = if !focus_files { Color::Cyan } else { Color::DarkGray };
        let options_title = " LỰA CHỌN PHƯƠNG ÁN XỬ LÝ ";
        let options_block = Block::default()
            .title(Span::styled(options_title, Style::default().fg(options_border_color).add_modifier(Modifier::BOLD)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(options_border_color));

        let items: Vec<ListItem> = options
            .iter()
            .enumerate()
            .map(|(i, opt)| {
                let style = if !focus_files && i == selected_idx {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else if i == selected_idx {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::UNDERLINED)
                } else {
                    Style::default()
                };
                ListItem::new(format!("  • {}", opt)).style(style)
            })
            .collect();
        
        let list = List::new(items).block(options_block);
        frame.render_widget(list, chunks[2]);

        let help_text = if focus_files {
            " [Tab] Chuyển Focus | [Up/Down] Cuộn danh sách file | [Esc] Hủy "
        } else {
            " [Tab] Chuyển Focus | [Up/Down] Chọn phương án | [Enter] Thực thi | [Esc] Hủy "
        };
        let help_paragraph = Paragraph::new(super::parse_help_line(help_text));
        frame.render_widget(help_paragraph, chunks[3]);
    } else {
        // Render options list
        let items: Vec<ListItem> = options
            .iter()
            .enumerate()
            .map(|(i, opt)| {
                let style = if i == selected_idx {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(format!("  • {}", opt)).style(style)
            })
            .collect();

        let block = Block::default()
            .title(Span::styled(
                title,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let height = chunks[1].height.saturating_sub(2) as usize;
        let range = super::calculate_scroll_range(selected_idx, items.len(), height);
        let visible_items: Vec<ListItem> = items.into_iter().skip(range.start).take(range.end - range.start).collect();

        let list = List::new(visible_items).block(block);
        frame.render_widget(list, chunks[1]);
    }
}

fn draw_input_rename(frame: &mut Frame, old_name: &str, input_buffer: &str, cursor_idx: usize) {
    let size = frame.size();
    let area = centered_rect(50, 25, size);
    frame.render_widget(Clear, area);

    let mut spans = vec![Span::styled("> ", Style::default().fg(Color::Cyan))];
    spans.extend(super::make_input_spans_with_cursor(input_buffer, cursor_idx, Color::White, Color::DarkGray));

    let text = vec![
        Line::from(format!("Tên cũ: {}", old_name)),
        Line::from(""),
        Line::from(spans),
    ];

    let block = Block::default()
        .title(Span::styled(
            " ĐỔI TÊN TỆP / THƯ MỤC ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_special_actions_menu(frame: &mut Frame, selected_idx: usize, is_trash_view: bool) {
    let size = frame.size();
    let area = centered_rect(50, 50, size);
    frame.render_widget(Clear, area);

    let options = if is_trash_view {
        vec![
            crate::lang::translate("exp_special_trash_untrash"),
            crate::lang::translate("exp_special_trash_exit"),
            crate::lang::translate("exp_special_close"),
        ]
    } else {
        vec![
            crate::lang::translate("exp_special_link"),
            crate::lang::translate("exp_special_hash"),
            crate::lang::translate("exp_special_cleanup"),
            crate::lang::translate("exp_special_rmdir"),
            crate::lang::translate("exp_special_rmdirs"),
            crate::lang::translate("exp_special_cryptdecode"),
            crate::lang::translate("exp_special_archive"),
            crate::lang::translate("exp_special_dedupe"),
            crate::lang::translate("exp_special_merge_similar"),
            crate::lang::translate("exp_special_trash_view"),
            crate::lang::translate("exp_special_close"),
        ]
    };

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", opt)).style(style)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("exp_special_title"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn draw_view_file(frame: &mut Frame, file_name: &str, content: &[String], scroll_offset: usize) {
    let size = frame.size();
    let area = centered_rect(75, 75, size);
    frame.render_widget(Clear, area);

    let height = area.height.saturating_sub(4) as usize; // reserve space for border and instructions
    let visible_lines: Vec<ListItem> = content
        .iter()
        .skip(scroll_offset)
        .take(height)
        .map(|line| ListItem::new(line.clone()))
        .collect();

    let footer = format!(" [Up/Down] Cuộn | [Esc] Thoát | Dòng {} - {} / {}", scroll_offset + 1, (scroll_offset + height).min(content.len()), content.len());
    let block = Block::default()
        .title(Span::styled(
            format!(" Xem file: {} ", file_name),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(visible_lines).block(block);
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);
    
    frame.render_widget(list, chunks[0]);
    frame.render_widget(Paragraph::new(footer).style(Style::default().fg(Color::DarkGray)), chunks[1]);
}

fn draw_checksum_type_select(frame: &mut Frame, selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(40, 35, size);
    frame.render_widget(Clear, area);

    let options = vec![
        "md5".to_string(),
        "sha1".to_string(),
        "sha256".to_string(),
        "crc32".to_string(),
        "xxhash".to_string(),
    ];

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", opt.to_uppercase())).style(style)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("exp_hash_title"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn draw_cryptdecode_form(
    frame: &mut Frame,
    remote_input: &str,
    encrypted_input: &str,
    is_remote_focused: bool,
    output_result: Option<&str>,
    cursor_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(65, 45, size);
    frame.render_widget(Clear, area);

    let remote_spans = if is_remote_focused {
        let mut spans = vec![Span::styled("1. Crypt Remote (e.g. mycrypt:): ", Style::default().fg(Color::Yellow))];
        spans.extend(super::make_input_spans_with_cursor(remote_input, cursor_idx, Color::White, Color::Blue));
        spans
    } else {
        vec![
            Span::styled("1. Crypt Remote (e.g. mycrypt:): ", Style::default().fg(Color::DarkGray)),
            Span::styled(remote_input, Style::default().fg(Color::White).bg(Color::DarkGray)),
        ]
    };

    let encrypted_spans = if !is_remote_focused {
        let mut spans = vec![Span::styled("2. Encrypted Filename/Path: ", Style::default().fg(Color::Yellow))];
        spans.extend(super::make_input_spans_with_cursor(encrypted_input, cursor_idx, Color::White, Color::Blue));
        spans
    } else {
        vec![
            Span::styled("2. Encrypted Filename/Path: ", Style::default().fg(Color::DarkGray)),
            Span::styled(encrypted_input, Style::default().fg(Color::White).bg(Color::DarkGray)),
        ]
    };

    let text = vec![
        Line::from(remote_spans),
        Line::from(""),
        Line::from(encrypted_spans),
        Line::from(""),
        Line::from("------------------------------------------------------------------"),
        Line::from("Kết quả giải mã / Decrypted Output:"),
        Line::from(""),
        Line::from(Span::styled(
            output_result.unwrap_or("Chưa giải mã (Nhấn Enter để giải mã)"),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(crate::lang::translate("exp_cryptdecode_help")),
    ];

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("exp_cryptdecode_title"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_decompress_mode_select(frame: &mut Frame, archive_path: &str, selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(55, 30, size);
    frame.render_widget(Clear, area);

    let options = vec![
        crate::lang::translate("exp_archive_here"),
        crate::lang::translate("exp_archive_folder"),
        crate::lang::translate("exp_archive_path"),
    ];

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", opt)).style(style)
        })
        .collect();

    let filename = std::path::Path::new(archive_path).file_name().and_then(|f| f.to_str()).unwrap_or(archive_path);
    let block = Block::default()
        .title(Span::styled(
            format!(" {} ({}) ", crate::lang::translate("exp_archive_title"), filename),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn draw_decompress_path_input(frame: &mut Frame, _archive_path: &str, selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(55, 25, size);
    frame.render_widget(Clear, area);

    let options = vec![
        crate::lang::translate("exp_archive_path_manual"),
        crate::lang::translate("exp_archive_path_tui"),
    ];

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", opt)).style(style)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("exp_archive_path_title"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn draw_decompress_path_manual_input(frame: &mut Frame, _archive_path: &str, input_buffer: &str, cursor_idx: usize) {
    let size = frame.size();
    let area = centered_rect(60, 25, size);
    frame.render_widget(Clear, area);

    let mut spans = vec![Span::styled("> ", Style::default().fg(Color::Cyan))];
    spans.extend(super::make_input_spans_with_cursor(input_buffer, cursor_idx, Color::White, Color::DarkGray));

    let text = vec![
        Line::from(crate::lang::translate("exp_archive_manual_prompt")),
        Line::from(""),
        Line::from(spans),
        Line::from(""),
        Line::from("[Enter] Xác nhận | [Esc] Hủy bỏ"),
    ];

    let block = Block::default()
        .title(Span::styled(
            " ĐƯỜNG DẪN GIẢI NÉN ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_tui_explorer_selector(
    frame: &mut Frame,
    _archive_path: &str,
    remote: &str,
    path: &str,
    items: &[FileItem],
    selected_idx: usize,
    scroll_offset: usize,
    loading: bool,
) {
    let size = frame.size();
    let area = centered_rect(70, 70, size);
    frame.render_widget(Clear, area);

    let fs_label = if remote.is_empty() {
        crate::lang::translate("srv_local_system")
    } else {
        remote.to_string()
    };
    let title = format!(" [Duyệt Đích] {} : {} ", fs_label, path);

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    frame.render_widget(&block, area);
    let inner_area = block.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(inner_area);

    let height = chunks[0].height as usize;
    let list_items: Vec<ListItem> = if items.is_empty() {
        if loading {
            vec![ListItem::new(crate::lang::translate("exp_loading"))]
        } else {
            vec![ListItem::new(crate::lang::translate("exp_empty"))]
        }
    } else {
        items
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(height)
            .map(|(i, item)| {
                let prefix = if item.is_dir { "📁 " } else { "📄 " };
                let line = Line::from(vec![
                    Span::styled(
                        format!("{}{}", prefix, format_display_name(&item.name)),
                        if item.is_dir {
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        },
                    ),
                ]);
                let style = if i == selected_idx {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default()
                };
                ListItem::new(line).style(style)
            })
            .collect()
    };

    let list = List::new(list_items);
    frame.render_widget(list, chunks[0]);

    let footer_text = crate::lang::translate("exp_archive_tui_prompt");
    frame.render_widget(Paragraph::new(footer_text).style(Style::default().fg(Color::Yellow)), chunks[1]);
}

fn draw_input_paste_rename(frame: &mut Frame, input_buffer: &str, cursor_idx: usize) {
    let size = frame.size();
    let area = centered_rect(50, 25, size);
    frame.render_widget(Clear, area);

    let mut spans = vec![Span::styled("> ", Style::default().fg(Color::Cyan))];
    spans.extend(super::make_input_spans_with_cursor(input_buffer, cursor_idx, Color::White, Color::DarkGray));

    let text = vec![
        Line::from(crate::lang::translate("exp_paste_rename_prompt")),
        Line::from(""),
        Line::from(spans),
        Line::from(""),
        Line::from("[Enter] Xác nhận & Dán | [Esc] Hủy bỏ"),
    ];

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("exp_paste_rename_title"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

use super::centered_rect;

fn draw_input_shared_link(frame: &mut Frame, input_buffer: &str, cursor_idx: usize) {
    let size = frame.size();
    let area = centered_rect(65, 25, size);
    frame.render_widget(Clear, area);

    let mut spans = vec![Span::styled("> ", Style::default().fg(Color::Cyan))];
    spans.extend(super::make_input_spans_with_cursor(input_buffer, cursor_idx, Color::White, Color::DarkGray));

    let text = vec![
        Line::from(crate::lang::translate("exp_input_shared_link_prompt")),
        Line::from(""),
        Line::from(spans),
        Line::from(""),
        Line::from("[Enter] Xác nhận | [Esc] Hủy bỏ"),
    ];

    let block = Block::default()
        .title(Span::styled(
            " THÊM LINK SHARED (GOOGLE DRIVE) ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_select_base_remote_popup(frame: &mut Frame, remotes: &[String], selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(50, 45, size);
    frame.render_widget(Clear, area);

    let items: Vec<ListItem> = remotes
        .iter()
        .enumerate()
        .map(|(i, remote)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", remote)).style(style)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("exp_select_base_remote_title"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let height = area.height.saturating_sub(2) as usize;
    let range = super::calculate_scroll_range(selected_idx, items.len(), height);
    let visible_items: Vec<ListItem> = items.into_iter().skip(range.start).take(range.end - range.start).collect();

    let list = List::new(visible_items).block(block);
    frame.render_widget(list, area);
}

fn draw_permission_scanning_popup(
    frame: &mut Frame,
    src: &str,
    dest: &str,
    scanned_count: usize,
    total_files: usize,
    restricted_count: usize,
) {
    let size = frame.size();
    let area = centered_rect(65, 35, size);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " KIỂM TRA QUYỀN SỞ HỮU / TẢI XUỐNG (PERMISSION PRE-CHECK) ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let width = area.width.saturating_sub(6) as usize;
    let bar_str = if total_files > 0 {
        let pct = (scanned_count as f64 / total_files as f64) * 100.0;
        let filled = ((pct.min(100.0) * width as f64) / 100.0) as usize;
        let empty = width.saturating_sub(filled);
        format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
    } else {
        let block_size = 6.max(width / 10).min(width.saturating_sub(1));
        let mut chars = vec!['░'; width];
        if width > 0 {
            let offset = scanned_count % width;
            for i in 0..block_size {
                let idx = (offset + i) % width;
                chars[idx] = '█';
            }
        }
        let marquee_str: String = chars.into_iter().collect();
        format!("[{}]", marquee_str)
    };

    let scan_status_line = if total_files > 0 {
        let pct = (scanned_count as f64 / total_files as f64) * 100.0;
        Line::from(vec![
            Span::styled("Đang quét: ", Style::default().fg(Color::Cyan)),
            Span::styled(format!("{:.1}% ", pct), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(format!("({}/{} file) ", scanned_count, total_files), Style::default().fg(Color::White)),
        ])
    } else {
        Line::from(vec![
            Span::styled("Đang quét: ", Style::default().fg(Color::Cyan)),
            Span::styled(format!("{} tệp tin ", scanned_count), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("(đang phân tích thư mục...) ", Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
        ])
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("Nguồn: ", Style::default().fg(Color::DarkGray)),
            Span::styled(src.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Đích:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(dest.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        scan_status_line,
        Line::from(Span::styled(bar_str, Style::default().fg(Color::Green))),
        Line::from(""),
        Line::from(vec![
            Span::styled("Phát hiện bị chặn tải: ", Style::default().fg(Color::LightRed)),
            Span::styled(format!("{} tệp tin", restricted_count), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Nhấn ", Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
            Span::styled("[Esc]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(" để ẩn (chạy ngầm) | ", Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
            Span::styled("[Ctrl+C]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(" để bỏ qua và đưa vào hàng chờ ", Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
        ]),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_dedupe_mode_select(frame: &mut Frame, selected_idx: usize, by_hash: bool) {
    let size = frame.size();
    let area = centered_rect(65, 55, size);
    frame.render_widget(Clear, area);

    let hash_status = if by_hash {
        Span::styled("BẬT (Tìm trùng theo nội dung/mã Hash)", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("TẮT (Tìm trùng theo Tên file)", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    };

    let title_text = crate::lang::translate("exp_dedupe_title");
    let prompt_text = crate::lang::translate("exp_dedupe_prompt");
    let by_hash_prompt = crate::lang::translate("exp_dedupe_by_hash_prompt").replace("{}", "");
    let help_text = crate::lang::translate("exp_dedupe_help");

    let modes = vec![
        crate::lang::translate("exp_dedupe_mode_rename"),
        crate::lang::translate("exp_dedupe_mode_newest"),
        crate::lang::translate("exp_dedupe_mode_oldest"),
        crate::lang::translate("exp_dedupe_mode_largest"),
        crate::lang::translate("exp_dedupe_mode_smallest"),
        crate::lang::translate("exp_dedupe_mode_first"),
        crate::lang::translate("exp_dedupe_mode_skip"),
    ];

    let items: Vec<ListItem> = modes
        .iter()
        .enumerate()
        .map(|(i, mode)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", mode)).style(style)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            title_text,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Prompt
            Constraint::Length(3), // By Hash toggle status
            Constraint::Min(5),    // List of modes
            Constraint::Length(2), // Help
        ])
        .split(block.inner(area));

    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    frame.render_widget(Paragraph::new(prompt_text), chunks[0]);

    let hash_prompt_para = Paragraph::new(vec![
        Line::from(by_hash_prompt),
        Line::from(hash_status),
    ])
    .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(hash_prompt_para, chunks[1]);

    let list = List::new(items);
    frame.render_widget(list, chunks[2]);

    let help_para = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC));
    frame.render_widget(help_para, chunks[3]);
}

fn draw_copy_mode_select(frame: &mut Frame, selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(65, 35, size);
    frame.render_widget(Clear, area);

    let title_text = crate::lang::translate("exp_copy_mode_title");
    let prompt_text = crate::lang::translate("exp_copy_mode_prompt");
    let help_text = crate::lang::translate("exp_copy_mode_help");

    let modes = vec![
        crate::lang::translate("exp_copy_mode_normal"),
        crate::lang::translate("exp_copy_mode_checksum"),
    ];

    let items: Vec<ListItem> = modes
        .iter()
        .enumerate()
        .map(|(i, mode)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", mode)).style(style)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            title_text,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Prompt
            Constraint::Min(2),    // List of options
            Constraint::Length(2), // Help
        ])
        .split(block.inner(area));

    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    frame.render_widget(Paragraph::new(prompt_text), chunks[0]);

    let list = List::new(items);
    frame.render_widget(list, chunks[1]);

    let help_para = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC));
    frame.render_widget(help_para, chunks[2]);
}

pub(crate) fn format_display_name(name: &str) -> String {
    if name.starts_with(' ') || name.ends_with(' ') {
        let trimmed_start = name.trim_start_matches(' ');
        let leading_count = name.len() - trimmed_start.len();
        let trimmed_end = trimmed_start.trim_end_matches(' ');
        let trailing_count = trimmed_start.len() - trimmed_end.len();
        format!(
            "{}{}{}",
            "·".repeat(leading_count),
            trimmed_end,
            "·".repeat(trailing_count)
        )
    } else {
        name.to_string()
    }
}

fn draw_merge_similar_destination_select(
    frame: &mut Frame,
    folders: &[FileItem],
    selected_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(65, 45, size);
    frame.render_widget(Clear, area);

    let items: Vec<ListItem> = folders
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_target = i == selected_idx;
            let display = format_display_name(&item.name);
            let line = if is_target {
                Line::from(vec![
                    Span::styled("👉 ", Style::default().fg(Color::Yellow)),
                    Span::styled(format!("{} (Thư mục đích / Destination)", display), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ])
            } else {
                Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(format!("{} (Thư mục nguồn / Source)", display), Style::default().fg(Color::White)),
                ])
            };
            ListItem::new(line)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            " CHỌN THƯ MỤC ĐÍCH ĐỂ GỘP (SELECT DESTINATION) ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner_area = block.inner(area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Prompt
            Constraint::Min(4),    // List of folders
            Constraint::Length(2), // Help line
        ])
        .split(inner_area);

    frame.render_widget(block, area);

    let prompt = Paragraph::new("Chọn thư mục sẽ nhận tất cả dữ liệu. Các thư mục khác sẽ bị xóa sau khi gộp:")
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(prompt, chunks[0]);

    let list = List::new(items)
        .block(Block::default().borders(Borders::NONE));
    frame.render_widget(list, chunks[1]);

    let help_line = Paragraph::new(" [Up/Down] Chọn thư mục đích | [Enter] Xem trước & Quét trùng | [Esc] Hủy ")
        .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC));
    frame.render_widget(help_line, chunks[2]);
}

fn draw_merge_similar_scanning(
    frame: &mut Frame,
    folders_count: usize,
    scanned_count: usize,
) {
    let size = frame.size();
    let area = centered_rect(50, 20, size);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " ĐANG QUÉT DỮ LIỆU THƯ MỤC... ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let msg = format!(
        "\n  Đang quét cấu trúc file của {} thư mục...\n\n  Đã quét xong: {} / {}\n\n  Vui lòng đợi trong giây lát...",
        folders_count, scanned_count, folders_count
    );

    let paragraph = Paragraph::new(msg).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_merge_similar_preview(
    frame: &mut Frame,
    summary_report: &[String],
    tree_root: &TreeNode,
    expanded_paths: &std::collections::HashSet<String>,
    selected_rel_path: &str,
    scroll_offset: usize,
) {
    let size = frame.size();
    let area = centered_rect(75, 75, size);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " XEM TRƯỚC KẾT QUẢ GỘP (MERGE PREVIEW REPORT) ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let inner_area = block.inner(area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),    // Report content
            Constraint::Length(2), // Help line
        ])
        .split(inner_area);

    frame.render_widget(block, area);

    let mut tree_lines = Vec::new();
    flatten_tree(tree_root, "", true, true, expanded_paths, selected_rel_path, &mut tree_lines);

    let mut items = Vec::new();
    for line in summary_report {
        let style = Style::default().fg(Color::White);
        items.push(ListItem::new(Line::from(Span::styled(line.clone(), style))));
    }

    items.push(ListItem::new(Line::from("")));

    for (formatted, _rel_path, is_selected) in tree_lines {
        let mut style = if formatted.contains("TRÙNG LẶP") || formatted.contains("XÓA") || formatted.contains("DELETE") {
            Style::default().fg(Color::Yellow)
        } else if formatted.contains("DI CHUYỂN") || formatted.contains("MOVE") {
            Style::default().fg(Color::Green)
        } else if formatted.contains("GHI ĐÈ") || formatted.contains("OVERWRITE") {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else if formatted.contains("=====") || formatted.contains("-----") {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };

        if is_selected {
            style = style.bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD);
        }

        let mut spans = Vec::new();
        let indicator_style = if is_selected {
            Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)
        };
        let expand_style = if is_selected {
            Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD)
        };

        if let Some(idx) = formatted.find('▶') {
            let prefix = &formatted[..idx];
            let rest = &formatted[idx + '▶'.len_utf8()..];
            spans.push(Span::styled(prefix.to_string(), style));
            spans.push(Span::styled("▶", indicator_style));
            spans.push(Span::styled(rest.to_string(), style));
        } else if let Some(idx) = formatted.find('▼') {
            let prefix = &formatted[..idx];
            let rest = &formatted[idx + '▼'.len_utf8()..];
            spans.push(Span::styled(prefix.to_string(), style));
            spans.push(Span::styled("▼", expand_style));
            spans.push(Span::styled(rest.to_string(), style));
        } else {
            spans.push(Span::styled(formatted, style));
        }
        items.push(ListItem::new(Line::from(spans)));
    }

    let height = chunks[0].height as usize;
    let visible_items: Vec<ListItem> = items
        .into_iter()
        .skip(scroll_offset)
        .take(height)
        .collect();

    let list = List::new(visible_items).block(Block::default().borders(Borders::NONE));
    frame.render_widget(list, chunks[0]);

    let help_line = Paragraph::new(" [Up/Down] Di chuyển | [Right/Left/Space] Đóng/Mở thư mục | [Enter] Bắt đầu gộp | [Esc] Hủy bỏ ")
        .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC));
    frame.render_widget(help_line, chunks[1]);
}

pub fn flatten_tree(
    node: &TreeNode,
    prefix: &str,
    is_last: bool,
    is_root: bool,
    expanded_paths: &std::collections::HashSet<String>,
    selected_rel_path: &str,
    lines: &mut Vec<(String, String, bool)>,
) {
    let mut node_str = if node.is_dir {
        format!("{}/", node.name)
    } else {
        node.name.clone()
    };

    if node.is_dir && !is_root {
        let indicator = if expanded_paths.contains(&node.rel_path) {
            "▼ "
        } else {
            "▶ "
        };
        node_str = format!("{}{}", indicator, node_str);
    }

    let action_str = match &node.action {
        Some(act) => format!("  {}", act),
        None => "".to_string(),
    };

    let formatted = if is_root {
        format!("{}{}", node_str, action_str)
    } else {
        format!("{}{}{}{}", prefix, if is_last { "└── " } else { "├── " }, node_str, action_str)
    };

    let is_selected = node.rel_path == selected_rel_path;
    lines.push((formatted, node.rel_path.clone(), is_selected));

    if is_root || expanded_paths.contains(&node.rel_path) {
        let new_prefix = if is_root {
            "".to_string()
        } else {
            format!("{}{}", prefix, if is_last { "    " } else { "│   " })
        };
        let child_count = node.children.len();
        for (idx, (_, child)) in node.children.iter().enumerate() {
            flatten_tree(
                child,
                &new_prefix,
                idx == child_count - 1,
                false,
                expanded_paths,
                selected_rel_path,
                lines,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_node_insert() {
        let mut root = TreeNode::new("dest".to_string(), "".to_string(), true);
        root.insert(&["subdir", "subsubdir"], true, None);
        root.insert(&["subdir", "file.txt"], false, Some("[+ MOVE]".to_string()));

        assert_eq!(root.name, "dest");
        assert_eq!(root.rel_path, "");
        assert!(root.is_dir);

        let subdir = root.children.get("subdir").unwrap();
        assert_eq!(subdir.name, "subdir");
        assert_eq!(subdir.rel_path, "subdir");
        assert!(subdir.is_dir);

        let subsubdir = subdir.children.get("subsubdir").unwrap();
        assert_eq!(subsubdir.name, "subsubdir");
        assert_eq!(subsubdir.rel_path, "subdir/subsubdir");
        assert!(subsubdir.is_dir);

        let file = subdir.children.get("file.txt").unwrap();
        assert_eq!(file.name, "file.txt");
        assert_eq!(file.rel_path, "subdir/file.txt");
        assert!(!file.is_dir);
        assert_eq!(file.action.as_deref(), Some("[+ MOVE]"));
    }
}


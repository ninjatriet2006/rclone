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
        ExplorerPane {
            remote: remote.to_string(),
            path: if remote.is_empty() {
                crate::app_config::get_home_dir()
            } else {
                "".to_string()
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
    CopyNative { src: String, dest: String },
    CopyLocalTransfer { src: String, dest: String },
    DeleteNative { target: String, is_dir: bool },
    DeleteIndividual { target: String },
    RenameCopyDelete { src: String, dest: String, is_dir: bool },
    RenameLocalTransfer { src: String, dest: String, is_dir: bool },
    CleanupCloud { fs: String },
    Rmdir { fs: String, remote: String },
    Rmdirs { fs: String, remote: String },
    Cancel,
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
    SyncConfirm,
    SelectRemote {
        remotes: Vec<String>,
        selected_idx: usize,
    },
    ConfirmFallback {
        title: String,
        options: Vec<String>,
        selected_idx: usize,
        actions: Vec<FallbackAction>,
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
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
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
    pub error_message: Option<String>,
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
            error_message: None,
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
        ExplorerPopup::SyncConfirm => {
            let src_pane = match state.active_pane {
                ActivePane::Left => &state.left_pane,
                ActivePane::Right => &state.right_pane,
            };
            let dest_pane = match state.active_pane {
                ActivePane::Left => &state.right_pane,
                ActivePane::Right => &state.left_pane,
            };
            let src_label = if src_pane.remote.is_empty() { "Local:" } else { &src_pane.remote };
            let dest_label = if dest_pane.remote.is_empty() { "Local:" } else { &dest_pane.remote };
            let msg = crate::lang::translate("exp_sync_msg")
                .replacen("{}", src_label, 1)
                .replacen("{}", &src_pane.path, 1)
                .replacen("{}", dest_label, 1)
                .replacen("{}", &dest_pane.path, 1);
            super::draw_popup(frame, &crate::lang::translate("exp_sync_title"), &msg, 65, 45);
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
            ..
        } => {
            draw_confirm_fallback_popup(frame, title, options, *selected_idx);
        }
        ExplorerPopup::InputRename { old_name, input_buffer, .. } => {
            draw_input_rename(frame, old_name, input_buffer, state.edit_cursor_idx);
        }
        ExplorerPopup::SpecialActionsMenu { selected_idx } => {
            draw_special_actions_menu(frame, *selected_idx);
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
        ExplorerPopup::None => {}
    }

    if let Some(ref err) = state.error_message {
        super::draw_popup(frame, &crate::lang::translate("exp_error_title"), err, 60, 30);
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
                let raw_name = format!("{}{}{}", select_prefix, prefix, item.name);
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

fn draw_confirm_fallback_popup(frame: &mut Frame, title: &str, options: &[String], selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(65, 50, size);
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Warning header
            Constraint::Min(4),    // Options list
        ])
        .split(area);

    // Render warning title / header
    let header = Paragraph::new(Line::from(vec![
        Span::styled("⚠️ CẢNH BÁO: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Tính năng này không hỗ trợ trực tiếp bởi Remote!"),
    ]))
    .block(Block::default().borders(Borders::BOTTOM))
    .style(Style::default().fg(Color::White));
    frame.render_widget(header, chunks[0]);

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

fn draw_special_actions_menu(frame: &mut Frame, selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(50, 45, size);
    frame.render_widget(Clear, area);

    let options = vec![
        crate::lang::translate("exp_special_link"),
        crate::lang::translate("exp_special_hash"),
        crate::lang::translate("exp_special_cleanup"),
        crate::lang::translate("exp_special_rmdir"),
        crate::lang::translate("exp_special_rmdirs"),
        crate::lang::translate("exp_special_cryptdecode"),
        crate::lang::translate("exp_special_archive"),
        crate::lang::translate("exp_special_close"),
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
                        format!("{}{}", prefix, item.name),
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

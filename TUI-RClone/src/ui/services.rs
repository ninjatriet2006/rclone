use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub enum ServiceType {
    Mount,
    NfsMount,
    WebGui,
    Serve,
}



#[derive(Debug, Clone, PartialEq)]
pub enum ServicesWizardState {
    None,
    AskMode {
        service_type: ServiceType,
        selected_idx: usize,
    },
    SelectRemote {
        service_type: ServiceType,
        remotes: Vec<String>,
        selected_idx: usize,
        is_simple_terminal: bool,
        is_simple_gui: bool,
    },
    InputPath {
        service_type: ServiceType,
        remote: String,
        input_buffer: String,
        is_simple_terminal: bool,
    },
    GuiSelectPath {
        service_type: ServiceType,
        remote: String,
        current_path: String,
        items: Vec<super::explorer::FileItem>,
        selected_idx: usize,
        loading: bool,
        error_msg: Option<String>,
        creating_folder: Option<String>,
    },
    GuiSelectLocalPath {
        service_type: ServiceType,
        remote: String,
        remote_path: String,
        current_path: String,
        items: Vec<super::explorer::FileItem>,
        selected_idx: usize,
        loading: bool,
        error_msg: Option<String>,
        creating_folder: Option<String>,
    },
    SelectProtocol {
        remote: String,
        path: String,
        selected_idx: usize, // 0: http, 1: ftp, 2: webdav, 3: sftp
    },
    AskFlags {
        service_type: ServiceType,
        remote: String,
        path: String,
        protocol: Option<String>,
        flags: Vec<(String, String, String, String)>, // (Tên flag, Câu hỏi, Tùy chọn mặc định, Giá trị hiện tại)
        current_flag_idx: usize,
        input_buffer: String,
        is_simple_terminal: bool,
        is_editing: bool,
    },
    SelectSystemdAction {
        service_name: String,
        file_path: String,
        is_user: bool,
        selected_idx: usize,
    },
    EditSystemdService {
        service_name: String,
        file_path: String,
        is_user: bool,
        fields: Vec<(String, String, String, Vec<String>)>, // (Tên trường, Mô tả, Giá trị, Lựa chọn)
        selected_idx: usize,
        scroll_offset: usize,
        is_editing: bool,
        input_buffer: String,
        active_tab: usize, // 0: Basic, 1: Advanced
        adding_new_key: bool,
        new_key_buffer: String,
    },
    CreateSystemdService {
        fields: Vec<(String, String, String, Vec<String>)>, // (Tên trường, Mô tả, Giá trị, Lựa chọn)
        selected_idx: usize,
        scroll_offset: usize,
        is_editing: bool,
        input_buffer: String,
        active_tab: usize, // 0: Basic, 1: Advanced
        adding_new_key: bool,
        new_key_buffer: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum WizardReturnTarget {
    CreateSystemd {
        fields: Vec<(String, String, String, Vec<String>)>,
        selected_idx: usize,
        scroll_offset: usize,
        active_tab: usize,
        target_field: String,
    },
    EditSystemd {
        service_name: String,
        file_path: String,
        is_user: bool,
        fields: Vec<(String, String, String, Vec<String>)>,
        selected_idx: usize,
        scroll_offset: usize,
        active_tab: usize,
        target_field: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveService {
    pub service_type_str: String, // "Mount", "WebGui", "Serve"
    pub remote: String,
    pub path: String,
    pub pid: u32,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemdServiceInfo {
    pub name: String,
    pub file_path: String,
    pub is_user: bool,
    pub active_status: String,
    pub sub_state: String,
    pub description: String,
}

pub struct ServicesState {
    pub menu_options: Vec<&'static str>,
    pub selected_menu_idx: usize,
    pub wizard: ServicesWizardState,
    pub active_services: Vec<ActiveService>,
    pub selected_active_idx: usize,
    pub active_focus: usize, // 0: focus vào menu, 1: focus vào TUI active, 2: focus vào Systemd services
    pub systemd_services: Vec<SystemdServiceInfo>,
    pub selected_systemd_idx: usize,
    pub error_message: Option<String>,
    pub info_message: Option<String>,
    pub all_remotes: Vec<String>,
    pub selecting_remote: Option<usize>,
    pub edit_cursor_idx: usize,
    pub systemd_wizard_return: Option<WizardReturnTarget>,
}

impl ServicesState {
    pub fn new() -> Self {
        ServicesState {
            menu_options: vec![
                "srv_opt_mount",
                "srv_opt_nfsmount",
                "srv_opt_gui",
                "srv_opt_serve",
            ],
            selected_menu_idx: 0,
            wizard: ServicesWizardState::None,
            active_services: Vec::new(),
            selected_active_idx: 0,
            active_focus: 0,
            systemd_services: Vec::new(),
            selected_systemd_idx: 0,
            error_message: None,
            info_message: None,
            all_remotes: Vec::new(),
            selecting_remote: None,
            edit_cursor_idx: 0,
            systemd_wizard_return: None,
        }
    }

    pub fn next_menu(&mut self) {
        self.selected_menu_idx = (self.selected_menu_idx + 1) % self.menu_options.len();
    }

    pub fn prev_menu(&mut self) {
        if self.selected_menu_idx == 0 {
            self.selected_menu_idx = self.menu_options.len() - 1;
        } else {
            self.selected_menu_idx -= 1;
        }
    }

    pub fn next_active(&mut self) {
        if !self.active_services.is_empty() {
            self.selected_active_idx = (self.selected_active_idx + 1) % self.active_services.len();
        }
    }

    pub fn prev_active(&mut self) {
        if !self.active_services.is_empty() {
            if self.selected_active_idx == 0 {
                self.selected_active_idx = self.active_services.len() - 1;
            } else {
                self.selected_active_idx -= 1;
            }
        }
    }

    pub fn next_systemd(&mut self) {
        if !self.systemd_services.is_empty() {
            self.selected_systemd_idx =
                (self.selected_systemd_idx + 1) % self.systemd_services.len();
        }
    }

    pub fn prev_systemd(&mut self) {
        if !self.systemd_services.is_empty() {
            if self.selected_systemd_idx == 0 {
                self.selected_systemd_idx = self.systemd_services.len() - 1;
            } else {
                self.selected_systemd_idx -= 1;
            }
        }
    }
}

pub fn draw(state: &ServicesState, frame: &mut Frame, area: Rect, fuse_installed: bool) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),
            Constraint::Length(3), // Help bar
        ])
        .split(area);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Menu dịch vụ
            Constraint::Percentage(50), // Các dịch vụ bên phải
        ])
        .split(main_chunks[0]);

    // 1. Vẽ Menu Dịch vụ
    let menu_items: Vec<ListItem> = state
        .menu_options
        .iter()
        .enumerate()
        .map(|(i, option)| {
            let style = if i == state.selected_menu_idx
                && state.active_focus == 0
                && state.wizard == ServicesWizardState::None
            {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Magenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(crate::lang::translate(option)).style(style)
        })
        .collect();

    let menu_block = Block::default()
        .title(Span::styled(
            crate::lang::translate("srv_title_config"),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(
            if state.active_focus == 0 && state.wizard == ServicesWizardState::None {
                Color::Magenta
            } else {
                Color::DarkGray
            },
        ));
    let menu_list = List::new(menu_items).block(menu_block);
    frame.render_widget(menu_list, content_chunks[0]);

    // Chia đôi bảng bên phải theo chiều dọc hoặc giữ 100% nếu là Windows
    let show_systemd = !cfg!(target_os = "windows");
    let right_chunks = if show_systemd {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50), // Dịch vụ chạy ngầm TUI
                Constraint::Percentage(50), // Dịch vụ hệ thống Systemd
            ])
            .split(content_chunks[1])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(100), // Dịch vụ chạy ngầm TUI
            ])
            .split(content_chunks[1])
    };

    // 2. Vẽ danh sách các tiến trình dịch vụ TUI đang hoạt động
    let active_h = right_chunks[0].height.saturating_sub(2) as usize;
    let active_range = super::calculate_scroll_range(
        state.selected_active_idx,
        state.active_services.len(),
        active_h,
    );
    let active_items: Vec<ListItem> = state.active_services[active_range.clone()]
        .iter()
        .enumerate()
        .map(|(relative_idx, s)| {
            let i = active_range.start + relative_idx;
            let style = if i == state.selected_active_idx
                && state.active_focus == 1
                && state.wizard == ServicesWizardState::None
            {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("[{}] ", s.service_type_str),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("PID: {} | ", s.pid)),
                Span::styled(&s.details, Style::default().fg(Color::Cyan)),
            ]);

            ListItem::new(line).style(style)
        })
        .collect();

    let active_block_fg = if state.active_focus == 1 && state.wizard == ServicesWizardState::None {
        Color::Green
    } else {
        Color::DarkGray
    };
    let active_block = Block::default()
        .title(Span::styled(
            crate::lang::translate("srv_title_tui"),
            Style::default()
                .fg(active_block_fg)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(active_block_fg));
    let active_list = List::new(active_items).block(active_block);
    frame.render_widget(active_list, right_chunks[0]);

    if show_systemd {
        // 3. Vẽ danh sách Dịch vụ hệ thống (Systemd)
        let sys_h = right_chunks[1].height.saturating_sub(2) as usize;
        let sys_range = super::calculate_scroll_range(
            state.selected_systemd_idx,
            state.systemd_services.len(),
            sys_h,
        );
        let sys_items: Vec<ListItem> = state.systemd_services[sys_range.clone()]
            .iter()
            .enumerate()
            .map(|(relative_idx, s)| {
                let i = sys_range.start + relative_idx;
                let style = if i == state.selected_systemd_idx
                    && state.active_focus == 2
                    && state.wizard == ServicesWizardState::None
                {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                // Chuyển đổi trạng thái hiển thị thân thiện với người dùng
                let status_text = match s.active_status.as_str() {
                    "active" => "running",
                    "activating" => "activating",
                    "failed" => "failed",
                    _ => "inactive",
                };

                // Icon biểu thị trạng thái
                let (status_icon, status_color) = match status_text {
                    "running" => ("● ", Color::Green),
                    "activating" => ("↻ ", Color::Yellow),
                    "failed" => ("✖ ", Color::Red),
                    _ => ("○ ", Color::DarkGray),
                };

                let level_tag = if s.is_user {
                    crate::lang::translate("srv_tag_user")
                } else {
                    crate::lang::translate("srv_tag_system")
                };
                let level_color = if s.is_user {
                    Color::Magenta
                } else {
                    Color::Blue
                };

                let mut line_spans = vec![
                    Span::styled(status_icon, Style::default().fg(status_color)),
                    Span::styled(
                        format!("{}{} ", s.name, level_tag),
                        Style::default()
                            .fg(level_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                ];

                if !s.description.is_empty() {
                    line_spans.push(Span::raw("| "));
                    line_spans.push(Span::styled(
                        &s.description,
                        Style::default().fg(Color::Cyan),
                    ));
                }

                line_spans.push(Span::raw(format!(" ({})", status_text)));

                ListItem::new(Line::from(line_spans)).style(style)
            })
            .collect();

        let sys_block_fg = if state.active_focus == 2 && state.wizard == ServicesWizardState::None {
            Color::Cyan
        } else {
            Color::DarkGray
        };
        let sys_block = Block::default()
            .title(Span::styled(
                crate::lang::translate("srv_title_systemd"),
                Style::default()
                    .fg(sys_block_fg)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(sys_block_fg));
        let sys_list = List::new(sys_items).block(sys_block);
        frame.render_widget(sys_list, right_chunks[1]);
    }

    // Help Bar
    let help_text = match &state.wizard {
        ServicesWizardState::None => match state.active_focus {
            0 => crate::lang::translate("srv_help_t0"),
            1 => crate::lang::translate("srv_help_t1"),
            _ => crate::lang::translate("srv_help_t2"),
        },
        ServicesWizardState::SelectSystemdAction { .. } => {
            crate::lang::translate("srv_help_action")
        }
        ServicesWizardState::EditSystemdService { .. }
        | ServicesWizardState::CreateSystemdService { .. } => {
            crate::lang::translate("srv_help_edit")
        }
        _ => crate::lang::translate("srv_help_general"),
    };
    let help_paragraph = Paragraph::new(super::parse_help_line(&help_text))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(help_paragraph, main_chunks[1]);

    // Vẽ Wizard Popups
    match &state.wizard {
        ServicesWizardState::AskMode {
            service_type,
            selected_idx,
        } => {
            draw_ask_mode(frame, service_type, *selected_idx);
        }
        ServicesWizardState::SelectRemote {
            remotes,
            selected_idx,
            service_type,
            ..
        } => {
            draw_select_remote(frame, remotes, *selected_idx, service_type);
        }
        ServicesWizardState::InputPath {
            service_type,
            remote,
            input_buffer,
            ..
        } => {
            draw_input_path(frame, service_type, remote, input_buffer, state.edit_cursor_idx);
        }
        ServicesWizardState::GuiSelectPath {
            service_type,
            remote,
            current_path,
            items,
            selected_idx,
            loading,
            error_msg,
            creating_folder,
            ..
        } => {
            draw_gui_select_path(
                frame,
                service_type,
                remote,
                current_path,
                items,
                *selected_idx,
                *loading,
                error_msg.as_deref(),
                creating_folder.as_deref(),
            );
        }
        ServicesWizardState::GuiSelectLocalPath {
            service_type,
            current_path,
            items,
            selected_idx,
            loading,
            error_msg,
            creating_folder,
            ..
        } => {
            draw_gui_select_path(
                frame,
                service_type,
                &crate::lang::translate("srv_local_system"),
                current_path,
                items,
                *selected_idx,
                *loading,
                error_msg.as_deref(),
                creating_folder.as_deref(),
            );
        }
        ServicesWizardState::SelectProtocol {
            remote,
            path,
            selected_idx,
        } => {
            draw_select_protocol(frame, remote, path, *selected_idx);
        }
        ServicesWizardState::AskFlags {
            service_type,
            remote,
            path,
            protocol,
            flags,
            current_flag_idx,
            input_buffer,
            is_simple_terminal: _,
            is_editing,
        } => {
            draw_ask_flags(
                frame,
                service_type,
                remote,
                path,
                protocol,
                flags,
                *current_flag_idx,
                input_buffer,
                *is_editing,
                state.edit_cursor_idx,
            );
        }
        ServicesWizardState::SelectSystemdAction {
            service_name,
            is_user,
            selected_idx,
            ..
        } => {
            draw_select_systemd_action(frame, service_name, *is_user, *selected_idx);
        }
        ServicesWizardState::EditSystemdService {
            service_name,
            fields,
            selected_idx,
            scroll_offset,
            is_editing,
            input_buffer,
            active_tab,
            adding_new_key,
            new_key_buffer,
            ..
        } => {
            draw_edit_systemd_service_wizard(
                frame,
                &format!("CHỈNH SỬA DỊCH VỤ SYSTEMD: {}", service_name),
                fields,
                *selected_idx,
                *scroll_offset,
                *is_editing,
                input_buffer,
                *active_tab,
                *adding_new_key,
                new_key_buffer,
                state.selecting_remote,
                &state.all_remotes,
                state.edit_cursor_idx,
                fuse_installed,
            );
        }
        ServicesWizardState::CreateSystemdService {
            fields,
            selected_idx,
            scroll_offset,
            is_editing,
            input_buffer,
            active_tab,
            adding_new_key,
            new_key_buffer,
            ..
        } => {
            draw_edit_systemd_service_wizard(
                frame,
                "TẠO MỚI DỊCH VỤ SYSTEMD (RCLONE MOUNT)",
                fields,
                *selected_idx,
                *scroll_offset,
                *is_editing,
                input_buffer,
                *active_tab,
                *adding_new_key,
                new_key_buffer,
                state.selecting_remote,
                &state.all_remotes,
                state.edit_cursor_idx,
                fuse_installed,
            );
        }
        ServicesWizardState::None => {}
    }

    if let Some(ref err) = state.error_message {
        super::draw_popup(frame, " LỖI DỊCH VỤ ", err, 60, 30);
    } else if let Some(ref info) = state.info_message {
        super::draw_popup(
            frame,
            &crate::lang::translate("srv_info_title"),
            info,
            60,
            30,
        );
    }
}

fn draw_select_remote(
    frame: &mut Frame,
    remotes: &[String],
    selected_idx: usize,
    service_type: &ServiceType,
) {
    let size = frame.size();
    let area = centered_rect(55, 60, size);
    frame.render_widget(Clear, area);

    let local_desc = crate::lang::translate("srv_local_desc");
    let mut items = vec![ListItem::new(local_desc.clone())];
    items.extend(remotes.iter().enumerate().map(|(i, remote)| {
        let style = if i + 1 == selected_idx {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        ListItem::new(crate::lang::translate("srv_cloud_desc").replace("{}", remote)).style(style)
    }));

    if selected_idx == 0 {
        items[0] = ListItem::new(local_desc).style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        );
    }

    let title_prefix = match service_type {
        ServiceType::Mount => "MOUNT",
        ServiceType::NfsMount => "NFS MOUNT",
        ServiceType::Serve => "SHARE",
        ServiceType::WebGui => "WEB GUI",
    };

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("srv_select_source_title").replace("{}", title_prefix),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let height = area.height.saturating_sub(2) as usize;
    let range = super::calculate_scroll_range(selected_idx, items.len(), height);
    let visible_items: Vec<ListItem> = items
        .into_iter()
        .skip(range.start)
        .take(range.end - range.start)
        .collect();

    let list = List::new(visible_items).block(block);
    frame.render_widget(list, area);
}

fn draw_input_path(
    frame: &mut Frame,
    service_type: &ServiceType,
    remote: &str,
    input_buffer: &str,
    cursor_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(60, 30, size);
    frame.render_widget(Clear, area);

    let prompt = match service_type {
        ServiceType::Mount | ServiceType::NfsMount => {
            crate::lang::translate("srv_mount_point_prompt")
        }
        ServiceType::Serve => crate::lang::translate("srv_share_path_prompt"),
        _ => "".to_string(),
    };

    let local_system_label = crate::lang::translate("srv_local_system");
    let mut spans = vec![
        Span::styled("> ", Style::default().fg(Color::Magenta)),
    ];
    spans.extend(super::make_input_spans_with_cursor(input_buffer, cursor_idx, Color::White, Color::DarkGray));

    let text = vec![
        Line::from(vec![
            Span::raw(crate::lang::translate("srv_selected_source")),
            Span::styled(
                if remote.is_empty() {
                    &local_system_label
                } else {
                    remote
                },
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(prompt),
        Line::from(spans),
    ];

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("srv_mount_point_title"),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_select_protocol(frame: &mut Frame, remote: &str, path: &str, selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(55, 35, size);
    frame.render_widget(Clear, area);

    let protocols = vec![
        crate::lang::translate("srv_proto_http"),
        crate::lang::translate("srv_proto_ftp"),
        crate::lang::translate("srv_proto_webdav"),
        crate::lang::translate("srv_proto_sftp"),
    ];

    let items: Vec<ListItem> = protocols
        .iter()
        .enumerate()
        .map(|(i, proto)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Magenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(proto.clone()).style(style)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("srv_select_proto_title").replace(
                "{}{}",
                &format!(
                    "{}{}",
                    if remote.is_empty() { "Local:" } else { remote },
                    path
                ),
            ),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn draw_ask_flags(
    frame: &mut Frame,
    _service_type: &ServiceType,
    remote: &str,
    path: &str,
    protocol: &Option<String>,
    flags: &[(String, String, String, String)],
    current_flag_idx: usize,
    input_buffer: &str,
    is_editing: bool,
    cursor_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(65, 45, size);
    frame.render_widget(Clear, area);

    let (flag_name, question, default_val, _) = &flags[current_flag_idx];

    let local_system_label = crate::lang::translate("srv_local_system");
    let mut info_spans = vec![
        Span::raw(crate::lang::translate("srv_config_for")),
        Span::styled(
            if remote.is_empty() {
                &local_system_label
            } else {
                remote
            },
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if !path.is_empty() {
        info_spans.push(Span::raw(format!(" (Path: {})", path)));
    }
    if let Some(proto) = protocol {
        info_spans.push(Span::raw(format!(" | Protocol: {}", proto)));
    }

    let progress_text = crate::lang::translate("srv_wizard_progress")
        .replacen("{}", &format!("{}", current_flag_idx + 1), 1)
        .replacen("{}", &format!("{}", flags.len()), 1);

        let mut input_spans = vec![
            Span::raw(crate::lang::translate("srv_wizard_input_prompt")),
            Span::styled(
                default_val,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("]): "),
        ];
        if is_editing {
            input_spans.extend(super::make_input_spans_with_cursor(input_buffer, cursor_idx, Color::White, Color::DarkGray));
        } else {
            input_spans.push(Span::styled(
                input_buffer,
                Style::default().fg(Color::White).bg(Color::DarkGray),
            ));
        }

        let text = vec![
            Line::from(info_spans),
            Line::from(progress_text),
            Line::from("------------------------------------------------------------------"),
            Line::from(""),
            Line::from(Span::styled(
                crate::lang::translate("srv_wizard_flag").replace("{}", flag_name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(question.as_str()),
            Line::from(""),
            Line::from(input_spans),
        ];

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("srv_wizard_title"),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

use super::centered_rect;

fn draw_ask_mode(frame: &mut Frame, service_type: &ServiceType, selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(65, 30, size);
    frame.render_widget(Clear, area);

    let options = vec![
        crate::lang::translate("srv_mode_terminal"),
        crate::lang::translate("srv_mode_gui"),
        crate::lang::translate("srv_mode_advanced"),
    ];

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Magenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(opt.clone()).style(style)
        })
        .collect();

    let title_prefix = match service_type {
        ServiceType::Mount => "MOUNT",
        ServiceType::NfsMount => "NFS MOUNT",
        ServiceType::Serve => "SHARE",
        ServiceType::WebGui => "WEB GUI",
    };

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("srv_select_mode_title").replace("{}", title_prefix),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn draw_gui_select_path(
    frame: &mut Frame,
    service_type: &ServiceType,
    remote: &str,
    current_path: &str,
    items: &[super::explorer::FileItem],
    selected_idx: usize,
    loading: bool,
    error_msg: Option<&str>,
    creating_folder: Option<&str>,
) {
    let size = frame.size();
    let area = centered_rect(75, 75, size);
    frame.render_widget(Clear, area);

    let help_text = crate::lang::translate("srv_browser_help");
    let available_width = area.width.saturating_sub(2) as usize;
    let needed_lines = super::estimate_wrapped_lines(&help_text, available_width);

    let mut help_height = needed_lines.min(3);
    if help_height > 1 {
        let temp_help_bar_height = help_height + 2;
        let list_height = area.height.saturating_sub(3 + temp_help_bar_height as u16);
        let visible_files_height = list_height.saturating_sub(2);
        if visible_files_height <= 8 {
            help_height = 1;
        }
    }
    if help_height == 0 {
        help_height = 1;
    }
    let help_bar_height = help_height + 2;

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),                      // Info bar
            Constraint::Min(5),                         // List of directories
            Constraint::Length(help_bar_height as u16), // Help bar
        ])
        .split(area);

    let title_prefix = match service_type {
        ServiceType::Mount => "MOUNT",
        ServiceType::NfsMount => "NFS MOUNT",
        ServiceType::Serve => "SHARE",
        ServiceType::WebGui => "WEB GUI",
    };

    // 1. Info block
    let info_text = crate::lang::translate("srv_browser_info")
        .replacen("{}", remote, 1)
        .replacen("{}", current_path, 1);
    let info_block = Block::default()
        .title(Span::styled(
            crate::lang::translate("srv_browser_title").replace("{}", title_prefix),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let info_p = Paragraph::new(info_text)
        .block(info_block)
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(info_p, layout[0]);

    // 2. Directory List or Loading/Error
    let list_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    if loading {
        let loading_p = Paragraph::new(crate::lang::translate("srv_browser_loading"))
            .block(list_block)
            .style(Style::default().fg(Color::Yellow))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(loading_p, layout[1]);
    } else if let Some(err) = error_msg {
        let err_p = Paragraph::new(format!("LỖI: {}\n\nNhấn [Backspace] để quay lại thư mục cha.", err))
            .block(list_block)
            .style(Style::default().fg(Color::Red))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(err_p, layout[1]);
    } else if items.is_empty() {
        let empty_p = Paragraph::new(crate::lang::translate("srv_browser_empty"))
            .block(list_block)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(empty_p, layout[1]);
    } else {
        let height = layout[1].height.saturating_sub(2) as usize;
        let range = super::calculate_scroll_range(selected_idx, items.len(), height);
        let visible_items = &items[range.clone()];

        let list_items: Vec<ListItem> = visible_items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let global_idx = range.start + i;
                let style = if global_idx == selected_idx {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Magenta)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(format!(" 📁 {}", item.name)).style(style)
            })
            .collect();
        let list = List::new(list_items).block(list_block);
        frame.render_widget(list, layout[1]);
    }

    // 3. Help block
    let help_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let help_p = Paragraph::new(super::parse_help_line(&help_text))
        .block(help_block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(help_p, layout[2]);

    // 4. Create Folder Overlay
    if let Some(buf) = creating_folder {
        let overlay_area = centered_rect(50, 25, size);
        frame.render_widget(Clear, overlay_area);
        let overlay_block = Block::default()
            .title(Span::styled(
                crate::lang::translate("srv_browser_new_title"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let overlay_text = vec![
            Line::from(crate::lang::translate("srv_browser_new_prompt")),
            Line::from(Span::styled(
                format!(" > {}", buf),
                Style::default().fg(Color::White).bg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(crate::lang::translate("srv_browser_new_help")),
        ];
        let overlay_p = Paragraph::new(overlay_text).block(overlay_block);
        frame.render_widget(overlay_p, overlay_area);
    }
}

fn draw_select_systemd_action(
    frame: &mut Frame,
    service_name: &str,
    is_user: bool,
    selected_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(50, 40, size);
    frame.render_widget(Clear, area);

    let level = if is_user {
        crate::lang::translate("srv_action_level_user")
    } else {
        crate::lang::translate("srv_action_level_system")
    };
    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("srv_action_title")
                .replacen("{}", service_name, 1)
                .replacen("{}", &level, 1),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let options = vec![
        crate::lang::translate("srv_action_start"),
        crate::lang::translate("srv_action_stop"),
        crate::lang::translate("srv_action_restart"),
        crate::lang::translate("srv_action_enable"),
        crate::lang::translate("srv_action_disable"),
        crate::lang::translate("srv_action_edit"),
    ];

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, option)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(option.clone()).style(style)
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn draw_edit_systemd_service_wizard(
    frame: &mut Frame,
    title: &str,
    fields: &[(String, String, String, Vec<String>)],
    selected_idx: usize,
    scroll_offset: usize,
    is_editing: bool,
    input_buffer: &str,
    active_tab: usize,
    adding_new_key: bool,
    new_key_buffer: &str,
    _selecting_remote: Option<usize>,
    _remotes: &[String],
    cursor_idx: usize,
    fuse_installed: bool,
) {
    let size = frame.size();
    let area = centered_rect(65, 75, size);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    frame.render_widget(&block, area);

    let inner_area = block.inner(area);

    // Lọc danh sách fields theo tab: tab 0 là Cơ bản (trường ảo bắt đầu bằng '_'), tab 1 là Nâng cao (khóa systemd thực tế)
    let filtered_fields: Vec<&(String, String, String, Vec<String>)> = fields
        .iter()
        .filter(|(name, _, _, _)| {
            if active_tab == 0 {
                name.starts_with('_')
            } else {
                !name.starts_with('_')
            }
        })
        .collect();

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Tab bar
            Constraint::Length(1), // Divider line
            Constraint::Min(3),    // Fields list
        ])
        .split(inner_area);

    // Vẽ Tab Bar
    let basic_style = if active_tab == 0 {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    };
    let adv_style = if active_tab == 1 {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    };

    let tab_line = Line::from(vec![
        Span::styled(
            crate::lang::translate("srv_systemd_edit_tab_basic"),
            basic_style,
        ),
        Span::raw("   "),
        Span::styled(
            crate::lang::translate("srv_systemd_edit_tab_adv"),
            adv_style,
        ),
        Span::raw(crate::lang::translate("srv_systemd_edit_tab_help")),
    ]);
    frame.render_widget(Paragraph::new(tab_line), inner_chunks[0]);

    // Vẽ đường chia ngang
    frame.render_widget(
        Paragraph::new("─".repeat(inner_chunks[1].width as usize))
            .style(Style::default().fg(Color::DarkGray)),
        inner_chunks[1],
    );

    let height = inner_chunks[2].height.saturating_sub(4) as usize; // trừ các nút bấm

    let mut items: Vec<ListItem> = filtered_fields
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(height)
        .map(|(i, (name, _, value, _choices))| {
            let (friendly_name, friendly_desc) = translate_systemd_field(name);

            let display_val = if i == selected_idx && is_editing {
                input_buffer.to_string()
            } else {
                value.to_string()
            };

            let line = if i == selected_idx {
                let cursor = if is_editing { " 📝 " } else { " >> " };
                let bg = if is_editing {
                    Color::DarkGray
                } else {
                    Color::Cyan
                };
                let fg = if is_editing {
                    Color::White
                } else {
                    Color::Black
                };

                let mut spans = vec![
                    Span::styled(cursor, Style::default().fg(Color::Red)),
                    Span::styled(
                        format!("{}: ", friendly_name),
                        Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
                    ),
                ];
                if is_editing {
                    spans.extend(super::make_input_spans_with_cursor(&display_val, cursor_idx, fg, bg));
                } else {
                    spans.push(Span::styled(display_val, Style::default().fg(fg).bg(bg)));
                }
                if name == "_remote" {
                    spans.push(Span::styled(
                        crate::lang::translate("srv_insert_gui_hint"),
                        Style::default()
                            .fg(Color::Yellow)
                            .bg(bg)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else if name == "_mount_path" {
                    if fuse_installed {
                        spans.push(Span::styled(
                            crate::lang::translate("srv_insert_gui_hint"),
                            Style::default()
                                .fg(Color::Yellow)
                                .bg(bg)
                                .add_modifier(Modifier::BOLD),
                        ));
                    } else {
                        spans.push(Span::styled(
                            crate::lang::translate("srv_mount_fuse_missing_hint"),
                            Style::default()
                                .fg(Color::Red)
                                .bg(bg)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                }
                spans.push(Span::styled(
                    format!(" - ({})", friendly_desc),
                    Style::default().fg(fg).bg(bg),
                ));
                Line::from(spans)
            } else {
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("{}: ", friendly_name),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(display_val),
                    Span::styled(
                        format!(" - ({})", friendly_desc),
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            };
            ListItem::new(line)
        })
        .collect();

    items.push(ListItem::new(Line::raw("")));

    let save_idx = filtered_fields.len();
    let cancel_idx = filtered_fields.len() + 1;

    // Nút Lưu
    let save_style = if selected_idx == save_idx {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::LightGreen)
    };
    items.push(ListItem::new(Line::from(vec![
        Span::raw(if selected_idx == save_idx {
            " >> "
        } else {
            "    "
        }),
        Span::styled(crate::lang::translate("srv_systemd_edit_save"), save_style),
    ])));

    // Nút Hủy
    let cancel_style = if selected_idx == cancel_idx {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::LightRed)
    };
    items.push(ListItem::new(Line::from(vec![
        Span::raw(if selected_idx == cancel_idx {
            " >> "
        } else {
            "    "
        }),
        Span::styled(
            crate::lang::translate("srv_systemd_edit_cancel"),
            cancel_style,
        ),
    ])));

    // Thêm dòng lưu ý/mẹo
    let tip = if is_editing
        && filtered_fields.get(selected_idx).map(|f| f.0.as_str()) == Some("_remote")
    {
        crate::lang::translate_tip("tip_select_remote")
    } else {
        crate::lang::translate_tip("unikey_tip")
    };
    items.push(ListItem::new(Line::from(vec![
        Span::raw("    "),
        Span::styled(
            tip,
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ),
    ])));

    let list = List::new(items);
    frame.render_widget(list, inner_chunks[2]);

    if adding_new_key {
        let overlay_area = centered_rect(50, 25, size);
        frame.render_widget(Clear, overlay_area);
        let overlay_block = Block::default()
            .title(Span::styled(
                crate::lang::translate("srv_systemd_add_param_title"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let overlay_text = vec![
            Line::from(crate::lang::translate("srv_systemd_add_param_prompt")),
            Line::from(Span::styled(
                format!(" > {}", new_key_buffer),
                Style::default().fg(Color::White).bg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(crate::lang::translate("srv_systemd_add_param_help")),
        ];
        let overlay_p = Paragraph::new(overlay_text).block(overlay_block);
        frame.render_widget(overlay_p, overlay_area);
    }
}

fn translate_systemd_field(name: &str) -> (String, String) {
    let raw_key = name.replace('[', "").replace(']', "").replace(' ', "_");
    let name_key = format!("sys_field_{}_name", raw_key);
    let desc_key = format!("sys_field_{}_desc", raw_key);
    let friendly_name = crate::lang::translate(&name_key);
    let friendly_desc = crate::lang::translate(&desc_key);

    // Fallbacks if translation keys don't exist
    let fallback_name = if name.starts_with('[') {
        if let Some(pos) = name.find(']') {
            let sec = &name[1..pos];
            let key = name[pos + 1..].trim();
            format!("[{}] {}", sec, key)
        } else {
            name.to_string()
        }
    } else {
        match name {
            "_service_name" => "Tên dịch vụ".to_string(),
            "_service_level" => "Loại dịch vụ".to_string(),
            "_remote" => "Cloud Remote (Nguồn)".to_string(),
            "_mount_path" => "Đường dẫn Mount cục bộ".to_string(),
            "_description" => "Mô tả dịch vụ".to_string(),
            "_user" => "Tài khoản chạy".to_string(),
            _ => name.to_string(),
        }
    };

    let fallback_desc = if name.starts_with('[') {
        if let Some(pos) = name.find(']') {
            let sec = &name[1..pos];
            let key = name[pos + 1..].trim();
            format!("Systemd key [{}] {}", sec, key)
        } else {
            name.to_string()
        }
    } else {
        match name {
            "_service_name" => "Tên không kèm đuôi .service (ví dụ: rclone-torrent)".to_string(),
            "_service_level" => "User (Cá nhân) hoặc System (Hệ thống)".to_string(),
            "_remote" => "Nhập Remote (ví dụ: Main: hoặc Main:ThưMục)".to_string(),
            "_mount_path" => "Đường dẫn thư mục trên máy tính của bạn".to_string(),
            "_description" => "Mô tả ngắn gọn về dịch vụ".to_string(),
            "_user" => "Tên tài khoản Linux chạy dịch vụ này".to_string(),
            _ => name.to_string(),
        }
    };

    let final_name = if friendly_name == name_key {
        fallback_name
    } else {
        friendly_name
    };
    let final_desc = if friendly_desc == desc_key {
        fallback_desc
    } else {
        friendly_desc
    };
    (final_name, final_desc)
}

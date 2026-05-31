use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

#[derive(Debug, Clone, PartialEq)]
pub enum WizardState {
    None,
    SelectProviders {
        providers: Vec<(String, String, bool)>, // (Name, Description, Checked)
        selected_idx: usize,
        scroll_offset: usize,
    },
    InputRemoteName {
        provider: String,
        input_buffer: String,
        selected_providers: Vec<String>, // Danh sách các provider tiếp theo cần cấu hình
    },
    SelectAuthMode {
        provider: String,
        remote_name: String,
        selected_idx: usize, // 0: Simple OAuth, 1: Headless OAuth, 2: Advanced Setup
        selected_providers: Vec<String>,
    },
    HeadlessOAuthInput {
        provider: String,
        remote_name: String,
        client_id: String,
        client_secret: String,
        token_input: String,
        focused_idx: usize, // 0: client_id, 1: client_secret, 2: token_input
        selected_providers: Vec<String>,
    },
    SimpleOAuthLoop {
        provider: String,
        remote_name: String,
        auth_url: String,
        selected_providers: Vec<String>,
    },
    AdvancedSetup {
        provider: String,
        remote_name: String,
        fields: Vec<(String, String, String, Vec<String>)>, // (Tên trường, Mô tả, Giá trị, Lựa chọn)
        selected_field_idx: usize,
        scroll_offset: usize,
        is_editing: bool,
        input_buffer: String,
        selected_providers: Vec<String>,
        active_tab: usize,
    },
    EditSetup {
        remote_name: String,
        provider: String,
        fields: Vec<(String, String, String, Vec<String>)>, // (Tên trường, Mô tả, Giá trị, Lựa chọn)
        selected_idx: usize,
        scroll_offset: usize,
        is_editing: bool,
        input_buffer: String,
        adding_new_key: bool,
        new_key_buffer: String,
        active_tab: usize,
    },
}

pub struct ConnectionState {
    pub remotes: Vec<String>,
    pub selected_idx: usize,
    pub wizard: WizardState,
    pub error_message: Option<String>,
    pub info_message: Option<String>,
    pub remote_statuses: std::collections::HashMap<String, String>,
}

impl ConnectionState {
    pub fn new() -> Self {
        ConnectionState {
            remotes: Vec::new(),
            selected_idx: 0,
            wizard: WizardState::None,
            error_message: None,
            info_message: None,
            remote_statuses: std::collections::HashMap::new(),
        }
    }

    pub fn next(&mut self) {
        if !self.remotes.is_empty() {
            self.selected_idx = (self.selected_idx + 1) % self.remotes.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.remotes.is_empty() {
            if self.selected_idx == 0 {
                self.selected_idx = self.remotes.len() - 1;
            } else {
                self.selected_idx -= 1;
            }
        }
    }
}

pub fn draw(state: &ConnectionState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(3), // Help bar
        ])
        .split(area);

    // Vẽ danh sách kết nối hiện có
    let items: Vec<ListItem> = state
        .remotes
        .iter()
        .enumerate()
        .map(|(i, remote)| {
            let style = if i == state.selected_idx && state.wizard == WizardState::None {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let status = state
                .remote_statuses
                .get(remote)
                .cloned()
                .unwrap_or_else(|| crate::lang::translate("status_unchecked"));
            let text = format!("  [Cloud] -> {:<25} | {}", remote, status);
            ListItem::new(text).style(style)
        })
        .collect();

    let list_title = crate::lang::translate("conn_title");
    let list_block = Block::default()
        .title(Span::styled(
            list_title,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let list = List::new(items).block(list_block);
    frame.render_widget(list, chunks[0]);

    // Help Bar
    let help_text = match &state.wizard {
        WizardState::None => {
            crate::lang::translate("conn_help_navigation")
        }
        WizardState::EditSetup {
            is_editing,
            adding_new_key,
            ..
        } => {
            if *is_editing || *adding_new_key {
                crate::lang::translate("help_editing")
            } else {
                crate::lang::translate("help_navigation")
            }
        }
        _ => {
            crate::lang::translate("help_general")
        }
    };
    let help_paragraph = Paragraph::new(super::parse_help_line(&help_text))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(help_paragraph, chunks[1]);

    // Vẽ Wizard Popup nếu đang trong tiến trình Add Remote
    match &state.wizard {
        WizardState::SelectProviders {
            providers,
            selected_idx,
            scroll_offset,
        } => {
            draw_select_providers_wizard(frame, providers, *selected_idx, *scroll_offset);
        }
        WizardState::InputRemoteName {
            provider,
            input_buffer,
            ..
        } => {
            draw_input_remote_name_wizard(frame, provider, input_buffer);
        }
        WizardState::SelectAuthMode {
            provider,
            remote_name,
            selected_idx,
            ..
        } => {
            draw_select_auth_mode_wizard(frame, provider, remote_name, *selected_idx);
        }
        WizardState::HeadlessOAuthInput {
            provider,
            remote_name,
            client_id,
            client_secret,
            token_input,
            focused_idx,
            ..
        } => {
            draw_headless_oauth_wizard(frame, provider, remote_name, client_id, client_secret, token_input, *focused_idx);
        }
        WizardState::SimpleOAuthLoop {
            provider,
            remote_name,
            auth_url,
            ..
        } => {
            draw_simple_oauth_wizard(frame, provider, remote_name, auth_url);
        }
        WizardState::AdvancedSetup {
            provider,
            remote_name,
            fields,
            selected_field_idx,
            scroll_offset,
            is_editing,
            input_buffer,
            active_tab,
            ..
        } => {
            draw_advanced_setup_wizard(
                frame,
                provider,
                remote_name,
                fields,
                *selected_field_idx,
                *scroll_offset,
                *is_editing,
                input_buffer,
                *active_tab,
            );
        }
        WizardState::EditSetup {
            remote_name,
            provider,
            fields,
            selected_idx,
            scroll_offset,
            is_editing,
            input_buffer,
            adding_new_key,
            new_key_buffer,
            active_tab,
        } => {
            draw_edit_setup_wizard(
                frame,
                remote_name,
                provider,
                fields,
                *selected_idx,
                *scroll_offset,
                *is_editing,
                input_buffer,
                *adding_new_key,
                new_key_buffer,
                *active_tab,
            );
        }
        WizardState::None => {}
    }



    // Vẽ thông báo lỗi/thông tin nếu có
    if let Some(ref err) = state.error_message {
        super::draw_popup(frame, &crate::lang::translate("conn_error_title"), err, 60, 30);
    } else if let Some(ref info) = state.info_message {
        super::draw_popup(frame, &crate::lang::translate("conn_info_title"), info, 60, 35);
    }
}

fn draw_select_providers_wizard(
    frame: &mut Frame,
    providers: &[(String, String, bool)],
    selected_idx: usize,
    scroll_offset: usize,
) {
    let size = frame.size();
    let area = centered_rect(65, 75, size);
    frame.render_widget(Clear, area);

    let height = area.height.saturating_sub(2) as usize; // trừ biên của block

    let items: Vec<ListItem> = providers
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(height)
        .map(|(i, (name, desc, checked))| {
            let checkbox = if *checked { "[X]" } else { "[ ]" };
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {} {} - {}", checkbox, name, desc)).style(style)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("conn_wizard_provider_title"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn draw_input_remote_name_wizard(frame: &mut Frame, provider: &str, input_buffer: &str) {
    let size = frame.size();
    let area = centered_rect(50, 25, size);
    frame.render_widget(Clear, area);

    let text = vec![
        Line::from(vec![
            Span::raw(crate::lang::translate("conn_wizard_provider_configuring")),
            Span::styled(
                provider,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(crate::lang::translate("conn_wizard_name_prompt")),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Green)),
            Span::styled(
                input_buffer,
                Style::default().fg(Color::White).bg(Color::DarkGray),
            ),
        ]),
    ];

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("conn_wizard_name_title"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_select_auth_mode_wizard(
    frame: &mut Frame,
    _provider: &str,
    remote_name: &str,
    selected_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(55, 35, size);
    frame.render_widget(Clear, area);

    let modes = vec![
        crate::lang::translate("conn_wizard_auth_simple"),
        crate::lang::translate("conn_wizard_auth_headless"),
        crate::lang::translate("conn_wizard_auth_advanced"),
    ];

    let items: Vec<ListItem> = modes
        .iter()
        .enumerate()
        .map(|(i, mode)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", mode)).style(style)
        })
        .collect();

    let title_raw = crate::lang::translate("conn_wizard_auth_mode_title");
    let title_fmt = format!(" {} ", title_raw.replace("{}", remote_name));
    let block = Block::default()
        .title(Span::styled(
            title_fmt,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn draw_simple_oauth_wizard(frame: &mut Frame, provider: &str, remote_name: &str, auth_url: &str) {
    let size = frame.size();
    let area = centered_rect(60, 40, size);
    frame.render_widget(Clear, area);

    let text = vec![
        Line::from(vec![
            Span::raw(crate::lang::translate("conn_wizard_oauth_started")),
            Span::styled(
                remote_name,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ("),
            Span::raw(provider),
            Span::raw(")"),
        ]),
        Line::from(""),
        Line::from(crate::lang::translate("conn_wizard_oauth_open_browser")),
        Line::from(
            crate::lang::translate("conn_wizard_oauth_copy_url"),
        ),
        Line::from(""),
        Line::from(Span::styled(
            auth_url,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::UNDERLINED),
        )),
        Line::from(""),
        Line::from(Span::styled(
            crate::lang::translate("conn_wizard_oauth_waiting"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::SLOW_BLINK),
        )),
    ];

    let block = Block::default()
        .title(Span::styled(
            crate::lang::translate("conn_wizard_oauth_title"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_headless_oauth_wizard(
    frame: &mut Frame,
    provider: &str,
    remote_name: &str,
    client_id: &str,
    client_secret: &str,
    token_input: &str,
    focused_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(65, 65, size);
    frame.render_widget(Clear, area);

    let cmd = if client_id.is_empty() {
        format!("rclone authorize \"{}\"", provider)
    } else {
        format!("rclone authorize \"{}\" \"{}\" \"{}\"", provider, client_id, client_secret)
    };

    let title = crate::lang::translate("conn_wizard_headless_title").replace("{}", remote_name);
    let prompt_tpl = crate::lang::translate("conn_wizard_headless_prompt");
    let prompt = prompt_tpl.replace("{}", &cmd);

    let mut text = vec![];
    for line in prompt.lines() {
        text.push(Line::from(line));
    }
    text.push(Line::from(""));
    text.push(Line::from("------------------------------------------------------------------"));

    let id_focused = focused_idx == 0;
    let sec_focused = focused_idx == 1;
    let tok_focused = focused_idx == 2;

    text.push(Line::from(vec![
        Span::styled("1. OAuth Client ID (Optional): ", Style::default().fg(if id_focused { Color::Yellow } else { Color::DarkGray })),
        Span::styled(client_id, Style::default().fg(Color::White).bg(if id_focused { Color::Blue } else { Color::DarkGray })),
    ]));
    text.push(Line::from(""));
    text.push(Line::from(vec![
        Span::styled("2. OAuth Client Secret (Optional): ", Style::default().fg(if sec_focused { Color::Yellow } else { Color::DarkGray })),
        Span::styled(client_secret, Style::default().fg(Color::White).bg(if sec_focused { Color::Blue } else { Color::DarkGray })),
    ]));
    text.push(Line::from(""));
    text.push(Line::from(vec![
        Span::styled("3. Paste Token JSON here: ", Style::default().fg(if tok_focused { Color::Yellow } else { Color::DarkGray })),
        Span::styled(token_input, Style::default().fg(Color::White).bg(if tok_focused { Color::Blue } else { Color::DarkGray })),
    ]));
    text.push(Line::from(""));
    text.push(Line::from(" [Tab] Chuyển trường | [Enter] Xác nhận | [Esc] Hủy "));

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(text).block(block).wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

use super::centered_rect;
fn translate_field(name: &str, desc: &str) -> (String, String) {
    if name == "_remote_name" {
        return ("Tên Remote (Remote Name)".to_string(), desc.to_string());
    }
    let friendly_name = name.to_string(); // Giữ nguyên mẫu ID từ rclone gốc theo yêu cầu của user
    let friendly_desc = crate::lang::translate_desc(name, desc);
    (friendly_name, friendly_desc)
}

pub fn is_basic_field(name: &str) -> bool {
    let name = name.to_lowercase();
    name == "_remote_name"
        || name == "remote"
        || name == "password"
        || name == "password2"
        || name == "client_id"
        || name == "client_secret"
        || name == "token"
        || name == "description"
        || name == "user"
        || name == "pass"
        || name == "host"
        || name == "port"
}

fn draw_advanced_setup_wizard(
    frame: &mut Frame,
    provider: &str,
    remote_name: &str,
    fields: &[(String, String, String, Vec<String>)],
    selected_field_idx: usize,
    scroll_offset: usize,
    is_editing: bool,
    input_buffer: &str,
    active_tab: usize,
) {
    let size = frame.size();
    let area = centered_rect(65, 75, size);
    frame.render_widget(Clear, area);

    let title_raw = crate::lang::translate("conn_wizard_edit_title");
    let title_fmt = format!(" {} ", title_raw.replacen("{}", remote_name, 1).replacen("{}", provider, 1));
    let block = Block::default()
        .title(Span::styled(
            title_fmt,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    frame.render_widget(&block, area);

    // Lấy inner_area bên trong đường viền của block
    let inner_area = block.inner(area);

    // Lọc danh sách fields theo tab
    let filtered_fields: Vec<&(String, String, String, Vec<String>)> = fields
        .iter()
        .filter(|(name, _, _, _)| {
            if active_tab == 0 {
                is_basic_field(name)
            } else {
                !is_basic_field(name)
            }
        })
        .collect();

    // Chia giao diện bên trong block viền thành các phần: Tab bar, đường chia và danh sách
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
        Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    };
    let adv_style = if active_tab == 1 {
        Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    };

    let tab_line = Line::from(vec![
        Span::styled(crate::lang::translate("conn_wizard_edit_tab_basic"), basic_style),
        Span::raw("   "),
        Span::styled(crate::lang::translate("conn_wizard_edit_tab_adv"), adv_style),
        Span::raw(crate::lang::translate("conn_wizard_edit_tab_help")),
    ]);
    frame.render_widget(Paragraph::new(tab_line), inner_chunks[0]);

    // Vẽ đường chia ngang
    frame.render_widget(
        Paragraph::new("─".repeat(inner_chunks[1].width as usize))
            .style(Style::default().fg(Color::DarkGray)),
        inner_chunks[1],
    );

    let height = inner_chunks[2].height.saturating_sub(4) as usize; // trừ nút bấm và dòng tip

    let mut items: Vec<ListItem> = filtered_fields
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(height)
        .map(|(i, (name, desc, value, choices))| {
            let (friendly_name, friendly_desc) = translate_field(name, desc);
            
            // Vẽ gợi ý các lựa chọn (choices) có sẵn ngay cạnh giá trị
            let choices_str = if !choices.is_empty() {
                format!(" < {} >", choices.join(" | "))
            } else {
                String::new()
            };

            let display_val = if i == selected_field_idx && is_editing {
                format!("{}{}", input_buffer, choices_str)
            } else {
                format!("{}{}", value, choices_str)
            };

            let line = if i == selected_field_idx {
                let cursor = if is_editing { " 📝 " } else { " >> " };
                let bg = if is_editing {
                    Color::DarkGray
                } else {
                    Color::Yellow
                };
                let fg = if is_editing {
                    Color::White
                } else {
                    Color::Black
                };

                Line::from(vec![
                    Span::styled(cursor, Style::default().fg(Color::Red)),
                    Span::styled(
                        format!("{}: ", friendly_name),
                        Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(display_val, Style::default().fg(fg).bg(bg)),
                    Span::raw(format!(" - ({})", friendly_desc)),
                ])
            } else {
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("{}: ", friendly_name),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::raw(display_val),
                    Span::raw(format!(" - ({})", friendly_desc)),
                ])
            };
            ListItem::new(line)
        })
        .collect();

    // Thêm dòng ngăn cách trống
    items.push(ListItem::new(Line::raw("")));

    // Nút Lưu và Hủy nằm ở cuối danh sách lọc của tab hiện tại
    let save_idx = filtered_fields.len();
    let cancel_idx = filtered_fields.len() + 1;

    // Nút Lưu
    let save_style = if selected_field_idx == save_idx {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };
    items.push(ListItem::new(Line::from(vec![
        Span::raw(if selected_field_idx == save_idx {
            " >> "
        } else {
            "    "
        }),
        Span::styled(crate::lang::translate("conn_wizard_edit_save"), save_style),
    ])));

    // Nút Quay lại
    let cancel_style = if selected_field_idx == cancel_idx {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red)
    };
    items.push(ListItem::new(Line::from(vec![
        Span::raw(if selected_field_idx == cancel_idx {
            " >> "
        } else {
            "    "
        }),
        Span::styled(crate::lang::translate("conn_wizard_edit_cancel"), cancel_style),
    ])));

    // Thêm dòng lưu ý/mẹo
    let unikey_tip = if is_editing
        && filtered_fields.get(selected_field_idx).map(|f| f.0.as_str()) == Some("remote")
    {
        crate::lang::translate_tip("tip_select_remote")
    } else if is_editing
        && filtered_fields
            .get(selected_field_idx)
            .map(|f| !f.3.is_empty())
            .unwrap_or(false)
    {
        crate::lang::translate_tip("tip_select_choice")
    } else {
        crate::lang::translate_tip("unikey_tip")
    };
    items.push(ListItem::new(Line::from(vec![
        Span::raw("    "),
        Span::styled(
            unikey_tip,
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ),
    ])));

    let list = List::new(items);
    frame.render_widget(list, inner_chunks[2]);
}

fn draw_edit_setup_wizard(
    frame: &mut Frame,
    remote_name: &str,
    provider: &str,
    fields: &[(String, String, String, Vec<String>)], // (Tên trường, Mô tả, Giá trị, Lựa chọn)
    selected_idx: usize,
    scroll_offset: usize,
    is_editing: bool,
    input_buffer: &str,
    _adding_new_key: bool,
    _new_key_buffer: &str,
    active_tab: usize,
) {
    let size = frame.size();
    let area = centered_rect(65, 75, size);
    frame.render_widget(Clear, area);

    let title_raw = crate::lang::translate("conn_wizard_edit_title_simple");
    let title_fmt = format!(" {} ", title_raw.replacen("{}", remote_name, 1).replacen("{}", provider, 1));
    let block = Block::default()
        .title(Span::styled(
            title_fmt,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    frame.render_widget(&block, area);

    // Lấy inner_area bên trong đường viền của block
    let inner_area = block.inner(area);

    // Lọc danh sách fields theo tab
    let filtered_fields: Vec<&(String, String, String, Vec<String>)> = fields
        .iter()
        .filter(|(name, _, _, _)| {
            if active_tab == 0 {
                is_basic_field(name)
            } else {
                !is_basic_field(name)
            }
        })
        .collect();

    // Chia giao diện bên trong block viền thành các phần: Tab bar, đường chia và danh sách
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
        Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    };
    let adv_style = if active_tab == 1 {
        Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    };

    let tab_line = Line::from(vec![
        Span::styled(crate::lang::translate("conn_wizard_edit_tab_basic"), basic_style),
        Span::raw("   "),
        Span::styled(crate::lang::translate("conn_wizard_edit_tab_adv"), adv_style),
        Span::raw(crate::lang::translate("conn_wizard_edit_tab_help")),
    ]);
    frame.render_widget(Paragraph::new(tab_line), inner_chunks[0]);

    // Vẽ đường chia ngang
    frame.render_widget(
        Paragraph::new("─".repeat(inner_chunks[1].width as usize))
            .style(Style::default().fg(Color::DarkGray)),
        inner_chunks[1],
    );

    let height = inner_chunks[2].height.saturating_sub(4) as usize; // trừ nút bấm và dòng tip

    let mut items: Vec<ListItem> = filtered_fields
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(height)
        .map(|(i, (name, desc, value, choices))| {
            let (friendly_name, friendly_desc) = translate_field(name, desc);
            
            // Vẽ gợi ý các lựa chọn (choices) có sẵn ngay cạnh giá trị
            let choices_str = if !choices.is_empty() {
                format!(" < {} >", choices.join(" | "))
            } else {
                String::new()
            };

            let display_val = if i == selected_idx && is_editing {
                format!("{}{}", input_buffer, choices_str)
            } else {
                format!("{}{}", value, choices_str)
            };

            let line = if i == selected_idx {
                let cursor = if is_editing { " 📝 " } else { " >> " };
                let bg = if is_editing {
                    Color::DarkGray
                } else {
                    Color::Yellow
                };
                let fg = if is_editing {
                    Color::White
                } else {
                    Color::Black
                };

                Line::from(vec![
                    Span::styled(cursor, Style::default().fg(Color::Red)),
                    Span::styled(
                        format!("{}: ", friendly_name),
                        Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(display_val, Style::default().fg(fg).bg(bg)),
                    Span::raw(format!(" - ({})", friendly_desc)),
                ])
            } else {
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("{}: ", friendly_name),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::raw(display_val),
                    Span::raw(format!(" - ({})", friendly_desc)),
                ])
            };
            ListItem::new(line)
        })
        .collect();

    // Thêm dòng ngăn cách trống
    items.push(ListItem::new(Line::raw("")));

    // Nút Lưu và Hủy nằm ở cuối danh sách lọc của tab hiện tại
    let save_idx = filtered_fields.len();
    let cancel_idx = filtered_fields.len() + 1;

    // Nút Lưu
    let save_style = if selected_idx == save_idx {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };
    items.push(ListItem::new(Line::from(vec![
        Span::raw(if selected_idx == save_idx {
            " >> "
        } else {
            "    "
        }),
        Span::styled(crate::lang::translate("conn_wizard_edit_save"), save_style),
    ])));

    // Nút Quay lại
    let cancel_style = if selected_idx == cancel_idx {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red)
    };
    items.push(ListItem::new(Line::from(vec![
        Span::raw(if selected_idx == cancel_idx {
            " >> "
        } else {
            "    "
        }),
        Span::styled(crate::lang::translate("conn_wizard_edit_cancel"), cancel_style),
    ])));

    // Thêm dòng lưu ý/mẹo
    let unikey_tip = if is_editing
        && filtered_fields.get(selected_idx).map(|f| f.0.as_str()) == Some("remote")
    {
        crate::lang::translate_tip("tip_select_remote")
    } else if is_editing
        && filtered_fields
            .get(selected_idx)
            .map(|f| !f.3.is_empty())
            .unwrap_or(false)
    {
        crate::lang::translate_tip("tip_select_choice")
    } else {
        crate::lang::translate_tip("unikey_tip")
    };
    items.push(ListItem::new(Line::from(vec![
        Span::raw("    "),
        Span::styled(
            unikey_tip,
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ),
    ])));

    let list = List::new(items);
    frame.render_widget(list, inner_chunks[2]);
}

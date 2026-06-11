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
    SelectZohoRegion {
        provider: String,
        remote_name: String,
        selected_idx: usize,
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
        fields: Vec<(String, String, String, Vec<(String, String)>, bool)>, // (Tên trường, Mô tả, Giá trị, Lựa chọn, Bắt buộc)
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
        fields: Vec<(String, String, String, Vec<(String, String)>, bool)>, // (Tên trường, Mô tả, Giá trị, Lựa chọn, Bắt buộc)
        selected_idx: usize,
        scroll_offset: usize,
        is_editing: bool,
        input_buffer: String,
        adding_new_key: bool,
        new_key_buffer: String,
        active_tab: usize,
    },
    ShowFeatures {
        remote_name: String,
        features: Vec<(String, bool)>,
        union_remotes_features: Option<Vec<(String, Vec<(String, bool)>)>>,
    },
    SelectOneChoice {
        provider: String,
        remote_name: String,
        fields: Vec<(String, String, String, Vec<(String, String)>, bool)>,
        selected_field_idx: usize,
        scroll_offset: usize,
        active_tab: usize,
        selected_providers: Vec<String>,
        is_edit_mode: bool,

        field_name: String,
        choices: Vec<(String, String)>,
        choices_selected_idx: usize,
    },
    SelectMultipleChoices {
        provider: String,
        remote_name: String,
        fields: Vec<(String, String, String, Vec<(String, String)>, bool)>,
        selected_field_idx: usize,
        scroll_offset: usize,
        active_tab: usize,
        selected_providers: Vec<String>,
        is_edit_mode: bool,

        field_name: String,
        options: Vec<(String, bool)>,
        choices_selected_idx: usize,
    },
    ImportConfigInput {
        input_buffer: String,
    },
}

pub struct ConnectionState {
    pub remotes: Vec<String>,
    pub selected_idx: usize,
    pub wizard: WizardState,
    pub error_message: Option<String>,
    pub info_message: Option<String>,
    pub remote_statuses: std::collections::HashMap<String, String>,
    pub edit_cursor_idx: usize,
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
            edit_cursor_idx: 0,
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

pub fn draw(
    state: &ConnectionState,
    frame: &mut Frame,
    area: Rect,
    filen_cli_installed: bool,
    remote_types: &std::collections::HashMap<String, String>,
) {
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
            let r_type = remote_types.get(remote).map(|s| s.as_str()).unwrap_or("Cloud");
            let text = format!("  [{}] -> {:<25} | {}", r_type, remote, status);
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
        WizardState::SelectZohoRegion {
            remote_name,
            selected_idx,
            ..
        } => {
            draw_select_zoho_region_wizard(frame, remote_name, *selected_idx);
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
                state.edit_cursor_idx,
                filen_cli_installed,
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
                state.edit_cursor_idx,
                filen_cli_installed,
            );
        }
        WizardState::ShowFeatures {
            remote_name,
            features,
            union_remotes_features,
        } => {
            draw_show_features_popup(frame, remote_name, features, union_remotes_features);
        }
        WizardState::SelectOneChoice {
            provider,
            remote_name,
            fields,
            selected_field_idx,
            scroll_offset,
            active_tab,
            is_edit_mode,
            field_name,
            choices,
            choices_selected_idx,
            ..
        } => {
            if *is_edit_mode {
                draw_edit_setup_wizard(
                    frame,
                    remote_name,
                    provider,
                    fields,
                    *selected_field_idx,
                    *scroll_offset,
                    false,
                    "",
                    false,
                    "",
                    *active_tab,
                    0,
                    filen_cli_installed,
                );
            } else {
                draw_advanced_setup_wizard(
                    frame,
                    provider,
                    remote_name,
                    fields,
                    *selected_field_idx,
                    *scroll_offset,
                    false,
                    "",
                    *active_tab,
                    0,
                    filen_cli_installed,
                );
            }
            draw_select_one_choice_wizard(frame, field_name, choices, *choices_selected_idx);
        }
        WizardState::SelectMultipleChoices {
            provider,
            remote_name,
            fields,
            selected_field_idx,
            scroll_offset,
            active_tab,
            is_edit_mode,
            field_name,
            options,
            choices_selected_idx,
            ..
        } => {
            if *is_edit_mode {
                draw_edit_setup_wizard(
                    frame,
                    remote_name,
                    provider,
                    fields,
                    *selected_field_idx,
                    *scroll_offset,
                    false,
                    "",
                    false,
                    "",
                    *active_tab,
                    0,
                    filen_cli_installed,
                );
            } else {
                draw_advanced_setup_wizard(
                    frame,
                    provider,
                    remote_name,
                    fields,
                    *selected_field_idx,
                    *scroll_offset,
                    false,
                    "",
                    *active_tab,
                    0,
                    filen_cli_installed,
                );
            }
            draw_select_multiple_choices_wizard(frame, field_name, options, *choices_selected_idx);
        }
        WizardState::ImportConfigInput { input_buffer } => {
            draw_import_config_input_wizard(frame, input_buffer);
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

fn draw_import_config_input_wizard(frame: &mut Frame, input_buffer: &str) {
    let size = frame.size();
    let area = centered_rect(65, 25, size);
    frame.render_widget(Clear, area);

    let text = vec![
        Line::from("Bổ sung cấu hình từ tệp rclone.conf khác"),
        Line::from(""),
        Line::from("Nhập đường dẫn tuyệt đối đến tệp cấu hình nguồn:"),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Green)),
            Span::styled(
                input_buffer,
                Style::default().fg(Color::White).bg(Color::DarkGray),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " [Enter] Bắt đầu nhập | [Esc] Quay lại ",
            Style::default().fg(Color::Gray),
        )),
    ];

    let block = Block::default()
        .title(Span::styled(
            " Nhập file cấu hình (Import Config) ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
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

fn draw_select_zoho_region_wizard(
    frame: &mut Frame,
    remote_name: &str,
    selected_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(55, 45, size);
    frame.render_widget(Clear, area);

    let regions = vec![
        "United States / Global (com)",
        "Europe (eu)",
        "India (in)",
        "Japan (jp)",
        "China (com.cn)",
        "Australia (com.au)",
    ];

    let items: Vec<ListItem> = regions
        .iter()
        .enumerate()
        .map(|(i, region)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", region)).style(style)
        })
        .collect();

    let title_raw = crate::lang::translate("conn_wizard_zoho_region_title");
    let title_fmt = format!(" {} ", title_raw.replace("{}", remote_name));
    let prompt = crate::lang::translate("conn_wizard_zoho_region_prompt");

    // We can show the prompt text inside the block or as a header.
    // Let's create a list with items, but also add a description paragraph.
    // Or even simpler: the block has a title, and we render the options. That's very clean and matches the other wizards.
    let block = Block::default()
        .title(Span::styled(
            title_fmt,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    // Let's split the area to draw a prompt instruction and the selection list.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Prompt text
            Constraint::Min(2),    // List of regions
        ])
        .split(block.inner(area));

    // Draw prompt
    let prompt_para = Paragraph::new(prompt)
        .wrap(ratatui::widgets::Wrap { trim: false })
        .style(Style::default().fg(Color::White));
    
    // Draw background block border around the whole wizard
    frame.render_widget(&block, area);
    frame.render_widget(prompt_para, chunks[0]);

    let list = List::new(items);
    frame.render_widget(list, chunks[1]);
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
pub fn translate_field(name: &str, desc: &str) -> (String, String) {
    if name == "_remote_name" {
        return ("Tên Remote (Remote Name)".to_string(), desc.to_string());
    }
    let friendly_name = if let Some(friendly) = crate::lang::translate_friendly(name) {
        format!("{} ({})", name, friendly)
    } else {
        name.to_string()
    };
    let friendly_desc = crate::lang::translate_desc(name, desc);
    (friendly_name, friendly_desc)
}

pub fn is_field_required(
    name: &str,
    fields: &[(String, String, String, Vec<(String, String)>, bool)],
    selected_idx: usize,
    is_editing: bool,
    input_buffer: &str,
) -> bool {
    let name_lower = name.to_lowercase();
    if name_lower == "_remote_name" {
        return true;
    }

    // Helper to get the current active value of a field in the wizard
    let get_val = |target_name: &str| -> String {
        for (idx, (f_name, _, f_val, _, _)) in fields.iter().enumerate() {
            if f_name.to_lowercase() == target_name {
                if idx == selected_idx && is_editing {
                    return input_buffer.to_string();
                } else {
                    return f_val.to_string();
                }
            }
        }
        String::new()
    };

    let required_by_default = fields.iter().find(|f| f.0.to_lowercase() == name_lower).map(|f| f.4).unwrap_or(false);

    // Mutual exclusivity groups:
    // 1. token vs client_id
    if name_lower == "token" {
        let has_client_id = !get_val("client_id").trim().is_empty();
        let has_token = !get_val("token").trim().is_empty();
        return !has_client_id && !has_token;
    }
    if name_lower == "client_id" || name_lower == "client_secret" {
        return false;
    }

    // 2. service_account_file vs service_account_credentials
    if name_lower == "service_account_file" {
        let has_creds = !get_val("service_account_credentials").trim().is_empty();
        let has_file = !get_val("service_account_file").trim().is_empty();
        return !has_creds && !has_file;
    }
    if name_lower == "service_account_credentials" {
        return false;
    }

    // 3. pass vs key_file / key_use_agent
    if name_lower == "pass" {
        let has_key = !get_val("key_file").trim().is_empty() || get_val("key_use_agent").trim() == "true";
        let has_pass = !get_val("pass").trim().is_empty();
        return !has_key && !has_pass;
    }
    if name_lower == "key_file" || name_lower == "key_use_agent" {
        return false;
    }

    required_by_default
}

pub fn is_basic_field(name: &str, required: bool) -> bool {
    if required {
        return true;
    }
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
    fields: &[(String, String, String, Vec<(String, String)>, bool)],
    selected_field_idx: usize,
    scroll_offset: usize,
    is_editing: bool,
    input_buffer: &str,
    active_tab: usize,
    cursor_idx: usize,
    filen_cli_installed: bool,
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
    let filtered_fields: Vec<&(String, String, String, Vec<(String, String)>, bool)> = fields
        .iter()
        .filter(|(name, _, _, _, required)| {
            if active_tab == 0 {
                is_basic_field(name, *required)
            } else {
                !is_basic_field(name, *required)
            }
        })
        .collect();

    // Chia giao diện bên trong block viền thành các phần: Tab bar, đường chia, danh sách, đường chia và hộp mô tả
    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Tab bar
            Constraint::Length(1), // Divider line
            Constraint::Min(3),    // Fields list
            Constraint::Length(1), // Divider line
            Constraint::Length(5), // Description box
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
        .map(|(i, (name, desc, value, choices, _required))| {
            let (friendly_name, _friendly_desc) = translate_field(name, desc);
            
            // Vẽ gợi ý các lựa chọn (choices) có sẵn ngay cạnh giá trị
            let choices_str = if !choices.is_empty() {
                let choice_vals: Vec<String> = choices.iter().map(|(val, _)| val.clone()).collect();
                format!(" < {} >", choice_vals.join(" | "))
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

                let is_req = is_field_required(name, fields, selected_field_idx, is_editing, input_buffer);
                let label_fg = if is_req {
                    Color::Red
                } else {
                    fg
                };

                let mut spans = vec![
                    Span::styled(cursor, Style::default().fg(Color::Red)),
                    Span::styled(
                        format!("{}: ", friendly_name),
                        Style::default().fg(label_fg).bg(bg).add_modifier(Modifier::BOLD),
                    ),
                ];
                if is_editing {
                    spans.extend(super::make_input_spans_with_cursor(input_buffer, cursor_idx, fg, bg));
                    if !choices_str.is_empty() {
                        spans.push(Span::styled(choices_str, Style::default().fg(fg).bg(bg)));
                    }
                } else {
                    spans.push(Span::styled(display_val, Style::default().fg(fg).bg(bg)));
                }
                if provider.to_lowercase() == "filen" && name == "api_key" {
                    if filen_cli_installed {
                        spans.push(Span::styled(
                            crate::lang::translate("conn_insert_api_key_hint"),
                            Style::default()
                                .fg(Color::Yellow)
                                .bg(bg)
                                .add_modifier(Modifier::BOLD),
                        ));
                    } else {
                        spans.push(Span::styled(
                            crate::lang::translate("conn_insert_api_key_missing_hint"),
                            Style::default()
                                .fg(Color::Red)
                                .bg(bg)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                }
                Line::from(spans)
            } else {
                let is_req = is_field_required(name, fields, selected_field_idx, is_editing, input_buffer);
                let (prefix, label_style) = if is_req {
                    ("  * ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                } else {
                    ("    ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
                };
                Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::Red)),
                    Span::styled(
                        format!("{}: ", friendly_name),
                        label_style,
                    ),
                    Span::styled(display_val, Style::default().fg(Color::White)),
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

    // Draw the description box
    let current_desc = filtered_fields.get(selected_field_idx)
        .map(|f| translate_field(&f.0, &f.1).1)
        .unwrap_or_default();

    frame.render_widget(
        Paragraph::new("─".repeat(inner_chunks[3].width as usize))
            .style(Style::default().fg(Color::DarkGray)),
        inner_chunks[3],
    );

    let desc_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(" MÔ TẢ / DESCRIPTION ", Style::default().fg(Color::DarkGray)));
    let desc_paragraph = Paragraph::new(current_desc)
        .block(desc_block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(desc_paragraph, inner_chunks[4]);
}

fn draw_edit_setup_wizard(
    frame: &mut Frame,
    remote_name: &str,
    provider: &str,
    fields: &[(String, String, String, Vec<(String, String)>, bool)], // (Tên trường, Mô tả, Giá trị, Lựa chọn, Bắt buộc)
    selected_idx: usize,
    scroll_offset: usize,
    is_editing: bool,
    input_buffer: &str,
    _adding_new_key: bool,
    _new_key_buffer: &str,
    active_tab: usize,
    cursor_idx: usize,
    filen_cli_installed: bool,
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
    let filtered_fields: Vec<&(String, String, String, Vec<(String, String)>, bool)> = fields
        .iter()
        .filter(|(name, _, _, _, required)| {
            if active_tab == 0 {
                is_basic_field(name, *required)
            } else {
                !is_basic_field(name, *required)
            }
        })
        .collect();

    // Chia giao diện bên trong block viền thành các phần: Tab bar, đường chia, danh sách, đường chia và hộp mô tả
    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Tab bar
            Constraint::Length(1), // Divider line
            Constraint::Min(3),    // Fields list
            Constraint::Length(1), // Divider line
            Constraint::Length(5), // Description box
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
        .map(|(i, (name, desc, value, choices, _required))| {
            let (friendly_name, _friendly_desc) = translate_field(name, desc);
            
            // Vẽ gợi ý các lựa chọn (choices) có sẵn ngay cạnh giá trị
            let choices_str = if !choices.is_empty() {
                let choice_vals: Vec<String> = choices.iter().map(|(val, _)| val.clone()).collect();
                format!(" < {} >", choice_vals.join(" | "))
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

                let is_req = is_field_required(name, fields, selected_idx, is_editing, input_buffer);
                let label_fg = if is_req {
                    Color::Red
                } else {
                    fg
                };

                let mut spans = vec![
                    Span::styled(cursor, Style::default().fg(Color::Red)),
                    Span::styled(
                        format!("{}: ", friendly_name),
                        Style::default().fg(label_fg).bg(bg).add_modifier(Modifier::BOLD),
                    ),
                ];
                if is_editing {
                    spans.extend(super::make_input_spans_with_cursor(input_buffer, cursor_idx, fg, bg));
                    if !choices_str.is_empty() {
                        spans.push(Span::styled(choices_str, Style::default().fg(fg).bg(bg)));
                    }
                } else {
                    spans.push(Span::styled(display_val, Style::default().fg(fg).bg(bg)));
                }
                if provider.to_lowercase() == "filen" && name == "api_key" {
                    if filen_cli_installed {
                        spans.push(Span::styled(
                            crate::lang::translate("conn_insert_api_key_hint"),
                            Style::default()
                                .fg(Color::Yellow)
                                .bg(bg)
                                .add_modifier(Modifier::BOLD),
                        ));
                    } else {
                        spans.push(Span::styled(
                            crate::lang::translate("conn_insert_api_key_missing_hint"),
                            Style::default()
                                .fg(Color::Red)
                                .bg(bg)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                }
                Line::from(spans)
            } else {
                let is_req = is_field_required(name, fields, selected_idx, is_editing, input_buffer);
                let (prefix, label_style) = if is_req {
                    ("  * ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                } else {
                    ("    ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
                };
                Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::Red)),
                    Span::styled(
                        format!("{}: ", friendly_name),
                        label_style,
                    ),
                    Span::styled(display_val, Style::default().fg(Color::White)),
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

    // Draw the description box
    let current_desc = filtered_fields.get(selected_idx)
        .map(|f| translate_field(&f.0, &f.1).1)
        .unwrap_or_default();

    frame.render_widget(
        Paragraph::new("─".repeat(inner_chunks[3].width as usize))
            .style(Style::default().fg(Color::DarkGray)),
        inner_chunks[3],
    );

    let desc_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(" MÔ TẢ / DESCRIPTION ", Style::default().fg(Color::DarkGray)));
    let desc_paragraph = Paragraph::new(current_desc)
        .block(desc_block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(desc_paragraph, inner_chunks[4]);
}

pub fn draw_show_features_popup(
    frame: &mut Frame,
    remote_name: &str,
    features: &[(String, bool)],
    union_remotes_features: &Option<Vec<(String, Vec<(String, bool)>)>>,
) {
    let size = frame.size();
    let area = centered_rect(85, 75, size);
    frame.render_widget(Clear, area);

    let mut lines = Vec::new();

    // Kiểm tra xem có đồng đẳng không (chỉ khi là union)
    let mut has_mismatch = false;
    if let Some(upstreams) = union_remotes_features {
        let mut all_keys = std::collections::BTreeSet::new();
        for (_, list) in upstreams {
            for (k, _) in list {
                all_keys.insert(k.clone());
            }
        }

        for key in &all_keys {
            let mut first_val = None;
            for (_, list) in upstreams {
                if let Some((_, val)) = list.iter().find(|(k, _)| k == key) {
                    if let Some(f) = first_val {
                        if f != val {
                            has_mismatch = true;
                            break;
                        }
                    } else {
                        first_val = Some(val);
                    }
                }
            }
            if has_mismatch {
                break;
            }
        }
    }

    // Tiêu đề popup
    let title = if union_remotes_features.is_some() {
        format!(" KIỂM TRA TÍNH NĂNG BACKEND (UNION REMOTE: {}) ", remote_name)
    } else {
        format!(" TÍNH NĂNG BACKEND HỖ TRỢ: {} ", remote_name)
    };

    let block_border_color = if has_mismatch { Color::Red } else { Color::Green };
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(block_border_color)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(block_border_color));

    // Banner Cảnh báo nếu các thành viên của Union không đồng đẳng
    if has_mismatch {
        lines.push(Line::from(vec![
            Span::styled("  ⚠ CẢNH BÁO: Các cloud thành viên không đồng đẳng! (Thiếu/Khác biệt tính năng)  ", Style::default().fg(Color::Black).bg(Color::Red).add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::from(""));
    }

    if let Some(upstreams) = union_remotes_features {
        lines.push(Line::from(vec![
            Span::styled("So sánh tính năng của các cloud thành viên:", Style::default().add_modifier(Modifier::UNDERLINED)),
        ]));
        lines.push(Line::from(""));

        // Table Header
        let mut header_spans = vec![
            Span::styled(format!("{:<22}", "Tính năng (Feature)"), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ];
        for (u_name, _) in upstreams {
            header_spans.push(Span::raw(" | "));
            let display_name = if u_name.len() > 12 {
                format!("{}.", &u_name[..11])
            } else {
                format!("{:<12}", u_name)
            };
            header_spans.push(Span::styled(display_name, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        }
        header_spans.push(Span::raw(" | "));
        header_spans.push(Span::styled("Giải thích (Mô tả)", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        lines.push(Line::from(header_spans));

        // Đường kẻ ngăn cách header
        let total_header_len = 22 + upstreams.len() * 15 + 30;
        lines.push(Line::from("-".repeat(total_header_len)));

        // Thu thập toàn bộ danh sách tính năng duy nhất
        let mut all_keys = std::collections::BTreeSet::new();
        for (_, list) in upstreams {
            for (k, _) in list {
                all_keys.insert(k.clone());
            }
        }

        // Vẽ từng hàng
        for key in all_keys {
            let mut key_mismatch = false;
            let mut first_val = None;
            for (_, list) in upstreams {
                if let Some((_, val)) = list.iter().find(|(k, _)| k == &key) {
                    if let Some(f) = first_val {
                        if f != val {
                            key_mismatch = true;
                        }
                    } else {
                        first_val = Some(val);
                    }
                }
            }

            let name_style = if key_mismatch {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };

            let prefix = if key_mismatch { "⚠ " } else { "  " };
            let mut row_spans = vec![
                Span::styled(format!("{}{:<20}", prefix, key), name_style),
            ];

            for (_, list) in upstreams {
                row_spans.push(Span::raw(" | "));
                if let Some((_, val)) = list.iter().find(|(k, _)| k == &key) {
                    if *val {
                        row_spans.push(Span::styled("  [ YES ]   ", Style::default().fg(Color::Green)));
                    } else {
                        row_spans.push(Span::styled("  [  NO ]   ", Style::default().fg(Color::Red)));
                    }
                } else {
                    row_spans.push(Span::styled("  [ N/A ]   ", Style::default().fg(Color::DarkGray)));
                }
            }
            row_spans.push(Span::raw(" | "));
            row_spans.push(Span::styled(get_feature_description(&key), Style::default().fg(Color::Gray)));
            lines.push(Line::from(row_spans));
        }
    } else {
        lines.push(Line::from(vec![
            Span::styled("Danh sách các tính năng được hỗ trợ bởi backend này:", Style::default().add_modifier(Modifier::UNDERLINED)),
        ]));
        lines.push(Line::from(""));

        for (k, v) in features {
            let val_span = if *v {
                Span::styled("  [ HỖ TRỢ ]  ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            } else {
                Span::styled("  [ KHÔNG ]   ", Style::default().fg(Color::Red))
            };
            let desc = get_feature_description(k);
            lines.push(Line::from(vec![
                Span::styled(format!("  - {:<22}: ", k), Style::default().fg(Color::Cyan)),
                val_span,
                Span::raw(" - "),
                Span::styled(desc, Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(" Nhấn [Esc] hoặc [Enter] để quay lại "));

    let list = Paragraph::new(lines).block(block).wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(list, area);
}

fn get_feature_description(key: &str) -> &'static str {
    match key {
        "About" => "Xem dung lượng đĩa",
        "BucketBased" => "Lưu trữ dạng bucket (S3/B2)",
        "BucketBasedRootOK" => "Thao tác trên root bucket",
        "CanHaveEmptyDirectories" => "Tạo thư mục rỗng",
        "CaseInsensitive" => "Không phân biệt hoa/thường",
        "ChangeNotify" => "Thông báo thay đổi tệp",
        "ChunkWriterDoesntSeek" => "Ghi phân đoạn không seek",
        "CleanUp" => "Dọn dẹp / Xóa thùng rác",
        "Command" => "Chạy lệnh backend riêng",
        "Copy" => "Sao chép trực tiếp trên Cloud",
        "DirCacheFlush" => "Xóa bộ đệm thư mục",
        "DirModTimeUpdatesOnWrite" => "Cập nhật thời gian thư mục cha",
        "DirMove" => "Di chuyển thư mục trên Cloud",
        "DirSetModTime" => "Đặt thời gian thư mục",
        "Disconnect" => "Đăng xuất / Ngắt kết nối",
        "DoubleSlash" => "Hỗ trợ đường dẫn //",
        "DuplicateFiles" => "Cho phép tệp trùng tên",
        "FilterAware" => "Bộ lọc file trên backend",
        "GetTier" => "Xem phân tầng lưu trữ (Tier)",
        "IsLocal" => "Ổ đĩa cục bộ (Local)",
        "ListP" => "Liệt kê thư mục tối ưu",
        "ListR" => "Liệt kê đệ quy tối ưu",
        "MergeDirs" => "Hợp nhất thư mục trùng tên",
        "MkdirMetadata" => "Tạo thư mục kèm Metadata",
        "Move" => "Di chuyển tệp trực tiếp trên Cloud",
        "NoMultiThreading" => "Không tải đa luồng",
        "OpenChunkWriter" => "Ghi phân đoạn lớn",
        "OpenWriterAt" => "Ghi tệp tại vị trí chỉ định",
        "Overlay" => "Lớp phủ hệ thống tệp",
        "PartialUploads" => "Hỗ trợ tải lên dang dở",
        "PublicLink" => "Tạo liên kết chia sẻ công khai",
        "Purge" => "Xóa sạch thư mục (đệ quy)",
        "PutStream" => "Tải lên dạng luồng dữ liệu",
        "PutUnchecked" => "Tải lên không check checksum",
        "ReadDirMetadata" => "Đọc Metadata của thư mục",
        "ReadMetadata" => "Đọc Metadata của tệp",
        "ReadMimeType" => "Đọc định dạng tệp (MIME)",
        "ServerSideAcrossConfigs" => "Sao chép/Di chuyển xuyên Cloud",
        "SetTier" => "Đặt phân tầng lưu trữ (Tier)",
        "SetWrapper" => "Bao bọc luồng ghi dữ liệu",
        "Shutdown" => "Đóng kết nối an toàn",
        "SlowHash" => "Tính mã hash chậm",
        "SlowModTime" => "Lấy thời gian sửa đổi chậm",
        "UnWrap" => "Mở gói để lấy backend gốc",
        "UserDirMetadata" => "Đặt Metadata thư mục tự chọn",
        "UserInfo" => "Xem thông tin tài khoản Cloud",
        "UserMetadata" => "Đặt Metadata tệp tin tự chọn",
        "WrapFs" => "Hệ thống tệp bao bọc (Wrapper)",
        "WriteDirMetadata" => "Ghi Metadata cho thư mục",
        "WriteDirSetModTime" => "Đặt thời gian cho thư mục",
        "WriteMetadata" => "Ghi Metadata cho tệp",
        "WriteMimeType" => "Đặt định dạng tệp (MIME) khi upload",
        _ => "Tính năng backend",
    }
}

pub fn draw_select_one_choice_wizard(
    frame: &mut Frame,
    field_name: &str,
    choices: &[(String, String)],
    selected_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(70, 60, size);
    frame.render_widget(Clear, area);

    let items: Vec<ListItem> = choices
        .iter()
        .enumerate()
        .map(|(i, (choice_val, _))| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let text = format!("  •  {}", choice_val);
            ListItem::new(text).style(style)
        })
        .collect();

    let title = format!(" CHỌN GIÁ TRỊ CHO: {} ", field_name.to_uppercase());
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    // Split list, divider, detail/description, and bottom help bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Choices list
            Constraint::Length(1), // Horizontal line divider
            Constraint::Length(4), // Description box
            Constraint::Length(1), // Help bar
        ])
        .split(block.inner(area));

    let list = List::new(items);
    frame.render_widget(block, area);
    frame.render_widget(list, chunks[0]);

    // Horizontal divider line
    frame.render_widget(
        Paragraph::new("─".repeat(chunks[1].width as usize))
            .style(Style::default().fg(Color::DarkGray)),
        chunks[1],
    );

    // Selected choice description/note
    let selected_desc = choices.get(selected_idx)
        .map(|(_, desc)| desc.as_str())
        .unwrap_or("");
    let selected_desc_trimmed = selected_desc.trim();
    let display_desc = if selected_desc_trimmed.is_empty() {
        "Không có mô tả thêm cho lựa chọn này."
    } else {
        selected_desc_trimmed
    };

    let desc_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(" CHI TIẾT / DETAILS ", Style::default().fg(Color::DarkGray)));
    let desc_paragraph = Paragraph::new(display_desc)
        .block(desc_block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(desc_paragraph, chunks[2]);

    let help_line = Line::from(vec![
        Span::styled(" [Mũi tên] Di chuyển | [Enter] Chọn | [Esc] Quay lại ", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(help_line), chunks[3]);
}

pub fn draw_select_multiple_choices_wizard(
    frame: &mut Frame,
    field_name: &str,
    options: &[(String, bool)],
    selected_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(55, 60, size);
    frame.render_widget(Clear, area);

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, (val, checked))| {
            let checkbox = if *checked { "[X]" } else { "[ ]" };
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}  {}", checkbox, val)).style(style)
        })
        .collect();

    let title = format!(" CHỌN NHIỀU REMOTE CHO: {} ", field_name.to_uppercase());
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    // Split list and bottom help bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(block.inner(area));

    let list = List::new(items);
    frame.render_widget(block, area);
    frame.render_widget(list, chunks[0]);

    let help_line = Line::from(vec![
        Span::styled(" [Mũi tên] Di chuyển | [Space] Chọn/Bỏ chọn | [Enter] Hoàn tất | [Esc] Hủy ", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(help_line), chunks[1]);
}


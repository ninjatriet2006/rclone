use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use crate::functions::*;

pub fn draw_services_utilities(state: &ServicesState, frame: &mut Frame, area: Rect) {
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
            ListItem::new(translate(option)).style(style)
        })
        .collect();

    let menu_block = Block::default()
        .title(Span::styled(
            translate("srv_title_config"),
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
    let active_range = calculate_scroll_range(
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
            translate("srv_title_tui"),
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
        let sys_range = calculate_scroll_range(
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

                let status_text = match s.active_status.as_str() {
                    "active" => "running",
                    "activating" => "activating",
                    "failed" => "failed",
                    _ => "inactive",
                };

                let (status_icon, status_color) = match status_text {
                    "running" => ("● ", Color::Green),
                    "activating" => ("↻ ", Color::Yellow),
                    "failed" => ("✖ ", Color::Red),
                    _ => ("○ ", Color::DarkGray),
                };

                let level_tag = if s.is_user {
                    translate("srv_tag_user")
                } else {
                    translate("srv_tag_system")
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
                translate("srv_title_systemd"),
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
            0 => translate("srv_help_t0"),
            1 => translate("srv_help_t1"),
            _ => translate("srv_help_t2"),
        },
        ServicesWizardState::SelectSystemdAction { .. } => {
            translate("srv_help_action")
        }
        ServicesWizardState::EditSystemdService { .. }
        | ServicesWizardState::CreateSystemdService { .. } => {
            translate("srv_help_edit")
        }
        _ => translate("srv_help_general"),
    };
    let help_paragraph = Paragraph::new(parse_help_line(&help_text))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(help_paragraph, main_chunks[1]);

    // Vẽ Wizard Popups từ widgets
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
                &translate("srv_local_system"),
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
            );
        }
        ServicesWizardState::None => {}
    }

    if let Some(ref err) = state.error_message {
        frame.render_widget(Clear, area);
        draw_popup(frame, " LỖI DỊCH VỤ ", err, 60, 30);
    } else if let Some(ref info) = state.info_message {
        frame.render_widget(Clear, area);
        draw_popup(
            frame,
            &translate("srv_info_title"),
            info,
            60,
            30,
        );
    }
}

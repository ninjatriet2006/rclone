use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    text::Span,
};
use crate::functions::*;

pub fn draw_connection_manager(
    state: &ConnectionState,
    frame: &mut Frame,
    area: Rect,
    remote_types: &std::collections::HashMap<String, String>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(3), // Help bar
        ])
        .split(area);

    // Pre-calculate column widths for alignment
    let max_type_len = state
        .remotes
        .iter()
        .map(|r| remote_types.get(r).map(|s| s.chars().count()).unwrap_or(5))
        .max()
        .unwrap_or(5);
    let max_remote_len = state
        .remotes
        .iter()
        .map(|r| r.chars().count())
        .max()
        .unwrap_or(25)
        .max(25);

    // Vẽ danh sách kết nối hiện có
    let items: Vec<ListItem> = state
        .remotes
        .iter()
        .enumerate()
        .map(|(i, remote)| {
            let is_selected_item = state.selected_names.contains(remote);
            let select_prefix = if is_selected_item {
                "✔ "
            } else {
                "  "
            };

            let mut style = if i == state.selected_idx && state.wizard == WizardState::None {
                if is_selected_item {
                    Style::default().fg(Color::Black).bg(Color::LightGreen).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)
                }
            } else if is_selected_item {
                Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            if state.shift_anchor == Some(i) {
                style = style.add_modifier(Modifier::UNDERLINED);
                if i != state.selected_idx || state.wizard != WizardState::None {
                    style = style.fg(Color::LightMagenta);
                }
            }

            let status = state
                .remote_statuses
                .get(remote)
                .cloned()
                .unwrap_or_else(|| translate("status_unchecked"));
            let r_type = remote_types.get(remote).map(|s| s.as_str()).unwrap_or("Cloud");
            let text = format!(
                "{} [{:<type_width$}] -> {:<remote_width$} | {}",
                select_prefix, r_type, remote, status,
                type_width = max_type_len,
                remote_width = max_remote_len
            );
            ListItem::new(text).style(style)
        })
        .collect();

    let list_title = translate("conn_title");
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
            translate("conn_help_navigation")
        }
        WizardState::EditSetup {
            is_editing,
            adding_new_key,
            ..
        } => {
            if *is_editing || *adding_new_key {
                translate("help_editing")
            } else {
                translate("help_navigation")
            }
        }
        _ => {
            translate("help_general")
        }
    };
    let help_paragraph = Paragraph::new(parse_help_line(&help_text))
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
                state.edit_cursor_idx,
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
            );
        }
        WizardState::ShowFeatures {
            remote_name,
            features,
            union_remotes_features,
        } => {
            draw_show_features_popup(frame, remote_name, features, union_remotes_features);
        }
        WizardState::ImportConfigInput { input_buffer } => {
            draw_import_config_input_wizard(frame, input_buffer);
        }
        WizardState::ExportConfigInput { input_buffer, .. } => {
            draw_export_config_input_wizard(frame, input_buffer, state.edit_cursor_idx);
        }
        WizardState::None => {}
    }

    // Vẽ thông báo lỗi/thông tin nếu có
    if let Some(ref err) = state.error_message {
        frame.render_widget(Clear, area); // clear under popup
        draw_popup(frame, &translate("conn_error_title"), err, 60, 30);
    } else if let Some(ref info) = state.info_message {
        frame.render_widget(Clear, area);
        draw_popup(frame, &translate("conn_info_title"), info, 60, 35);
    }
}

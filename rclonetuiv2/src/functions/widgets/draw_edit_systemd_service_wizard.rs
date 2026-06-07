use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use crate::functions::*;

pub fn translate_systemd_field(name: &str) -> (String, String) {
    let raw_key = name.replace('[', "").replace(']', "").replace(' ', "_");
    let name_key = format!("sys_field_{}_name", raw_key);
    let desc_key = format!("sys_field_{}_desc", raw_key);
    let friendly_name = translate(&name_key);
    let friendly_desc = translate(&desc_key);

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

pub fn draw_edit_systemd_service_wizard(
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
            translate("srv_systemd_edit_tab_basic"),
            basic_style,
        ),
        Span::raw("   "),
        Span::styled(
            translate("srv_systemd_edit_tab_adv"),
            adv_style,
        ),
        Span::raw(translate("srv_systemd_edit_tab_help")),
    ]);
    frame.render_widget(Paragraph::new(tab_line), inner_chunks[0]);

    frame.render_widget(
        Paragraph::new("─".repeat(inner_chunks[1].width as usize))
            .style(Style::default().fg(Color::DarkGray)),
        inner_chunks[1],
    );

    let height = inner_chunks[2].height.saturating_sub(4) as usize;

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
                    spans.extend(make_input_spans_with_cursor(&display_val, cursor_idx, fg, bg));
                } else {
                    spans.push(Span::styled(display_val, Style::default().fg(fg).bg(bg)));
                }
                if name == "_remote" || name == "_mount_path" {
                    spans.push(Span::styled(
                        translate("srv_insert_gui_hint"),
                        Style::default()
                            .fg(Color::Yellow)
                            .bg(bg)
                            .add_modifier(Modifier::BOLD),
                    ));
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
        Span::styled(translate("srv_systemd_edit_save"), save_style),
    ])));

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
            translate("srv_systemd_edit_cancel"),
            cancel_style,
        ),
    ])));

    let tip = if is_editing
        && filtered_fields.get(selected_idx).map(|f| f.0.as_str()) == Some("_remote")
    {
        translate_tip("tip_select_remote")
    } else {
        translate_tip("unikey_tip")
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
                translate("srv_systemd_add_param_title"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let overlay_text = vec![
            Line::from(translate("srv_systemd_add_param_prompt")),
            Line::from(Span::styled(
                format!(" > {}", new_key_buffer),
                Style::default().fg(Color::White).bg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(translate("srv_systemd_add_param_help")),
        ];
        let overlay_p = Paragraph::new(overlay_text).block(overlay_block);
        frame.render_widget(overlay_p, overlay_area);
    }
}

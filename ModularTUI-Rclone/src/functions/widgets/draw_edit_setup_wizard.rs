use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use crate::functions::*;
use crate::functions::widgets::draw_advanced_setup_wizard::{translate_field, is_basic_field};

pub fn draw_edit_setup_wizard(
    frame: &mut Frame,
    remote_name: &str,
    provider: &str,
    fields: &[(String, String, String, Vec<String>)],
    selected_idx: usize,
    scroll_offset: usize,
    is_editing: bool,
    input_buffer: &str,
    _adding_new_key: bool,
    _new_key_buffer: &str,
    active_tab: usize,
    cursor_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(65, 75, size);
    frame.render_widget(Clear, area);

    let title_raw = translate("conn_wizard_edit_title_simple");
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

    let inner_area = block.inner(area);

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

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Tab bar
            Constraint::Length(1), // Divider line
            Constraint::Min(3),    // Fields list
        ])
        .split(inner_area);

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
        Span::styled(translate("conn_wizard_edit_tab_basic"), basic_style),
        Span::raw("   "),
        Span::styled(translate("conn_wizard_edit_tab_adv"), adv_style),
        Span::raw(translate("conn_wizard_edit_tab_help")),
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
        .map(|(i, (name, desc, value, choices))| {
            let (friendly_name, friendly_desc) = translate_field(name, desc);
            
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

                let mut spans = vec![
                    Span::styled(cursor, Style::default().fg(Color::Red)),
                    Span::styled(
                        format!("{}: ", friendly_name),
                        Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
                    ),
                ];
                if is_editing {
                    spans.extend(make_input_spans_with_cursor(input_buffer, cursor_idx, fg, bg));
                    if !choices_str.is_empty() {
                        spans.push(Span::styled(choices_str, Style::default().fg(fg).bg(bg)));
                    }
                } else {
                    spans.push(Span::styled(display_val, Style::default().fg(fg).bg(bg)));
                }
                spans.push(Span::raw(format!(" - ({})", friendly_desc)));
                Line::from(spans)
            } else {
                let is_required = name == "_remote_name";
                let label_style = if is_required {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().add_modifier(Modifier::BOLD)
                };
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("{}: ", friendly_name),
                        label_style,
                    ),
                    Span::raw(display_val),
                    Span::raw(format!(" - ({})", friendly_desc)),
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
        Span::styled(translate("conn_wizard_edit_save"), save_style),
    ])));

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
        Span::styled(translate("conn_wizard_edit_cancel"), cancel_style),
    ])));

    let unikey_tip = if is_editing
        && filtered_fields.get(selected_idx).map(|f| f.0.as_str()) == Some("remote")
    {
        translate_tip("tip_select_remote")
    } else if is_editing
        && filtered_fields
            .get(selected_idx)
            .map(|f| !f.3.is_empty())
            .unwrap_or(false)
    {
        translate_tip("tip_select_choice")
    } else {
        translate_tip("unikey_tip")
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

use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use crate::functions::*;

pub fn draw_decompress_path_popup(frame: &mut Frame, _archive_path: &str, selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(55, 25, size);
    frame.render_widget(Clear, area);

    let options = vec![
        translate("exp_archive_path_manual"),
        translate("exp_archive_path_tui"),
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
            translate("exp_archive_path_title"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

pub fn draw_decompress_path_manual_input(frame: &mut Frame, _archive_path: &str, input_buffer: &str, cursor_idx: usize) {
    let size = frame.size();
    let area = centered_rect(60, 25, size);
    frame.render_widget(Clear, area);

    let mut spans = vec![Span::styled("> ", Style::default().fg(Color::Cyan))];
    spans.extend(make_input_spans_with_cursor(input_buffer, cursor_idx, Color::White, Color::DarkGray));

    let text = vec![
        Line::from(translate("exp_archive_manual_prompt")),
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

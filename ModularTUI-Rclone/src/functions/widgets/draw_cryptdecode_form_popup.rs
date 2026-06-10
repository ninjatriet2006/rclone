use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use crate::functions::*;

pub fn draw_cryptdecode_form_popup(
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
        spans.extend(make_input_spans_with_cursor(remote_input, cursor_idx, Color::White, Color::Blue));
        spans
    } else {
        vec![
            Span::styled("1. Crypt Remote (e.g. mycrypt:): ", Style::default().fg(Color::DarkGray)),
            Span::styled(remote_input, Style::default().fg(Color::White).bg(Color::DarkGray)),
        ]
    };

    let encrypted_spans = if !is_remote_focused {
        let mut spans = vec![Span::styled("2. Encrypted Filename/Path: ", Style::default().fg(Color::Yellow))];
        spans.extend(make_input_spans_with_cursor(encrypted_input, cursor_idx, Color::White, Color::Blue));
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
        Line::from(translate("exp_cryptdecode_help")),
    ];

    let block = Block::default()
        .title(Span::styled(
            translate("exp_cryptdecode_title"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

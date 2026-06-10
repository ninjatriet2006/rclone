use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use crate::functions::*;

pub fn draw_import_config_input_wizard(frame: &mut Frame, input_buffer: &str) {
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

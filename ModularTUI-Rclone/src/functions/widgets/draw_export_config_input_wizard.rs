use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use crate::functions::*;

pub fn draw_export_config_input_wizard(frame: &mut Frame, input_buffer: &str, cursor_idx: usize) {
    let size = frame.size();
    let area = centered_rect(65, 25, size);
    frame.render_widget(Clear, area);

    let mut spans = vec![Span::styled("> ", Style::default().fg(Color::Green))];
    spans.extend(make_input_spans_with_cursor(input_buffer, cursor_idx, Color::White, Color::DarkGray));

    let text = vec![
        Line::from("Xuất danh sách các cấu hình remote đã chọn"),
        Line::from(""),
        Line::from("Nhập đường dẫn tuyệt đối tệp tin muốn xuất (mặc định tại Desktop):"),
        Line::from(""),
        Line::from(spans),
        Line::from(""),
        Line::from(Span::styled(
            " [Enter] Xác nhận xuất | [Esc] Hủy bỏ ",
            Style::default().fg(Color::Gray),
        )),
    ];

    let block = Block::default()
        .title(Span::styled(
            " Xuất cấu hình Remote (Alt+X) ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

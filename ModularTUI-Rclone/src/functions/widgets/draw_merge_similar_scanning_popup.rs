use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph},
};
use crate::functions::*;

pub fn draw_merge_similar_scanning_popup(
    frame: &mut Frame,
    folders_count: usize,
    scanned_count: usize,
) {
    let size = frame.size();
    let area = centered_rect(50, 20, size);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " ĐANG QUÉT DỮ LIỆU THƯ MỤC... ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let msg = format!(
        "\n  Đang quét cấu trúc file của {} thư mục...\n\n  Đã quét xong: {} / {}\n\n  Vui lòng đợi trong giây lát...",
        folders_count, scanned_count, folders_count
    );

    let paragraph = Paragraph::new(msg).block(block);
    frame.render_widget(paragraph, area);
}

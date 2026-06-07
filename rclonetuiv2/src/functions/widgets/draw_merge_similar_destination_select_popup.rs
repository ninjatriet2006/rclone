use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use crate::functions::*;

pub fn draw_merge_similar_destination_select_popup(
    frame: &mut Frame,
    folders: &[FileItem],
    selected_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(65, 45, size);
    frame.render_widget(Clear, area);

    let items: Vec<ListItem> = folders
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_target = i == selected_idx;
            let display = format_display_name(&item.name);
            let line = if is_target {
                Line::from(vec![
                    Span::styled("👉 ", Style::default().fg(Color::Yellow)),
                    Span::styled(format!("{} (Thư mục đích / Destination)", display), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ])
            } else {
                Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(format!("{} (Thư mục nguồn / Source)", display), Style::default().fg(Color::White)),
                ])
            };
            ListItem::new(line)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            " CHỌN THƯ MỤC ĐÍCH ĐỂ GỘP (SELECT DESTINATION) ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner_area = block.inner(area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Prompt
            Constraint::Min(4),    // List of folders
            Constraint::Length(2), // Help line
        ])
        .split(inner_area);

    frame.render_widget(block, area);

    let prompt = Paragraph::new("Chọn thư mục sẽ nhận tất cả dữ liệu. Các thư mục khác sẽ bị xóa sau khi gộp:")
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(prompt, chunks[0]);

    let list = List::new(items)
        .block(Block::default().borders(Borders::NONE));
    frame.render_widget(list, chunks[1]);

    let help_line = Paragraph::new(" [Up/Down] Chọn thư mục đích | [Enter] Xem trước & Quét trùng | [Esc] Hủy ")
        .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC));
    frame.render_widget(help_line, chunks[2]);
}

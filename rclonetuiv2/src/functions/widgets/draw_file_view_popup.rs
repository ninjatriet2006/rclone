use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use crate::functions::*;

pub fn draw_file_view_popup(frame: &mut Frame, file_name: &str, content: &[String], scroll_offset: usize) {
    let size = frame.size();
    let area = centered_rect(75, 75, size);
    frame.render_widget(Clear, area);

    let height = area.height.saturating_sub(4) as usize; // reserve space for border and instructions
    let visible_lines: Vec<ListItem> = content
        .iter()
        .skip(scroll_offset)
        .take(height)
        .map(|line| ListItem::new(line.clone()))
        .collect();

    let footer = format!(" [Up/Down] Cuộn | [Esc] Thoát | Dòng {} - {} / {}", scroll_offset + 1, (scroll_offset + height).min(content.len()), content.len());
    let block = Block::default()
        .title(Span::styled(
            format!(" Xem file: {} ", file_name),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(visible_lines).block(block);
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);
    
    frame.render_widget(list, chunks[0]);
    frame.render_widget(Paragraph::new(footer).style(Style::default().fg(Color::DarkGray)), chunks[1]);
}

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use crate::app::App;

pub fn draw_dependency_manager(app: &App, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Hướng dẫn
            Constraint::Min(5),    // Danh sách
        ])
        .split(area);

    let welcome = Paragraph::new("Dùng các phím Mũi tên để di chuyển, Enter để cài đặt phụ thuộc đã chọn. Esc để quay lại Menu chính.")
        .style(Style::default())
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(welcome, chunks[0]);

    let fuse_status = if app.fuse_installed { "Đã cài đặt (Installed)" } else { "Chưa cài đặt (Not installed)" };
    let filen_status = if app.filen_cli_installed { "Đã cài đặt (Installed)" } else { "Chưa cài đặt (Not installed)" };

    let items = vec![
        ListItem::new(format!("1. Tiện ích FUSE (Hỗ trợ Mount ổ đĩa ảo) - Trạng thái: {}", fuse_status)),
        ListItem::new(format!("2. Filen CLI (Hỗ trợ đồng bộ Filen) - Trạng thái: {}", filen_status)),
    ];

    let styled_items: Vec<ListItem> = items
        .into_iter()
        .enumerate()
        .map(|(i, item)| {
            let style = if i == app.selected_dependency_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            item.style(style)
        })
        .collect();

    let list = List::new(styled_items).block(
        Block::default()
            .title(" QUẢN LÝ CÀI ĐẶT PHỤ THUỘC (DEPENDENCY MANAGER) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list, chunks[1]);
}

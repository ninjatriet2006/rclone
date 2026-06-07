use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use crate::functions::*;

pub fn draw_main_menu(state: &MenuState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Banner hướng dẫn
            Constraint::Min(10),   // Menu options
        ])
        .split(area);

    // Banner chào mừng
    let welcome_text = translate("menu_welcome");
    let welcome = Paragraph::new(welcome_text)
        .style(Style::default())
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(welcome, chunks[0]);

    // Các phần tử Menu
    let items: Vec<ListItem> = state
        .options
        .iter()
        .enumerate()
        .map(|(i, key)| {
            let style = if i == state.selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let text = translate(key);
            ListItem::new(text).style(style)
        })
        .collect();

    let title_text = translate("menu_title");
    let list = List::new(items).block(
        Block::default()
            .title(title_text)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list, chunks[1]);
}

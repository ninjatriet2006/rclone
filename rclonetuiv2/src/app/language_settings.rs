use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use crate::functions::*;

pub fn draw_language_settings(
    available_languages: &[String],
    selected_lang_idx: usize,
    active_language: &str,
    frame: &mut Frame,
    area: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Hướng dẫn
            Constraint::Min(5),    // Danh sách
        ])
        .split(area);

    let welcome = Paragraph::new(translate("lang_welcome"))
        .style(Style::default())
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(welcome, chunks[0]);

    let items: Vec<ListItem> = available_languages
        .iter()
        .enumerate()
        .map(|(i, lang)| {
            let is_active = lang == active_language;
            let text = if is_active {
                format!("* {} ({})", lang, translate("lang_active"))
            } else {
                format!("  {}", lang)
            };

            let style = if i == selected_lang_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(translate("lang_title"))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list, chunks[1]);
}

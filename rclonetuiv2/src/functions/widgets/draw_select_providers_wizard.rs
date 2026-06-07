use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, List, ListItem},
};
use crate::functions::*;

pub fn draw_select_providers_wizard(
    frame: &mut Frame,
    providers: &[(String, String, bool)],
    selected_idx: usize,
    scroll_offset: usize,
) {
    let size = frame.size();
    let area = centered_rect(65, 75, size);
    frame.render_widget(Clear, area);

    let height = area.height.saturating_sub(2) as usize;

    let items: Vec<ListItem> = providers
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(height)
        .map(|(i, (name, desc, checked))| {
            let checkbox = if *checked { "[X]" } else { "[ ]" };
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {} {} - {}", checkbox, name, desc)).style(style)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            translate("conn_wizard_provider_title"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

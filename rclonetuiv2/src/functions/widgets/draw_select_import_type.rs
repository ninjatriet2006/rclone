use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, List, ListItem},
};
use crate::functions::*;

pub fn draw_select_import_type(frame: &mut Frame, profile_name: &str, selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(55, 30, size);
    frame.render_widget(Clear, area);

    let types = vec![
        translate("prof_type_url"),
        translate("prof_type_local"),
    ];

    let items: Vec<ListItem> = types
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", t)).style(style)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            translate("prof_type_title").replace("{}", profile_name),
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

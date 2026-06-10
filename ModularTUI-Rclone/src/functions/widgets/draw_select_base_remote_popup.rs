use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, List, ListItem},
};
use crate::functions::*;

pub fn draw_select_base_remote_popup(frame: &mut Frame, remotes: &[String], selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(50, 45, size);
    frame.render_widget(Clear, area);

    let items: Vec<ListItem> = remotes
        .iter()
        .enumerate()
        .map(|(i, remote)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", remote)).style(style)
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            translate("exp_select_base_remote_title"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let height = area.height.saturating_sub(2) as usize;
    let range = calculate_scroll_range(selected_idx, items.len(), height);
    let visible_items: Vec<ListItem> = items.into_iter().skip(range.start).take(range.end - range.start).collect();

    let list = List::new(visible_items).block(block);
    frame.render_widget(list, area);
}

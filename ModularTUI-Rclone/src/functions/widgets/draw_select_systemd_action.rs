use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, List, ListItem},
};
use crate::functions::*;

pub fn draw_select_systemd_action(
    frame: &mut Frame,
    service_name: &str,
    is_user: bool,
    selected_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(50, 40, size);
    frame.render_widget(Clear, area);

    let level = if is_user {
        translate("srv_action_level_user")
    } else {
        translate("srv_action_level_system")
    };
    let block = Block::default()
        .title(Span::styled(
            translate("srv_action_title")
                .replacen("{}", service_name, 1)
                .replacen("{}", &level, 1),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let options = vec![
        translate("srv_action_start"),
        translate("srv_action_stop"),
        translate("srv_action_restart"),
        translate("srv_action_enable"),
        translate("srv_action_disable"),
        translate("srv_action_edit"),
    ];

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, option)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(option.clone()).style(style)
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

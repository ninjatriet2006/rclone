use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, List, ListItem},
};
use crate::functions::*;

pub fn draw_ask_mode(frame: &mut Frame, service_type: &ServiceType, selected_idx: usize) {
    let size = frame.size();
    let area = centered_rect(65, 30, size);
    frame.render_widget(Clear, area);

    let options = vec![
        translate("srv_mode_terminal"),
        translate("srv_mode_gui"),
        translate("srv_mode_advanced"),
    ];

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Magenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(opt.clone()).style(style)
        })
        .collect();

    let title_prefix = match service_type {
        ServiceType::Mount => "MOUNT",
        ServiceType::NfsMount => "NFS MOUNT",
        ServiceType::Serve => "SHARE",
        ServiceType::WebGui => "WEB GUI",
    };

    let block = Block::default()
        .title(Span::styled(
            translate("srv_select_mode_title").replace("{}", title_prefix),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

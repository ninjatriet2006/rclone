use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, List, ListItem},
};
use crate::functions::*;

pub fn draw_select_remote(
    frame: &mut Frame,
    remotes: &[String],
    selected_idx: usize,
    service_type: &ServiceType,
) {
    let size = frame.size();
    let area = centered_rect(55, 60, size);
    frame.render_widget(Clear, area);

    let local_desc = translate("srv_local_desc");
    let mut items = vec![ListItem::new(local_desc.clone())];
    items.extend(remotes.iter().enumerate().map(|(i, remote)| {
        let style = if i + 1 == selected_idx {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        ListItem::new(translate("srv_cloud_desc").replace("{}", remote)).style(style)
    }));

    if selected_idx == 0 {
        items[0] = ListItem::new(local_desc).style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        );
    }

    let title_prefix = match service_type {
        ServiceType::Mount => "MOUNT",
        ServiceType::NfsMount => "NFS MOUNT",
        ServiceType::Serve => "SHARE",
        ServiceType::WebGui => "WEB GUI",
    };

    let block = Block::default()
        .title(Span::styled(
            translate("srv_select_source_title").replace("{}", title_prefix),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let height = area.height.saturating_sub(2) as usize;
    let range = calculate_scroll_range(selected_idx, items.len(), height);
    let visible_items: Vec<ListItem> = items
        .into_iter()
        .skip(range.start)
        .take(range.end - range.start)
        .collect();

    let list = List::new(visible_items).block(block);
    frame.render_widget(list, area);
}

use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use crate::functions::*;

pub fn draw_headless_oauth_wizard(
    frame: &mut Frame,
    provider: &str,
    remote_name: &str,
    client_id: &str,
    client_secret: &str,
    token_input: &str,
    focused_idx: usize,
) {
    let size = frame.size();
    let area = centered_rect(65, 65, size);
    frame.render_widget(Clear, area);

    let cmd = if client_id.is_empty() {
        format!("rclone authorize \"{}\"", provider)
    } else {
        format!("rclone authorize \"{}\" \"{}\" \"{}\"", provider, client_id, client_secret)
    };

    let title = translate("conn_wizard_headless_title").replace("{}", remote_name);
    let prompt_tpl = translate("conn_wizard_headless_prompt");
    let prompt = prompt_tpl.replace("{}", &cmd);

    let mut text = vec![];
    for line in prompt.lines() {
        text.push(Line::from(line));
    }
    text.push(Line::from(""));
    text.push(Line::from("------------------------------------------------------------------"));

    let id_focused = focused_idx == 0;
    let sec_focused = focused_idx == 1;
    let tok_focused = focused_idx == 2;

    text.push(Line::from(vec![
        Span::styled("1. OAuth Client ID (Optional): ", Style::default().fg(if id_focused { Color::Yellow } else { Color::DarkGray })),
        Span::styled(client_id, Style::default().fg(Color::White).bg(if id_focused { Color::Blue } else { Color::DarkGray })),
    ]));
    text.push(Line::from(""));
    text.push(Line::from(vec![
        Span::styled("2. OAuth Client Secret (Optional): ", Style::default().fg(if sec_focused { Color::Yellow } else { Color::DarkGray })),
        Span::styled(client_secret, Style::default().fg(Color::White).bg(if sec_focused { Color::Blue } else { Color::DarkGray })),
    ]));
    text.push(Line::from(""));
    text.push(Line::from(vec![
        Span::styled("3. Paste Token JSON here: ", Style::default().fg(if tok_focused { Color::Yellow } else { Color::DarkGray })),
        Span::styled(token_input, Style::default().fg(Color::White).bg(if tok_focused { Color::Blue } else { Color::DarkGray })),
    ]));
    text.push(Line::from(""));
    text.push(Line::from(" [Tab] Chuyển trường | [Enter] Xác nhận | [Esc] Hủy "));

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(text).block(block).wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};
use crate::functions::parse_help_line;

pub fn draw_footer_bar(frame: &mut Frame, area: Rect, help_text: &str) {
    let help_paragraph = Paragraph::new(parse_help_line(help_text))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(help_paragraph, area);
}

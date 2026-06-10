use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

pub fn draw_header_bar(frame: &mut Frame, area: Rect, active_profile_name: &str, is_fuse_installed: bool) {
    let top_text = format!(
        " === Rclone TUI Engine === [Profile: {}] [FUSE: {}] [VFS Cache: Bật]",
        active_profile_name,
        if is_fuse_installed {
            "Đã cài đặt"
        } else {
            "Chưa cài đặt"
        }
    );
    let top_paragraph = Paragraph::new(top_text)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );
    frame.render_widget(top_paragraph, area);
}

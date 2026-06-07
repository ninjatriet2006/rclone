use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use crate::functions::*;

pub fn draw_permission_scanning_popup(
    frame: &mut Frame,
    src: &str,
    dest: &str,
    scanned_count: usize,
    total_files: usize,
    restricted_count: usize,
) {
    let size = frame.size();
    let area = centered_rect(65, 35, size);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " KIỂM TRA QUYỀN SỞ HỮU / TẢI XUỐNG (PERMISSION PRE-CHECK) ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let width = area.width.saturating_sub(6) as usize;
    let bar_str = if total_files > 0 {
        let pct = (scanned_count as f64 / total_files as f64) * 100.0;
        let filled = ((pct.min(100.0) * width as f64) / 100.0) as usize;
        let empty = width.saturating_sub(filled);
        format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
    } else {
        let block_size = 6.max(width / 10).min(width.saturating_sub(1));
        let mut chars = vec!['░'; width];
        if width > 0 {
            let offset = scanned_count % width;
            for i in 0..block_size {
                let idx = (offset + i) % width;
                chars[idx] = '█';
            }
        }
        let marquee_str: String = chars.into_iter().collect();
        format!("[{}]", marquee_str)
    };

    let scan_status_line = if total_files > 0 {
        let pct = (scanned_count as f64 / total_files as f64) * 100.0;
        Line::from(vec![
            Span::styled("Đang quét: ", Style::default().fg(Color::Cyan)),
            Span::styled(format!("{:.1}% ", pct), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(format!("({}/{} file) ", scanned_count, total_files), Style::default().fg(Color::White)),
        ])
    } else {
        Line::from(vec![
            Span::styled("Đang quét: ", Style::default().fg(Color::Cyan)),
            Span::styled(format!("{} tệp tin ", scanned_count), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("(đang phân tích thư mục...) ", Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
        ])
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("Nguồn: ", Style::default().fg(Color::DarkGray)),
            Span::styled(src.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Đích:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(dest.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        scan_status_line,
        Line::from(Span::styled(bar_str, Style::default().fg(Color::Green))),
        Line::from(""),
        Line::from(vec![
            Span::styled("Phát hiện bị chặn tải: ", Style::default().fg(Color::LightRed)),
            Span::styled(format!("{} tệp tin", restricted_count), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " Nhấn [Esc] để bỏ qua và chuyển tác vụ vào hàng chờ của Job Monitor ",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
        )),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

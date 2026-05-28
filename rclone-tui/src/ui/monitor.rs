use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

#[derive(Debug, Clone)]
pub struct TransferJob {
    pub name: String,
    pub size: u64,
    pub bytes: u64,
    pub speed: u64,
    pub percentage: u16,
    pub eta: i64,
    pub job_id: Option<i64>,
}

pub struct MonitorState {
    pub speed: f64,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub active_jobs: Vec<TransferJob>,
    pub selected_job_idx: usize,
    pub history: Vec<String>,
    pub confirm_stop_job: Option<TransferJob>,
}

impl MonitorState {
    pub fn new() -> Self {
        MonitorState {
            speed: 0.0,
            bytes_transferred: 0,
            total_bytes: 0,
            active_jobs: Vec::new(),
            selected_job_idx: 0,
            history: Vec::new(),
            confirm_stop_job: None,
        }
    }

    pub fn next(&mut self) {
        if !self.active_jobs.is_empty() {
            self.selected_job_idx = (self.selected_job_idx + 1) % self.active_jobs.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.active_jobs.is_empty() {
            if self.selected_job_idx == 0 {
                self.selected_job_idx = self.active_jobs.len() - 1;
            } else {
                self.selected_job_idx -= 1;
            }
        }
    }
}

pub fn draw(state: &MonitorState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),      // Tổng quan tiến trình (Global Stats)
            Constraint::Percentage(50), // Danh sách các tệp đang truyền tải (Active Jobs)
            Constraint::Min(5),         // Lịch sử truyền tải (History Logs)
            Constraint::Length(3),      // Help bar
        ])
        .split(area);

    // 1. Vẽ Tổng quan tiến trình
    let speed_str = format!("{}/s", super::format_size(state.speed as u64));
    let progress_pct = if state.total_bytes > 0 {
        (state.bytes_transferred as f64 / state.total_bytes as f64) * 100.0
    } else {
        0.0
    };

    let stats_text = vec![
        Line::from(vec![
            Span::raw(crate::lang::translate("mon_speed_label")),
            Span::styled(
                speed_str,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(crate::lang::translate("mon_transferred_label")),
            Span::styled(
                super::format_size(state.bytes_transferred),
                Style::default().fg(Color::Green),
            ),
            Span::raw(" / "),
            Span::styled(
                super::format_size(state.total_bytes),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::raw(crate::lang::translate("mon_total_pct_label")),
            Span::styled(
                format!("{:.1}%", progress_pct),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let stats_block = Block::default()
        .title(Span::styled(
            crate::lang::translate("mon_stats_title"),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let stats_paragraph = Paragraph::new(stats_text).block(stats_block);
    frame.render_widget(stats_paragraph, chunks[0]);

    // 2. Vẽ các Job đang chạy
    let active_items: Vec<ListItem> = state
        .active_jobs
        .iter()
        .enumerate()
        .map(|(i, job)| {
            let style = if i == state.selected_job_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let line = if let Some(_id) = job.job_id {
                Line::from(vec![
                    Span::styled("⚡ ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format!("{} ", job.name),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ])
            } else {
                let eta_str = if job.eta >= 0 {
                    format!("ETA: {}s", job.eta)
                } else {
                    "ETA: --".to_string()
                };

                Line::from(vec![
                    Span::raw(format!("  {}% ", job.percentage)),
                    Span::styled(
                        format!("{} ", job.name),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(
                            "({} / {}) ",
                            super::format_size(job.bytes),
                            super::format_size(job.size)
                        ),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(
                        format!("{} - ", eta_str),
                        Style::default().fg(Color::Magenta),
                    ),
                    Span::styled(
                        format!("{}/s", super::format_size(job.speed)),
                        Style::default().fg(Color::Yellow),
                    ),
                ])
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let active_block = Block::default()
        .title(Span::styled(
            crate::lang::translate("mon_active_title"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let active_list = List::new(active_items).block(active_block);
    frame.render_widget(active_list, chunks[1]);

    // 3. Vẽ lịch sử truyền tải (History Logs)
    // Giới hạn hiển thị lịch sử gần nhất để tránh lag (Bug 96)
    let history_items: Vec<ListItem> = state
        .history
        .iter()
        .rev()
        .take(50)
        .map(|log| ListItem::new(Span::raw(log)))
        .collect();

    let history_block = Block::default()
        .title(Span::styled(
            crate::lang::translate("mon_history_title"),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let history_list = List::new(history_items).block(history_block);
    frame.render_widget(history_list, chunks[2]);

    // Help Bar
    let help_paragraph = Paragraph::new(
        crate::lang::translate("mon_help"),
    )
    .style(Style::default().fg(Color::DarkGray))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(help_paragraph, chunks[3]);

    // Vẽ popup xác nhận dừng tác vụ
    if let Some(job) = &state.confirm_stop_job {
        let overlay_area = super::centered_rect(60, 25, area);
        frame.render_widget(Clear, overlay_area);

        let popup_block = Block::default()
            .title(Span::styled(
                " XÁC NHẬN HỦY BỎ TÁC VỤ ",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));

        let text = vec![
            Line::from(vec![
                Span::raw("Bạn có chắc chắn muốn dừng và hủy bỏ tác vụ sau:"),
            ]),
            Line::from(vec![
                Span::styled(
                    format!(" {}", job.name),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" [Enter] Đồng ý ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" | "),
                Span::styled(" [Esc] Hủy bỏ ", Style::default().fg(Color::Gray)),
            ]),
        ];

        let paragraph = Paragraph::new(text)
            .block(popup_block)
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(paragraph, overlay_area);
    }
}

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use crate::functions::*;

fn make_progress_bar(percentage: u16, width: usize) -> String {
    let capped = percentage.min(100) as usize;
    let filled = (capped * width) / 100;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

pub fn draw_job_monitor(state: &MonitorState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),      // Tổng quan tiến trình (Global Stats)
            Constraint::Percentage(55), // Khung ở giữa (Active, Pending & Failed)
            Constraint::Min(5),         // Chi tiết tác vụ đang chọn
            Constraint::Length(3),      // Help bar
        ])
        .split(area);

    let mid_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), // Active & Pending Jobs
            Constraint::Percentage(40), // Failed & Restricted Files
        ])
        .split(chunks[1]);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60), // Active Jobs
            Constraint::Percentage(40), // Pending Jobs
        ])
        .split(mid_chunks[0]);

    // 1. Vẽ Tổng quan tiến trình
    let speed_str = format!("{}/s", format_size(state.speed as u64));
    let progress_pct = if state.active_jobs.is_empty() {
        100.0
    } else if state.total_bytes > 0 {
        (state.bytes_transferred as f64 / state.total_bytes as f64) * 100.0
    } else {
        0.0
    };

    let stats_text = vec![
        Line::from(vec![
            Span::raw(translate("mon_speed_label")),
            Span::styled(
                speed_str,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(translate("mon_transferred_label")),
            Span::styled(
                format_size(state.bytes_transferred),
                Style::default().fg(Color::Green),
            ),
            Span::raw(" / "),
            Span::styled(
                format_size(state.total_bytes),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(" | PID Engine: "),
            Span::styled(
                std::process::id().to_string(),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw(translate("mon_total_pct_label")),
            Span::styled(
                format!("{} {:.1}%", make_progress_bar(progress_pct as u16, 25), progress_pct),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let stats_block = Block::default()
        .title(Span::styled(
            translate("mon_stats_title"),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let stats_paragraph = Paragraph::new(stats_text).block(stats_block);
    frame.render_widget(stats_paragraph, chunks[0]);

    // 2. Vẽ các Job đang chạy (Active Jobs)
    let is_active_focused = state.active_pane == MonitorPane::ActiveJobs;
    let active_border_color = if is_active_focused { Color::Yellow } else { Color::DarkGray };

    let active_items: Vec<ListItem> = state
        .active_jobs
        .iter()
        .enumerate()
        .map(|(i, job)| {
            let style = if is_active_focused && i == state.selected_job_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let line = if let Some(_id) = job.job_id {
                let bar = make_progress_bar(job.percentage, 12);
                let eta_str = if job.eta >= 0 {
                    format!("ETA: {}s", job.eta)
                } else {
                    "ETA: --".to_string()
                };

                let mut spans = vec![
                    Span::styled("⚡ ", Style::default().fg(Color::Yellow)),
                    Span::styled(format!(" {} ", bar), Style::default().fg(Color::Green)),
                    Span::styled(
                        format!("{}% ", job.percentage),
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{} ", job.name),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ];

                if job.size > 0 {
                    spans.push(Span::styled(
                        format!(
                            "({} / {}) ",
                            format_size(job.bytes),
                            format_size(job.size)
                        ),
                        Style::default().fg(Color::DarkGray),
                    ));
                }

                spans.push(Span::styled(
                    format!("{} - ", eta_str),
                    Style::default().fg(Color::Magenta),
                ));
                spans.push(Span::styled(
                    format!("{}/s", format_size(job.speed)),
                    Style::default().fg(Color::Yellow),
                ));

                Line::from(spans)
            } else {
                let eta_str = if job.eta >= 0 {
                    format!("ETA: {}s", job.eta)
                } else {
                    "ETA: --".to_string()
                };

                let bar = make_progress_bar(job.percentage, 12);
                Line::from(vec![
                    Span::styled(format!(" {} ", bar), Style::default().fg(Color::Green)),
                    Span::styled(
                        format!("{}% ", job.percentage),
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{} ", job.name),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(
                            "({} / {}) ",
                            format_size(job.bytes),
                            format_size(job.size)
                        ),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("{} - ", eta_str),
                        Style::default().fg(Color::Magenta),
                    ),
                    Span::styled(
                        format!("{}/s", format_size(job.speed)),
                        Style::default().fg(Color::Yellow),
                    ),
                ])
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let active_block = Block::default()
        .title(Span::styled(
            format!(" {} ", translate("mon_active_title")),
            Style::default()
                .fg(active_border_color)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(active_border_color));
    let active_list = List::new(active_items).block(active_block);
    frame.render_widget(active_list, left_chunks[0]);

    // 3. Vẽ các tác vụ đang chờ xác nhận (Pending Jobs)
    let is_pending_focused = state.active_pane == MonitorPane::PendingJobs;
    let pending_border_color = if is_pending_focused { Color::Yellow } else { Color::DarkGray };

    let pending_items: Vec<ListItem> = state
        .pending_jobs
        .iter()
        .enumerate()
        .map(|(i, job)| {
            let style = if is_pending_focused && i == state.selected_pending_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let status_tag = match job.status.as_str() {
                "Bypassed" => "[ĐÃ BỎ QUA - CẦN QUYẾT ĐỊNH]",
                "Scanned (Has Restrictions)" => "[CÓ FILE BỊ CHẶN TẢI]",
                "Scanned (No Restrictions)" => "[AN TOÀN - KHÔNG BỊ CHẶN]",
                _ => "[CHỜ]",
            };

            let status_color = match job.status.as_str() {
                "Scanned (Has Restrictions)" => Color::Red,
                "Scanned (No Restrictions)" => Color::Green,
                _ => Color::Yellow,
            };

            let line = Line::from(vec![
                Span::styled("⚠️ ", Style::default().fg(Color::Yellow)),
                Span::styled(format!("{} ", status_tag), Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{} -> {} ", job.src, job.dest), Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("(Bị chặn {}/{} file)", job.restricted_files.len(), job.total_files),
                    Style::default().fg(Color::LightRed)
                ),
            ]);

            ListItem::new(line).style(style)
        })
        .collect();

    let pending_block = Block::default()
        .title(Span::styled(
            " TÁC VỤ SAO CHÉP CHỜ XÁC NHẬN (PENDING) ",
            Style::default()
                .fg(pending_border_color)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(pending_border_color));
    let pending_list = List::new(pending_items).block(pending_block);
    frame.render_widget(pending_list, left_chunks[1]);

    // 4. Vẽ danh sách các file lỗi hoặc file bị hạn chế (Failed / Restricted Files)
    let (right_panel_title, right_panel_items) = if state.active_pane == MonitorPane::PendingJobs && !state.pending_jobs.is_empty() {
        let job = &state.pending_jobs[state.selected_pending_idx];
        let title = format!(" FILE BỊ KHÓA CỦA JOB ĐANG CHỌN ({}) ", job.restricted_files.len());
        let items: Vec<ListItem> = job.restricted_files.iter().map(|f| {
            ListItem::new(Line::from(vec![
                Span::styled("🔒 ", Style::default().fg(Color::Red)),
                Span::styled(f.clone(), Style::default().fg(Color::White)),
            ]))
        }).collect();
        (title, items)
    } else {
        let title = " CÁC FILE BỊ LỖI QUYỀN / KHÔNG SAO CHÉP ĐƯỢC ".to_string();
        let items: Vec<ListItem> = state
            .failed_files
            .iter()
            .map(|item| {
                let line = Line::from(vec![
                    Span::styled("❌ ", Style::default().fg(Color::Red)),
                    Span::styled(format!("[{}] ", item.time), Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{} ", item.src), Style::default().fg(Color::White)),
                    Span::styled(format!("(Lỗi: {})", item.error), Style::default().fg(Color::Red)),
                ]);
                ListItem::new(line)
            })
            .collect();
        (title, items)
    };

    let right_block = Block::default()
        .title(Span::styled(
            right_panel_title,
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let right_list = List::new(right_panel_items).block(right_block);
    frame.render_widget(right_list, mid_chunks[1]);

    // 5. Vẽ nhật ký debug/chi tiết tác vụ đang chọn
    let details_block = Block::default()
        .title(Span::styled(
            " NHẬT KÝ DEBUG & CHI TIẾT TÁC VỤ ĐANG CHỌN ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let details_text = if state.active_jobs.is_empty() {
        vec![Line::from(vec![
            Span::styled("[DEBUG] Không có tác vụ rclone nào đang chạy.", Style::default().fg(Color::Black)),
        ])]
    } else if state.selected_job_idx < state.active_jobs.len() {
        let job = &state.active_jobs[state.selected_job_idx];
        let mut lines = Vec::new();

        lines.push(Line::from(vec![
            Span::styled(
                format!(
                    "[DEBUG] Tác vụ: {} | Job ID: {}",
                    job.name,
                    job.job_id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "Không có".to_string())
                ),
                Style::default().fg(Color::Black),
            ),
        ]));

        if job.job_id.is_some() {
            lines.push(Line::from(vec![
                Span::styled(
                    format!(
                        "[DEBUG] Nhóm thống kê: {} | Bắt đầu: {} | Đã chạy: {:.1}s",
                        job.group, job.start_time, job.duration
                    ),
                    Style::default().fg(Color::Black),
                ),
            ]));
        }

        lines.push(Line::from(vec![
            Span::styled(
                format!(
                    "[DEBUG] Tiến độ: {}% | Tốc độ: {}/s | Truyền tải: {} / {}",
                    job.percentage,
                    format_size(job.speed),
                    format_size(job.bytes),
                    format_size(job.size)
                ),
                Style::default().fg(Color::Black),
            ),
        ]));

        if !job.description.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("[DEBUG] Lệnh/Mô tả đầy đủ: {}", job.description),
                    Style::default().fg(Color::Black),
                ),
            ]));
        }

        lines
    } else {
        vec![Line::from(vec![
            Span::styled("[DEBUG] Vui lòng chọn một tác vụ phía trên.", Style::default().fg(Color::Black)),
        ])]
    };

    let details_paragraph = Paragraph::new(details_text).block(details_block);
    frame.render_widget(details_paragraph, chunks[2]);

    // Help Bar
    let help_text = if state.active_pane == MonitorPane::PendingJobs {
        " [Tab] Chuyển Khung | [Up/Down] Chọn | [Enter/C] Giải quyết | [Delete/D] Xóa Job | [Esc] Quay lại "
    } else {
        " [Tab] Chuyển Khung | [Up/Down] Chọn | [Delete/D] Dừng Job | [Esc] Quay lại "
    };
    let help_paragraph = Paragraph::new(
        parse_help_line(help_text),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(help_paragraph, chunks[3]);

    // Vẽ popup xác nhận dừng tác vụ
    if let Some(job) = &state.confirm_stop_job {
        let overlay_area = centered_rect(60, 25, area);
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

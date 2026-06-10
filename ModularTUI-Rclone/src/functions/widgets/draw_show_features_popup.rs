use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use crate::functions::*;

pub fn get_feature_description(key: &str) -> &'static str {
    match key {
        "About" => "Xem dung lượng đĩa",
        "BucketBased" => "Lưu trữ dạng bucket (S3/B2)",
        "BucketBasedRootOK" => "Thao tác trên root bucket",
        "CanHaveEmptyDirectories" => "Tạo thư mục rỗng",
        "CaseInsensitive" => "Không phân biệt hoa/thường",
        "ChangeNotify" => "Thông báo thay đổi tệp",
        "ChunkWriterDoesntSeek" => "Ghi phân đoạn không seek",
        "CleanUp" => "Dọn dẹp / Xóa thùng rác",
        "Command" => "Chạy lệnh backend riêng",
        "Copy" => "Sao chép trực tiếp trên Cloud",
        "DirCacheFlush" => "Xóa bộ đệm thư mục",
        "DirModTimeUpdatesOnWrite" => "Cập nhật thời gian thư mục cha",
        "DirMove" => "Di chuyển thư mục trên Cloud",
        "DirSetModTime" => "Đặt thời gian thư mục",
        "Disconnect" => "Đăng xuất / Ngắt kết nối",
        "DoubleSlash" => "Hỗ trợ đường dẫn //",
        "DuplicateFiles" => "Cho phép tệp trùng tên",
        "FilterAware" => "Bộ lọc file trên backend",
        "GetTier" => "Xem phân tầng lưu trữ (Tier)",
        "IsLocal" => "Ổ đĩa cục bộ (Local)",
        "ListP" => "Liệt kê thư mục tối ưu",
        "ListR" => "Liệt kê đệ quy tối ưu",
        "MergeDirs" => "Hợp nhất thư mục trùng tên",
        "MkdirMetadata" => "Tạo thư mục kèm Metadata",
        "Move" => "Di chuyển tệp trực tiếp trên Cloud",
        "NoMultiThreading" => "Không tải đa luồng",
        "OpenChunkWriter" => "Ghi phân đoạn lớn",
        "OpenWriterAt" => "Ghi tệp tại vị trí chỉ định",
        "Overlay" => "Lớp phủ hệ thống tệp",
        "PartialUploads" => "Hỗ trợ tải lên dang dở",
        "PublicLink" => "Tạo liên kết chia sẻ công khai",
        "Purge" => "Xóa sạch thư mục (đệ quy)",
        "PutStream" => "Tải lên dạng luồng dữ liệu",
        "PutUnchecked" => "Tải lên không check checksum",
        "ReadDirMetadata" => "Đọc Metadata của thư mục",
        "ReadMetadata" => "Đọc Metadata của tệp",
        "ReadMimeType" => "Đọc định dạng tệp (MIME)",
        "ServerSideAcrossConfigs" => "Sao chép/Di chuyển xuyên Cloud",
        "SetTier" => "Đặt phân tầng lưu trữ (Tier)",
        "SetWrapper" => "Bao bọc luồng ghi dữ liệu",
        "Shutdown" => "Đóng kết nối an toàn",
        "SlowHash" => "Tính mã hash chậm",
        "SlowModTime" => "Lấy thời gian sửa đổi chậm",
        "UnWrap" => "Mở gói để lấy backend gốc",
        "UserDirMetadata" => "Đặt Metadata thư mục tự chọn",
        "UserInfo" => "Xem thông tin tài khoản Cloud",
        "UserMetadata" => "Đặt Metadata tệp tin tự chọn",
        "WrapFs" => "Hệ thống tệp bao bọc (Wrapper)",
        "WriteDirMetadata" => "Ghi Metadata cho thư mục",
        "WriteDirSetModTime" => "Đặt thời gian cho thư mục",
        "WriteMetadata" => "Ghi Metadata cho tệp",
        "WriteMimeType" => "Đặt định dạng tệp (MIME) khi upload",
        _ => "Tính năng backend",
    }
}

pub fn draw_show_features_popup(
    frame: &mut Frame,
    remote_name: &str,
    features: &[(String, bool)],
    union_remotes_features: &Option<Vec<(String, Vec<(String, bool)>)>>,
) {
    let size = frame.size();
    let area = centered_rect(85, 75, size);
    frame.render_widget(Clear, area);

    let mut lines = Vec::new();

    let mut has_mismatch = false;
    if let Some(upstreams) = union_remotes_features {
        let mut all_keys = std::collections::BTreeSet::new();
        for (_, list) in upstreams {
            for (k, _) in list {
                all_keys.insert(k.clone());
            }
        }

        for key in &all_keys {
            let mut first_val = None;
            for (_, list) in upstreams {
                if let Some((_, val)) = list.iter().find(|(k, _)| k == key) {
                    if let Some(f) = first_val {
                        if f != val {
                            has_mismatch = true;
                            break;
                        }
                    } else {
                        first_val = Some(val);
                    }
                }
            }
            if has_mismatch {
                break;
            }
        }
    }

    let title = if union_remotes_features.is_some() {
        format!(" KIỂM TRA TÍNH NĂNG BACKEND (UNION REMOTE: {}) ", remote_name)
    } else {
        format!(" TÍNH NĂNG BACKEND HỖ TRỢ: {} ", remote_name)
    };

    let block_border_color = if has_mismatch { Color::Red } else { Color::Green };
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(block_border_color)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(block_border_color));

    if has_mismatch {
        lines.push(Line::from(vec![
            Span::styled("  ⚠ CẢNH BÁO: Các cloud thành viên không đồng đẳng! (Thiếu/Khác biệt tính năng)  ", Style::default().fg(Color::Black).bg(Color::Red).add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::from(""));
    }

    if let Some(upstreams) = union_remotes_features {
        lines.push(Line::from(vec![
            Span::styled("So sánh tính năng của các cloud thành viên:", Style::default().add_modifier(Modifier::UNDERLINED)),
        ]));
        lines.push(Line::from(""));

        let mut header_spans = vec![
            Span::styled(format!("{:<22}", "Tính năng (Feature)"), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ];
        for (u_name, _) in upstreams {
            header_spans.push(Span::raw(" | "));
            let display_name = if u_name.len() > 12 {
                format!("{}.", &u_name[..11])
            } else {
                format!("{:<12}", u_name)
            };
            header_spans.push(Span::styled(display_name, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        }
        header_spans.push(Span::raw(" | "));
        header_spans.push(Span::styled("Giải thích (Mô tả)", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        lines.push(Line::from(header_spans));

        let total_header_len = 22 + upstreams.len() * 15 + 30;
        lines.push(Line::from("-".repeat(total_header_len)));

        let mut all_keys = std::collections::BTreeSet::new();
        for (_, list) in upstreams {
            for (k, _) in list {
                all_keys.insert(k.clone());
            }
        }

        for key in all_keys {
            let mut key_mismatch = false;
            let mut first_val = None;
            for (_, list) in upstreams {
                if let Some((_, val)) = list.iter().find(|(k, _)| k == &key) {
                    if let Some(f) = first_val {
                        if f != val {
                            key_mismatch = true;
                        }
                    } else {
                        first_val = Some(val);
                    }
                }
            }

            let name_style = if key_mismatch {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };

            let prefix = if key_mismatch { "⚠ " } else { "  " };
            let mut row_spans = vec![
                Span::styled(format!("{}{:<20}", prefix, key), name_style),
            ];

            for (_, list) in upstreams {
                row_spans.push(Span::raw(" | "));
                if let Some((_, val)) = list.iter().find(|(k, _)| k == &key) {
                    if *val {
                        row_spans.push(Span::styled("  [ YES ]   ", Style::default().fg(Color::Green)));
                    } else {
                        row_spans.push(Span::styled("  [  NO ]   ", Style::default().fg(Color::Red)));
                    }
                } else {
                    row_spans.push(Span::styled("  [ N/A ]   ", Style::default().fg(Color::DarkGray)));
                }
            }
            row_spans.push(Span::raw(" | "));
            row_spans.push(Span::styled(get_feature_description(&key), Style::default().fg(Color::Gray)));
            lines.push(Line::from(row_spans));
        }
    } else {
        lines.push(Line::from(vec![
            Span::styled("Danh sách các tính năng được hỗ trợ bởi backend này:", Style::default().add_modifier(Modifier::UNDERLINED)),
        ]));
        lines.push(Line::from(""));

        for (k, v) in features {
            let val_span = if *v {
                Span::styled("  [ HỖ TRỢ ]  ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            } else {
                Span::styled("  [ KHÔNG ]   ", Style::default().fg(Color::Red))
            };
            let desc = get_feature_description(k);
            lines.push(Line::from(vec![
                Span::styled(format!("  - {:<22}: ", k), Style::default().fg(Color::Cyan)),
                val_span,
                Span::raw(" - "),
                Span::styled(desc, Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(" Nhấn [Esc] hoặc [Enter] để quay lại "));

    let list = Paragraph::new(lines).block(block).wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(list, area);
}

use crate::app::{App, AppEvent, Screen, DeleteTarget};
use crate::functions::*;
use crate::functions::rclone;
use crate::functions::ui_helpers as ui;
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
use std::process::Command;

pub async fn handle_services_keys(
    app: &mut App,
    key: KeyEvent,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
) {
        let wizard = app.services_state.wizard.clone();
        match wizard {
            ServicesWizardState::None => {
                match key.code {
                    KeyCode::Esc => {
                        app.screen = Screen::MainMenu;
                    }
                    KeyCode::Tab => {
                        let limit = if cfg!(target_os = "windows") { 2 } else { 3 };
                        app.services_state.active_focus = (app.services_state.active_focus + 1) % limit;
                    }
                    KeyCode::Up => {
                        match app.services_state.active_focus {
                            0 => app.services_state.prev_menu(),
                            1 => app.services_state.prev_active(),
                            _ => app.services_state.prev_systemd(),
                        }
                    }
                    KeyCode::Down => {
                        match app.services_state.active_focus {
                            0 => app.services_state.next_menu(),
                            1 => app.services_state.next_active(),
                            _ => app.services_state.next_systemd(),
                        }
                    }
                    KeyCode::Enter => {
                        if app.services_state.active_focus == 0 {
                            // Khởi động wizard tạo dịch vụ mới
                            let service_type = match app.services_state.selected_menu_idx {
                                0 => ServiceType::Mount,
                                1 => ServiceType::NfsMount,
                                2 => ServiceType::WebGui,
                                3 => ServiceType::Serve,
                                _ => return,
                            };

                            // Web GUI không cần chọn Remote, chuyển thẳng sang hỏi flags
                            if service_type == ServiceType::WebGui {
                                app.services_state.wizard = ServicesWizardState::AskFlags {
                                    service_type,
                                    remote: String::new(),
                                    path: String::new(),
                                    protocol: None,
                                    flags: vec![
                                        ("--rc-addr".to_string(), "Nhập địa chỉ cổng Web GUI (Mặc định: localhost:5572)".to_string(), "localhost:5572".to_string(), String::new()),
                                        ("--rc-no-auth".to_string(), "Bạn có muốn bỏ qua bảo mật đăng nhập Web GUI? (y/N)".to_string(), "n".to_string(), String::new()),
                                    ],
                                    current_flag_idx: 0,
                                    input_buffer: String::new(),
                                    is_simple_terminal: false,
                                    is_editing: false,
                                };
                            } else if service_type == ServiceType::Mount || service_type == ServiceType::NfsMount {
                                // Chọn chế độ Simple hay Advanced
                                app.services_state.wizard =
                                    ServicesWizardState::AskMode {
                                        service_type,
                                        selected_idx: 0,
                                    };
                            } else {
                                // Cần chọn Remote cho Serve
                                app.services_state.wizard =
                                    ServicesWizardState::SelectRemote {
                                        service_type,
                                        remotes: app.connection_state.remotes.clone(),
                                        selected_idx: 0,
                                        is_simple_terminal: false,
                                        is_simple_gui: false,
                                    };
                            }
                        } else if app.services_state.active_focus == 2 {
                            if !app.services_state.systemd_services.is_empty() {
                                let idx = app.services_state.selected_systemd_idx;
                                let svc = &app.services_state.systemd_services[idx];
                                match app.load_systemd_service_fields(&svc.file_path, svc.is_user) {
                                    Ok(fields) => {
                                        app.services_state.wizard = ServicesWizardState::EditSystemdService {
                                            service_name: svc.name.clone(),
                                            file_path: svc.file_path.clone(),
                                            is_user: svc.is_user,
                                            fields,
                                            selected_idx: 0,
                                            scroll_offset: 0,
                                            is_editing: false,
                                            input_buffer: String::new(),
                                            active_tab: 0,
                                            adding_new_key: false,
                                            new_key_buffer: String::new(),
                                        };
                                    }
                                    Err(e) => {
                                        app.services_state.error_message = Some(format!(
                                            "Lỗi đọc file cấu hình: {}", e
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char(' ') => {
                        if app.services_state.active_focus == 2 {
                            if !app.services_state.systemd_services.is_empty() {
                                let idx = app.services_state.selected_systemd_idx;
                                let svc = &app.services_state.systemd_services[idx];
                                app.services_state.wizard = ServicesWizardState::SelectSystemdAction {
                                    service_name: svc.name.clone(),
                                    file_path: svc.file_path.clone(),
                                    is_user: svc.is_user,
                                    selected_idx: 0,
                                };
                            }
                        }
                    }
                    KeyCode::Insert => {
                        if app.services_state.active_focus == 2 {
                            let fields = app.init_create_systemd_service_fields();
                            app.services_state.wizard = ServicesWizardState::CreateSystemdService {
                                fields,
                                selected_idx: 0,
                                scroll_offset: 0,
                                is_editing: false,
                                input_buffer: String::new(),
                                active_tab: 0,
                                adding_new_key: false,
                                new_key_buffer: String::new(),
                            };
                        }
                    }
                    KeyCode::Delete => {
                        if app.services_state.active_focus == 1 {
                            if !app.services_state.active_services.is_empty() {
                                let idx = app.services_state.selected_active_idx;
                                app.delete_confirm = Some(DeleteTarget::Service(idx));
                            }
                        } else if app.services_state.active_focus == 2 {
                            if !app.services_state.systemd_services.is_empty() {
                                let idx = app.services_state.selected_systemd_idx;
                                app.delete_confirm = Some(DeleteTarget::SystemdService(idx));
                            }
                        }
                    }
                    _ => {}
                }
            }
            ServicesWizardState::AskMode {
                service_type,
                mut selected_idx,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        app.services_state.wizard = ServicesWizardState::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = 2;
                        } else {
                            selected_idx -= 1;
                        }
                        app.services_state.wizard =
                            ServicesWizardState::AskMode {
                                service_type,
                                selected_idx,
                            };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % 3;
                        app.services_state.wizard =
                            ServicesWizardState::AskMode {
                                service_type,
                                selected_idx,
                            };
                    }
                    KeyCode::Enter => {
                        let is_simple_terminal = selected_idx == 0;
                        let is_simple_gui = selected_idx == 1;

                        // Tìm remote Secret để preselect nếu có
                        let mut default_idx = 0;
                        for (i, r) in app.connection_state.remotes.iter().enumerate() {
                            if r.to_lowercase() == "secret" {
                                default_idx = i + 1;
                                break;
                            }
                        }

                        app.services_state.wizard =
                            ServicesWizardState::SelectRemote {
                                service_type,
                                remotes: app.connection_state.remotes.clone(),
                                selected_idx: default_idx,
                                is_simple_terminal,
                                is_simple_gui,
                            };
                    }
                    _ => {}
                }
            }
            ServicesWizardState::SelectRemote {
                service_type,
                remotes,
                mut selected_idx,
                is_simple_terminal,
                is_simple_gui,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        app.services_state.wizard = ServicesWizardState::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = remotes.len();
                        } else {
                            selected_idx -= 1;
                        }
                        app.services_state.wizard =
                            ServicesWizardState::SelectRemote {
                                service_type,
                                remotes,
                                selected_idx,
                                is_simple_terminal,
                                is_simple_gui,
                            };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % (remotes.len() + 1);
                        app.services_state.wizard =
                            ServicesWizardState::SelectRemote {
                                service_type,
                                remotes,
                                selected_idx,
                                is_simple_terminal,
                                is_simple_gui,
                            };
                    }
                    KeyCode::Enter => {
                        let remote = if selected_idx == 0 {
                            String::new() // Local
                        } else {
                            format!("{}:", remotes[selected_idx - 1])
                        };

                        if is_simple_gui {
                            app.services_state.wizard = ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote: remote.clone(),
                                current_path: "/".to_string(),
                                items: Vec::new(),
                                selected_idx: 0,
                                loading: true,
                                error_msg: None,
                                creating_folder: None,
                            };
                            app.refresh_wizard_gui_list(tx).await;
                        } else {
                            let default_path = if remote.to_lowercase().starts_with("secret:") {
                                "Khobaomat".to_string()
                            } else {
                                String::new()
                            };
                            app.services_state.edit_cursor_idx = default_path.chars().count();
                            app.services_state.wizard = ServicesWizardState::InputPath {
                                service_type,
                                remote,
                                input_buffer: default_path,
                                is_simple_terminal,
                            };
                        }
                    }
                    _ => {}
                }
            }
            ServicesWizardState::InputPath {
                service_type,
                remote,
                mut input_buffer,
                is_simple_terminal,
            } => {
                let mut cursor = app.services_state.edit_cursor_idx;
                if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                    app.services_state.edit_cursor_idx = cursor;
                    app.services_state.wizard = ServicesWizardState::InputPath {
                        service_type,
                        remote,
                        input_buffer,
                        is_simple_terminal,
                    };
                } else {
                    match key.code {
                        KeyCode::Esc => {
                            app.services_state.wizard = ServicesWizardState::None;
                        }
                        KeyCode::Enter => {
                        let path = input_buffer.trim().to_string();

                        if service_type == ServiceType::Mount || service_type == ServiceType::NfsMount {
                            let flags = if cfg!(target_os = "windows") {
                                if is_simple_terminal {
                                    vec![
                                        ("mount_point".to_string(), "Nhập ổ đĩa mount cục bộ (Mặc định: X:)".to_string(), "X:".to_string(), String::new()),
                                    ]
                                } else {
                                    vec![
                                        ("mount_point".to_string(), "Nhập ổ đĩa mount cục bộ (Ví dụ: X: hoặc Y:)".to_string(), "X:".to_string(), String::new()),
                                        ("--vfs-cache-mode".to_string(), "Chế độ cache VFS (off, minimal, writes, full - Mặc định: writes)".to_string(), "writes".to_string(), String::new()),
                                        ("--dir-cache-time".to_string(), "Thời gian cache thư mục (Ví dụ: 5m, 1h - Mặc định: 5m)".to_string(), "5m".to_string(), String::new()),
                                        ("--read-only".to_string(), "Giới hạn ổ đĩa chỉ đọc (Read Only)? (y/N)".to_string(), "n".to_string(), String::new()),
                                    ]
                                }
                            } else {
                                if is_simple_terminal {
                                    vec![
                                        ("mount_point".to_string(), "Nhập thư mục mount cục bộ (Mặc định: /mnt/drive)".to_string(), "/mnt/drive".to_string(), String::new()),
                                    ]
                                } else {
                                    vec![
                                        ("mount_point".to_string(), "Nhập thư mục mount cục bộ (Ví dụ: /mnt/drive hoặc ~/Cloud)".to_string(), "/mnt/drive".to_string(), String::new()),
                                        ("--vfs-cache-mode".to_string(), "Chế độ cache VFS (off, minimal, writes, full - Mặc định: writes)".to_string(), "writes".to_string(), String::new()),
                                        ("--dir-cache-time".to_string(), "Thời gian cache thư mục (Ví dụ: 5m, 1h - Mặc định: 5m)".to_string(), "5m".to_string(), String::new()),
                                        ("--read-only".to_string(), "Giới hạn ổ đĩa chỉ đọc (Read Only)? (y/N)".to_string(), "n".to_string(), String::new()),
                                        ("--daemon".to_string(), "Chạy ngầm tiến trình mount dưới nền? (Y/n)".to_string(), "y".to_string(), String::new()),
                                    ]
                                }
                            };
                            // Chuyển sang hỏi Flags cho Mount
                            app.services_state.wizard = ServicesWizardState::AskFlags {
                                service_type,
                                remote,
                                path,
                                protocol: None,
                                flags,
                                current_flag_idx: 0,
                                input_buffer: String::new(),
                                is_simple_terminal,
                                is_editing: false,
                            };
                        } else {
                            // Chia sẻ mạng: Chọn giao thức trước
                            app.services_state.wizard =
                                ServicesWizardState::SelectProtocol {
                                    remote,
                                    path,
                                    selected_idx: 0,
                                };
                        }
                    }
                    _ => {}
                }
              }
            }
            ServicesWizardState::GuiSelectPath {
                service_type,
                remote,
                mut current_path,
                items,
                mut selected_idx,
                loading,
                error_msg,
                creating_folder,
            } => {
                if let Some(mut input_buffer) = creating_folder {
                    match key.code {
                        KeyCode::Esc => {
                            app.services_state.wizard = ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: None,
                            };
                        }
                        KeyCode::Char(c) => {
                            input_buffer.push(c);
                            app.services_state.wizard = ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(input_buffer),
                            };
                        }
                        KeyCode::Backspace => {
                            input_buffer.pop();
                            app.services_state.wizard = ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(input_buffer),
                            };
                        }
                        KeyCode::Enter => {
                            let folder_name = input_buffer.trim().to_string();
                            if !folder_name.is_empty() {
                                let is_local = remote.is_empty();
                                let parent_fs = if is_local {
                                    current_path.clone()
                                } else {
                                    format!("{}{}", remote, current_path)
                                };
                                let fs_target = if parent_fs.ends_with('/') {
                                    format!("{}{}", parent_fs, folder_name)
                                } else {
                                    format!("{}/{}", parent_fs, folder_name)
                                };

                                let param = if is_local {
                                    serde_json::json!({
                                        "fs": fs_target,
                                        "remote": "",
                                    })
                                } else {
                                    serde_json::json!({
                                        "fs": parent_fs,
                                        "remote": folder_name,
                                    })
                                }
                                .to_string();

                                let tx_clone = tx.clone();
                                tokio::spawn(async move {
                                    if is_local {
                                        if std::fs::create_dir_all(&fs_target).is_err() {
                                            let _ = std::process::Command::new("pkexec")
                                                .args(&["mkdir", "-p", &fs_target])
                                                .status();
                                        }
                                    } else {
                                        let _ = rclone::rpc_async("operations/mkdir".to_string(), param).await;
                                    }
                                    let _ = tx_clone.send(AppEvent::WizardGuiRefresh);
                                });
                            }
                            app.services_state.wizard = ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading: true,
                                error_msg,
                                creating_folder: None,
                            };
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Esc => {
                            app.services_state.wizard = ServicesWizardState::None;
                        }
                        KeyCode::Up => {
                            if !items.is_empty() {
                                if selected_idx == 0 {
                                    selected_idx = items.len() - 1;
                                } else {
                                    selected_idx -= 1;
                                }
                                app.services_state.wizard = ServicesWizardState::GuiSelectPath {
                                    service_type,
                                    remote,
                                    current_path,
                                    items,
                                    selected_idx,
                                    loading,
                                    error_msg,
                                    creating_folder: None,
                                };
                            }
                        }
                        KeyCode::Down => {
                            if !items.is_empty() {
                                selected_idx = (selected_idx + 1) % items.len();
                                app.services_state.wizard = ServicesWizardState::GuiSelectPath {
                                    service_type,
                                    remote,
                                    current_path,
                                    items,
                                    selected_idx,
                                    loading,
                                    error_msg,
                                    creating_folder: None,
                                };
                            }
                        }
                        KeyCode::Enter => {
                            if !items.is_empty() && selected_idx < items.len() {
                                let name = &items[selected_idx].name;
                                if name == ".." {
                                    if current_path != "/" && !current_path.is_empty() {
                                        let mut parts: Vec<&str> = current_path.split('/').filter(|s| !s.is_empty()).collect();
                                        if !parts.is_empty() {
                                            parts.pop();
                                        }
                                        current_path = if parts.is_empty() {
                                            "/".to_string()
                                        } else {
                                            format!("/{}", parts.join("/"))
                                        };
                                    }
                                } else {
                                    current_path = if current_path == "/" {
                                        format!("/{}", name)
                                    } else {
                                        format!("{}/{}", current_path, name)
                                    };
                                }
                                app.services_state.wizard = ServicesWizardState::GuiSelectPath {
                                    service_type,
                                    remote,
                                    current_path,
                                    items: Vec::new(),
                                    selected_idx: 0,
                                    loading: true,
                                    error_msg: None,
                                    creating_folder: None,
                                };
                                app.refresh_wizard_gui_list(tx.clone()).await;
                            }
                        }
                        KeyCode::Insert => {
                            let clean_path = if remote.is_empty() {
                                current_path.clone()
                            } else if current_path == "/" {
                                "/".to_string()
                            } else {
                                current_path.trim_start_matches('/').to_string()
                            };

                            if service_type == ServiceType::Mount || service_type == ServiceType::NfsMount {
                                app.services_state.wizard = ServicesWizardState::GuiSelectLocalPath {
                                    service_type,
                                    remote: remote.clone(),
                                    remote_path: clean_path,
                                    current_path: "/".to_string(),
                                    items: Vec::new(),
                                    selected_idx: 0,
                                    loading: true,
                                    error_msg: None,
                                    creating_folder: None,
                                };
                                app.refresh_wizard_gui_list(tx.clone()).await;
                            } else {
                                let flags = vec![
                                    ("mount_point".to_string(), "Nhập thư mục mount cục bộ (Mặc định: /mnt/drive)".to_string(), "/mnt/drive".to_string(), String::new()),
                                ];
                                app.services_state.wizard = ServicesWizardState::AskFlags {
                                    service_type,
                                    remote,
                                    path: clean_path,
                                    protocol: None,
                                    flags,
                                    current_flag_idx: 0,
                                    input_buffer: String::new(),
                                    is_simple_terminal: true,
                                    is_editing: false,
                                };
                            }
                        }
                        KeyCode::F(7) => {
                            app.services_state.wizard = ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        KeyCode::Char('n') if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) => {
                            app.services_state.wizard = ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        KeyCode::Char('N') if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) => {
                            app.services_state.wizard = ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        KeyCode::Char('N') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                            app.services_state.wizard = ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        KeyCode::Char('n') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT) => {
                            app.services_state.wizard = ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        _ => {}
                    }
                }
            }
            ServicesWizardState::GuiSelectLocalPath {
                service_type,
                remote,
                remote_path,
                mut current_path,
                items,
                mut selected_idx,
                loading,
                error_msg,
                creating_folder,
            } => {
                let tx_clone = tx.clone();
                if let Some(mut buf) = creating_folder {
                    match key.code {
                        KeyCode::Esc => {
                            app.services_state.wizard = ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: None,
                            };
                        }
                        KeyCode::Char(c) => {
                            buf.push(c);
                            app.services_state.wizard = ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(buf),
                            };
                        }
                        KeyCode::Backspace => {
                            buf.pop();
                            app.services_state.wizard = ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(buf),
                            };
                        }
                        KeyCode::Enter => {
                            if !buf.trim().is_empty() {
                                let folder_name = buf.trim().to_string();
                                let fs_target = if current_path == "/" {
                                    format!("/{}", folder_name)
                                } else {
                                    format!("{}/{}", current_path, folder_name)
                                };
                                tokio::spawn(async move {
                                    if std::fs::create_dir_all(&fs_target).is_err() {
                                        let _ = std::process::Command::new("pkexec")
                                            .args(&["mkdir", "-p", &fs_target])
                                            .status();
                                    }
                                    let _ = tx_clone.send(AppEvent::WizardGuiRefresh);
                                });
                            }
                            app.services_state.wizard = ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading: true,
                                error_msg,
                                creating_folder: None,
                            };
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Esc => {
                            app.services_state.wizard = ServicesWizardState::None;
                        }
                        KeyCode::Up => {
                            if !items.is_empty() {
                                if selected_idx == 0 {
                                    selected_idx = items.len() - 1;
                                } else {
                                    selected_idx -= 1;
                                }
                                app.services_state.wizard = ServicesWizardState::GuiSelectLocalPath {
                                    service_type,
                                    remote,
                                    remote_path,
                                    current_path,
                                    items,
                                    selected_idx,
                                    loading,
                                    error_msg,
                                    creating_folder: None,
                                };
                            }
                        }
                        KeyCode::Down => {
                            if !items.is_empty() {
                                selected_idx = (selected_idx + 1) % items.len();
                                app.services_state.wizard = ServicesWizardState::GuiSelectLocalPath {
                                    service_type,
                                    remote,
                                    remote_path,
                                    current_path,
                                    items,
                                    selected_idx,
                                    loading,
                                    error_msg,
                                    creating_folder: None,
                                };
                            }
                        }
                        KeyCode::Enter => {
                            if !items.is_empty() && selected_idx < items.len() {
                                let name = &items[selected_idx].name;
                                if name == ".." {
                                    if current_path != "/" && !current_path.is_empty() {
                                        let mut parts: Vec<&str> = current_path.split('/').filter(|s| !s.is_empty()).collect();
                                        if !parts.is_empty() {
                                            parts.pop();
                                        }
                                        current_path = if parts.is_empty() {
                                            "/".to_string()
                                        } else {
                                            format!("/{}", parts.join("/"))
                                        };
                                    }
                                } else {
                                    current_path = if current_path == "/" {
                                        format!("/{}", name)
                                    } else {
                                        format!("{}/{}", current_path, name)
                                    };
                                }
                                app.services_state.wizard = ServicesWizardState::GuiSelectLocalPath {
                                    service_type,
                                    remote,
                                    remote_path,
                                    current_path,
                                    items: Vec::new(),
                                    selected_idx: 0,
                                    loading: true,
                                    error_msg: None,
                                    creating_folder: None,
                                };
                                app.refresh_wizard_gui_list(tx.clone()).await;
                            }
                        }
                        KeyCode::Insert => {
                            let local_mnt = current_path.clone();
                            let flags = vec![
                                ("mount_point".to_string(), String::new(), String::new(), local_mnt),
                            ];
                            app.execute_launch_service(
                                service_type,
                                remote,
                                remote_path,
                                None,
                                flags,
                            );
                        }
                        KeyCode::F(7) => {
                            app.services_state.wizard = ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        KeyCode::Char('n') if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) => {
                            app.services_state.wizard = ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        KeyCode::Char('N') if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) => {
                            app.services_state.wizard = ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        KeyCode::Char('N') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                            app.services_state.wizard = ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        KeyCode::Char('n') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT) => {
                            app.services_state.wizard = ServicesWizardState::GuiSelectLocalPath {
                                service_type,
                                remote,
                                remote_path,
                                current_path,
                                items,
                                selected_idx,
                                loading,
                                error_msg,
                                creating_folder: Some(String::new()),
                            };
                        }
                        _ => {}
                    }
                }
            }
            ServicesWizardState::SelectProtocol {
                remote,
                path,
                mut selected_idx,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        app.services_state.wizard = ServicesWizardState::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = 3;
                        } else {
                            selected_idx -= 1;
                        }
                        app.services_state.wizard =
                            ServicesWizardState::SelectProtocol {
                                remote,
                                path,
                                selected_idx,
                            };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % 4;
                        app.services_state.wizard =
                            ServicesWizardState::SelectProtocol {
                                remote,
                                path,
                                selected_idx,
                            };
                    }
                    KeyCode::Enter => {
                        let proto = match selected_idx {
                            0 => "http",
                            1 => "ftp",
                            2 => "webdav",
                            3 => "sftp",
                            _ => "http",
                        }
                        .to_string();

                        // Chuyển sang hỏi cờ cho Serve
                        app.services_state.wizard = ServicesWizardState::AskFlags {
                            service_type: ServiceType::Serve,
                            remote,
                            path,
                            protocol: Some(proto),
                            flags: vec![
                                (
                                    "--addr".to_string(),
                                    "Địa chỉ IP và Cổng gán (Ví dụ :8080 hoặc 127.0.0.1:8080 - Mặc định: :8080)".to_string(),
                                    ":8080".to_string(),
                                    String::new(),
                                ),
                                (
                                    "--user".to_string(),
                                    "Tên đăng nhập bảo mật (Để trống nếu không dùng)".to_string(),
                                    "".to_string(),
                                    String::new(),
                                ),
                                (
                                    "--pass".to_string(),
                                    "Mật khẩu bảo mật (Để trống nếu không dùng)".to_string(),
                                    "".to_string(),
                                    String::new(),
                                ),
                                (
                                    "--read-only".to_string(),
                                    "Chỉ cho phép tải xuống (Read Only)? (y/N)".to_string(),
                                    "n".to_string(),
                                    String::new(),
                                ),
                            ],
                            current_flag_idx: 0,
                            input_buffer: String::new(),
                            is_simple_terminal: false,
                            is_editing: false,
                        };
                    }
                    _ => {}
                }
            }
            ServicesWizardState::AskFlags {
                service_type,
                remote,
                path,
                protocol,
                mut flags,
                mut current_flag_idx,
                mut input_buffer,
                is_simple_terminal,
                is_editing,
            } => {
                let total_options = flags.len() + 2;

                if is_editing {
                    let mut cursor = app.services_state.edit_cursor_idx;
                    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                        app.services_state.edit_cursor_idx = cursor;
                        app.services_state.wizard = ServicesWizardState::AskFlags {
                            service_type,
                            remote,
                            path,
                            protocol,
                            flags,
                            current_flag_idx,
                            input_buffer,
                            is_simple_terminal,
                            is_editing: true,
                        };
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                app.services_state.wizard = ServicesWizardState::AskFlags {
                                    service_type,
                                    remote,
                                    path,
                                    protocol,
                                    flags,
                                    current_flag_idx,
                                    input_buffer: String::new(),
                                    is_simple_terminal,
                                    is_editing: false,
                                };
                            }
                            KeyCode::Enter => {
                                flags[current_flag_idx].3 = input_buffer.trim().to_string();
                                app.services_state.wizard = ServicesWizardState::AskFlags {
                                    service_type,
                                    remote,
                                    path,
                                    protocol,
                                    flags,
                                    current_flag_idx,
                                    input_buffer: String::new(),
                                    is_simple_terminal,
                                    is_editing: false,
                                };
                            }
                            _ => {}
                        }
                    }
                    return;
                }

                match key.code {
                    KeyCode::Esc => {
                        app.services_state.wizard = ServicesWizardState::None;
                    }
                    KeyCode::Up => {
                        if current_flag_idx == 0 {
                            current_flag_idx = total_options - 1;
                        } else {
                            current_flag_idx -= 1;
                        }
                        app.services_state.wizard = ServicesWizardState::AskFlags {
                            service_type,
                            remote,
                            path,
                            protocol,
                            flags,
                            current_flag_idx,
                            input_buffer: String::new(),
                            is_simple_terminal,
                            is_editing: false,
                        };
                    }
                    KeyCode::Down => {
                        current_flag_idx = (current_flag_idx + 1) % total_options;
                        app.services_state.wizard = ServicesWizardState::AskFlags {
                            service_type,
                            remote,
                            path,
                            protocol,
                            flags,
                            current_flag_idx,
                            input_buffer: String::new(),
                            is_simple_terminal,
                            is_editing: false,
                        };
                    }
                    KeyCode::Enter => {
                        if current_flag_idx < flags.len() {
                            let val = flags[current_flag_idx].3.clone();
                            app.services_state.edit_cursor_idx = val.chars().count();
                            app.services_state.wizard = ServicesWizardState::AskFlags {
                                service_type,
                                remote,
                                path,
                                protocol,
                                flags,
                                current_flag_idx,
                                input_buffer: val,
                                is_simple_terminal,
                                is_editing: true,
                            };
                        } else if current_flag_idx == flags.len() {
                            for item in flags.iter_mut() {
                                if item.3.is_empty() {
                                    if item.0 == "mount_point" || item.0 == "--addr" || item.0 == "--rc-addr" {
                                        item.3 = item.2.clone();
                                    }
                                }
                            }
                            app.execute_launch_service(
                                service_type,
                                remote,
                                path,
                                protocol,
                                flags,
                            );
                        } else {
                            app.services_state.wizard = ServicesWizardState::None;
                        }
                    }
                    _ => {}
                }
            }
            ServicesWizardState::SelectSystemdAction {
                service_name,
                file_path,
                is_user,
                mut selected_idx,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        app.services_state.wizard = ServicesWizardState::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = 5;
                        } else {
                            selected_idx -= 1;
                        }
                        app.services_state.wizard = ServicesWizardState::SelectSystemdAction {
                            service_name,
                            file_path,
                            is_user,
                            selected_idx,
                        };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % 6;
                        app.services_state.wizard = ServicesWizardState::SelectSystemdAction {
                            service_name,
                            file_path,
                            is_user,
                            selected_idx,
                        };
                    }
                    KeyCode::Enter => {
                        app.services_state.wizard = ServicesWizardState::None;
                        let cmd_res = match selected_idx {
                            0 => {
                                app.ensure_mount_point_exists_from_service_file(&file_path);
                                if is_user {
                                    Command::new("systemctl").args(["--user", "start", &service_name]).status()
                                } else {
                                    Command::new("pkexec").args(["systemctl", "start", &service_name]).status()
                                }
                            }
                            1 => {
                                if is_user {
                                    Command::new("systemctl").args(["--user", "stop", &service_name]).status()
                                } else {
                                    Command::new("pkexec").args(["systemctl", "stop", &service_name]).status()
                                }
                            }
                            2 => {
                                app.ensure_mount_point_exists_from_service_file(&file_path);
                                if is_user {
                                    Command::new("systemctl").args(["--user", "restart", &service_name]).status()
                                } else {
                                    Command::new("pkexec").args(["systemctl", "restart", &service_name]).status()
                                }
                            }
                            3 => {
                                if is_user {
                                    Command::new("systemctl").args(["--user", "enable", &service_name]).status()
                                } else {
                                    Command::new("pkexec").args(["systemctl", "enable", &service_name]).status()
                                }
                            }
                            4 => {
                                if is_user {
                                    Command::new("systemctl").args(["--user", "disable", &service_name]).status()
                                } else {
                                    Command::new("pkexec").args(["systemctl", "disable", &service_name]).status()
                                }
                            }
                            5 => {
                                match app.load_systemd_service_fields(&file_path, is_user) {
                                    Ok(fields) => {
                                        app.services_state.wizard = ServicesWizardState::EditSystemdService {
                                            service_name: service_name.clone(),
                                            file_path: file_path.clone(),
                                            is_user,
                                            fields,
                                            selected_idx: 0,
                                            scroll_offset: 0,
                                            is_editing: false,
                                            input_buffer: String::new(),
                                            active_tab: 0,
                                            adding_new_key: false,
                                            new_key_buffer: String::new(),
                                        };
                                        return;
                                    }
                                    Err(e) => {
                                        app.services_state.error_message = Some(format!(
                                            "Lỗi đọc file cấu hình: {}", e
                                        ));
                                        return;
                                    }
                                }
                            }
                            _ => return,
                        };
                        match cmd_res {
                            Ok(status) if status.success() => {
                                app.services_state.info_message = Some("Thực thi lệnh systemd thành công!".to_string());
                            }
                            Ok(status) => {
                                 let is_eng = translate("srv_error_title").contains("SERVICE");
                                let mut err_msg = if is_eng {
                                    format!("Error executing systemd command: {}", status)
                                } else {
                                    format!("Lỗi thực thi lệnh systemd: {}", status)
                                };
                                if selected_idx == 0 || selected_idx == 2 {
                                    let logs = app.get_systemd_error_logs(&service_name, is_user);
                                    if !logs.is_empty() {
                                        let detail_hdr = if is_eng {
                                            "\n\nError details from system journal:\n"
                                        } else {
                                            "\n\nChi tiết lỗi từ nhật ký hệ thống:\n"
                                        };
                                        err_msg.push_str(&format!("{}{}", detail_hdr, logs));
                                    }
                                }
                                app.services_state.error_message = Some(err_msg);
                            }
                            Err(e) => {
                                app.services_state.error_message = Some(format!("Không thể chạy systemctl: {}", e));
                            }
                        }
                        app.scan_systemd_services();
                    }
                    _ => {}
                }
            }
            ServicesWizardState::EditSystemdService {
                service_name,
                file_path,
                is_user,
                mut fields,
                mut selected_idx,
                mut scroll_offset,
                is_editing,
                mut input_buffer,
                mut active_tab,
                adding_new_key,
                mut new_key_buffer,
            } => {
                let filtered_fields: Vec<&(String, String, String, Vec<String>)> = fields
                    .iter()
                    .filter(|(name, _, _, _)| {
                        if active_tab == 0 {
                            name.starts_with('_')
                        } else {
                            !name.starts_with('_')
                        }
                    })
                    .collect();
                let total_fields_count = filtered_fields.len();
                let total_options = total_fields_count + 2;

                if adding_new_key {
                    match key.code {
                        KeyCode::Esc => {
                            app.services_state.wizard = ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key: false, new_key_buffer: String::new()
                            };
                        }
                        KeyCode::Char(c) => {
                            new_key_buffer.push(c);
                            app.services_state.wizard = ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key: true, new_key_buffer
                            };
                        }
                        KeyCode::Backspace => {
                            new_key_buffer.pop();
                            app.services_state.wizard = ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key: true, new_key_buffer
                            };
                        }
                        KeyCode::Enter => {
                            let trimmed = new_key_buffer.trim().to_string();
                            if !trimmed.is_empty() {
                                if !fields.iter().any(|(k, _, _, _)| k == &trimmed) {
                                    fields.push((trimmed.clone(), String::new(), String::new(), Vec::new()));
                                }
                                let new_filtered: Vec<&(String, String, String, Vec<String>)> = fields
                                    .iter()
                                    .filter(|(name, _, _, _)| !name.starts_with('_'))
                                    .collect();
                                let new_sel = new_filtered.iter().position(|(k, _, _, _)| *k == trimmed).unwrap_or(0);
                                app.services_state.wizard = ServicesWizardState::EditSystemdService {
                                    service_name, file_path, is_user, fields, selected_idx: new_sel, scroll_offset: 0, is_editing: false, input_buffer: String::new(), active_tab: 1, adding_new_key: false, new_key_buffer: String::new()
                                };
                            }
                        }
                        _ => {}
                    }
                    return;
                }

                if is_editing {
                    let is_remote_field = filtered_fields.get(selected_idx).map(|f| f.0.as_str()) == Some("_remote");
                    if is_remote_field && (key.code == KeyCode::Up || key.code == KeyCode::Down) {
                        let remote_list = &app.services_state.all_remotes;
                        if !remote_list.is_empty() {
                            let (prefix, suffix) = if let Some(pos) = input_buffer.find(':') {
                                (input_buffer[..pos].to_string(), input_buffer[pos..].to_string())
                            } else {
                                (input_buffer.clone(), ":".to_string())
                            };
                            let current_idx = remote_list.iter().position(|r| r == &prefix);
                            let next_idx = match current_idx {
                                Some(idx) => {
                                    if key.code == KeyCode::Up {
                                        if idx == 0 {
                                            remote_list.len() - 1
                                        } else {
                                            idx - 1
                                        }
                                    } else {
                                        (idx + 1) % remote_list.len()
                                    }
                                }
                                None => 0,
                            };
                            input_buffer = format!("{}{}", remote_list[next_idx], suffix);
                        }
                        app.services_state.wizard = ServicesWizardState::EditSystemdService {
                            service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key, new_key_buffer
                        };
                    } else {
                        let mut cursor = app.services_state.edit_cursor_idx;
                        if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                            app.services_state.edit_cursor_idx = cursor;
                            app.services_state.wizard = ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: true, input_buffer, active_tab, adding_new_key, new_key_buffer
                            };
                        } else {
                            match key.code {
                                KeyCode::Esc => {
                                    app.services_state.wizard = ServicesWizardState::EditSystemdService {
                                        service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                    };
                                }
                                KeyCode::Enter => {
                                    let field_to_update = filtered_fields[selected_idx].0.clone();
                                    if let Some(item) = fields.iter_mut().find(|(k, _, _, _)| k == &field_to_update) {
                                        item.2 = input_buffer.clone();
                                    }
                                    app.services_state.wizard = ServicesWizardState::EditSystemdService {
                                        service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                    };
                                }
                                _ => {}
                            }
                        }
                    }
                    return;
                }

                match key.code {
                    KeyCode::Esc => {
                        app.services_state.wizard = ServicesWizardState::None;
                    }
                    KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
                        active_tab = if active_tab == 0 { 1 } else { 0 };
                        app.services_state.wizard = ServicesWizardState::EditSystemdService {
                            service_name, file_path, is_user, fields, selected_idx: 0, scroll_offset: 0, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                        };
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = total_options - 1;
                        } else {
                            selected_idx -= 1;
                        }
                        scroll_offset = ui_helpers::calculate_scroll_range(selected_idx, total_options, 15).start;
                        app.services_state.wizard = ServicesWizardState::EditSystemdService {
                            service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                        };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % total_options;
                        scroll_offset = ui_helpers::calculate_scroll_range(selected_idx, total_options, 15).start;
                        app.services_state.wizard = ServicesWizardState::EditSystemdService {
                            service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                        };
                    }
                    KeyCode::Enter => {
                        if selected_idx < total_fields_count {
                            let (field_to_update, choices, val) = {
                                let f = filtered_fields[selected_idx];
                                (f.0.clone(), f.3.clone(), f.2.clone())
                            };
                            if !choices.is_empty() && field_to_update != "_remote" {
                                let pos = choices.iter().position(|c| c == &val).unwrap_or(0);
                                let next_pos = (pos + 1) % choices.len();
                                let next_val = choices[next_pos].clone();
                                if let Some(item) = fields.iter_mut().find(|(k, _, _, _)| k == &field_to_update) {
                                    item.2 = next_val;
                                }
                                app.services_state.wizard = ServicesWizardState::EditSystemdService {
                                    service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                };
                            } else {
                                app.services_state.edit_cursor_idx = val.chars().count();
                                app.services_state.wizard = ServicesWizardState::EditSystemdService {
                                    service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: true, input_buffer: val, active_tab, adding_new_key, new_key_buffer
                                };
                            }
                        } else if selected_idx == total_fields_count {
                            // Lưu
                            match app.save_systemd_service_file(false, &service_name, &file_path, is_user, &fields) {
                                Ok(_) => {
                                    app.services_state.info_message = Some(format!("Đã cập nhật dịch vụ '{}' thành công!", service_name));
                                    app.services_state.wizard = ServicesWizardState::None;
                                    app.scan_systemd_services();
                                }
                                Err(e) => {
                                    app.services_state.error_message = Some(format!("Lỗi khi lưu cấu hình dịch vụ: {}", e));
                                }
                            }
                        } else {
                            // Hủy
                            app.services_state.wizard = ServicesWizardState::None;
                        }
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        match app.save_systemd_service_file(false, &service_name, &file_path, is_user, &fields) {
                            Ok(_) => {
                                app.services_state.info_message = Some(format!("Đã cập nhật dịch vụ '{}' thành công!", service_name));
                                app.services_state.wizard = ServicesWizardState::None;
                                app.scan_systemd_services();
                            }
                            Err(e) => {
                                app.services_state.error_message = Some(format!("Lỗi khi lưu cấu hình dịch vụ: {}", e));
                            }
                        }
                    }
                    KeyCode::Insert => {
                        if active_tab == 1 {
                            app.services_state.wizard = ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key: true, new_key_buffer: String::new()
                            };
                        }
                    }
                    KeyCode::Delete | KeyCode::Backspace => {
                        if active_tab == 1 && selected_idx < total_fields_count {
                            let field_to_delete = filtered_fields[selected_idx].0.clone();
                            fields.retain(|(k, _, _, _)| k != &field_to_delete);
                            let new_sel = if selected_idx >= fields.len() && !fields.is_empty() {
                                fields.len() - 1
                            } else {
                                selected_idx
                            };
                            app.services_state.wizard = ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx: new_sel, scroll_offset: 0, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                            };
                        }
                    }
                    _ => {}
                }
            }
            ServicesWizardState::CreateSystemdService {
                mut fields,
                mut selected_idx,
                mut scroll_offset,
                is_editing,
                mut input_buffer,
                mut active_tab,
                adding_new_key,
                mut new_key_buffer,
            } => {
                let filtered_fields: Vec<&(String, String, String, Vec<String>)> = fields
                    .iter()
                    .filter(|(name, _, _, _)| {
                        if active_tab == 0 {
                            name.starts_with('_')
                        } else {
                            !name.starts_with('_')
                        }
                    })
                    .collect();
                let total_fields_count = filtered_fields.len();
                let total_options = total_fields_count + 2;

                if adding_new_key {
                    match key.code {
                        KeyCode::Esc => {
                            app.services_state.wizard = ServicesWizardState::CreateSystemdService {
                                fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key: false, new_key_buffer: String::new()
                            };
                        }
                        KeyCode::Char(c) => {
                            new_key_buffer.push(c);
                            app.services_state.wizard = ServicesWizardState::CreateSystemdService {
                                fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key: true, new_key_buffer
                            };
                        }
                        KeyCode::Backspace => {
                            new_key_buffer.pop();
                            app.services_state.wizard = ServicesWizardState::CreateSystemdService {
                                fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key: true, new_key_buffer
                            };
                        }
                        KeyCode::Enter => {
                            let trimmed = new_key_buffer.trim().to_string();
                            if !trimmed.is_empty() {
                                if !fields.iter().any(|(k, _, _, _)| k == &trimmed) {
                                    fields.push((trimmed.clone(), String::new(), String::new(), Vec::new()));
                                }
                                let new_filtered: Vec<&(String, String, String, Vec<String>)> = fields
                                    .iter()
                                    .filter(|(name, _, _, _)| !name.starts_with('_'))
                                    .collect();
                                let new_sel = new_filtered.iter().position(|(k, _, _, _)| *k == trimmed).unwrap_or(0);
                                app.services_state.wizard = ServicesWizardState::CreateSystemdService {
                                    fields, selected_idx: new_sel, scroll_offset: 0, is_editing: false, input_buffer: String::new(), active_tab: 1, adding_new_key: false, new_key_buffer: String::new()
                                };
                            }
                        }
                        _ => {}
                    }
                    return;
                }

                if is_editing {
                    let is_remote_field = filtered_fields.get(selected_idx).map(|f| f.0.as_str()) == Some("_remote");
                    if is_remote_field && (key.code == KeyCode::Up || key.code == KeyCode::Down) {
                        let remote_list = &app.services_state.all_remotes;
                        if !remote_list.is_empty() {
                            let (prefix, suffix) = if let Some(pos) = input_buffer.find(':') {
                                (input_buffer[..pos].to_string(), input_buffer[pos..].to_string())
                            } else {
                                (input_buffer.clone(), ":".to_string())
                            };
                            let current_idx = remote_list.iter().position(|r| r == &prefix);
                            let next_idx = match current_idx {
                                Some(idx) => {
                                    if key.code == KeyCode::Up {
                                        if idx == 0 {
                                            remote_list.len() - 1
                                        } else {
                                            idx - 1
                                        }
                                    } else {
                                        (idx + 1) % remote_list.len()
                                    }
                                }
                                None => 0,
                            };
                            input_buffer = format!("{}{}", remote_list[next_idx], suffix);
                        }
                        app.services_state.wizard = ServicesWizardState::CreateSystemdService {
                            fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key, new_key_buffer
                        };
                    } else {
                        let mut cursor = app.services_state.edit_cursor_idx;
                        if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                            app.services_state.edit_cursor_idx = cursor;
                            app.services_state.wizard = ServicesWizardState::CreateSystemdService {
                                fields, selected_idx, scroll_offset, is_editing: true, input_buffer, active_tab, adding_new_key, new_key_buffer
                            };
                        } else {
                            match key.code {
                                KeyCode::Esc => {
                                    app.services_state.wizard = ServicesWizardState::CreateSystemdService {
                                        fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                    };
                                }
                                KeyCode::Enter => {
                                    let field_to_update = filtered_fields[selected_idx].0.clone();
                                    if let Some(item) = fields.iter_mut().find(|(k, _, _, _)| k == &field_to_update) {
                                        item.2 = input_buffer.clone();
                                    }
                                    app.services_state.wizard = ServicesWizardState::CreateSystemdService {
                                        fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                    };
                                }
                                _ => {}
                            }
                        }
                    }
                    return;
                }

                match key.code {
                    KeyCode::Esc => {
                        app.services_state.wizard = ServicesWizardState::None;
                    }
                    KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
                        active_tab = if active_tab == 0 { 1 } else { 0 };
                        app.services_state.wizard = ServicesWizardState::CreateSystemdService {
                            fields, selected_idx: 0, scroll_offset: 0, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                        };
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = total_options - 1;
                        } else {
                            selected_idx -= 1;
                        }
                        scroll_offset = ui_helpers::calculate_scroll_range(selected_idx, total_options, 15).start;
                        app.services_state.wizard = ServicesWizardState::CreateSystemdService {
                            fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                        };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % total_options;
                        scroll_offset = ui_helpers::calculate_scroll_range(selected_idx, total_options, 15).start;
                        app.services_state.wizard = ServicesWizardState::CreateSystemdService {
                            fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                        };
                    }
                    KeyCode::Enter => {
                        if selected_idx < total_fields_count {
                            let (field_to_update, choices, val) = {
                                let f = filtered_fields[selected_idx];
                                (f.0.clone(), f.3.clone(), f.2.clone())
                            };
                            if !choices.is_empty() && field_to_update != "_remote" {
                                let pos = choices.iter().position(|c| c == &val).unwrap_or(0);
                                let next_pos = (pos + 1) % choices.len();
                                let next_val = choices[next_pos].clone();
                                if let Some(item) = fields.iter_mut().find(|(k, _, _, _)| k == &field_to_update) {
                                    item.2 = next_val;
                                }
                                app.services_state.wizard = ServicesWizardState::CreateSystemdService {
                                    fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                };
                            } else {
                                app.services_state.edit_cursor_idx = val.chars().count();
                                app.services_state.wizard = ServicesWizardState::CreateSystemdService {
                                    fields, selected_idx, scroll_offset, is_editing: true, input_buffer: val, active_tab, adding_new_key, new_key_buffer
                                };
                            }
                        } else if selected_idx == total_fields_count {
                            // Thực hiện tạo file dịch vụ mới
                            let get_val = |key: &str| {
                                fields.iter().find(|(k, _, _, _)| k == key).map(|(_, _, v, _)| v.clone()).unwrap_or_default()
                            };
                            let raw_svc_name = get_val("_service_name");
                            let service_name = if raw_svc_name.ends_with(".service") {
                                raw_svc_name
                            } else {
                                format!("{}.service", raw_svc_name)
                            };
                            let level = get_val("_service_level");
                            let is_user = level.contains("User");
                            let home_dir = get_home_dir();
                            let file_path = if is_user {
                                format!("{}/.config/systemd/user/{}", home_dir, service_name)
                            } else {
                                format!("/etc/systemd/system/{}", service_name)
                            };

                            match app.save_systemd_service_file(true, &service_name, &file_path, is_user, &fields) {
                                Ok(_) => {
                                    app.services_state.info_message = Some(format!("Đã tạo thành công dịch vụ systemd '{}'!", service_name));
                                    app.services_state.wizard = ServicesWizardState::None;
                                    app.scan_systemd_services();
                                }
                                Err(e) => {
                                    app.services_state.error_message = Some(format!("Lỗi khi lưu cấu hình dịch vụ: {}", e));
                                }
                            }
                        } else {
                            app.services_state.wizard = ServicesWizardState::None;
                        }
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let get_val = |key: &str| {
                            fields.iter().find(|(k, _, _, _)| k == key).map(|(_, _, v, _)| v.clone()).unwrap_or_default()
                        };
                        let raw_svc_name = get_val("_service_name");
                        let service_name = if raw_svc_name.ends_with(".service") {
                            raw_svc_name
                        } else {
                            format!("{}.service", raw_svc_name)
                        };
                        let level = get_val("_service_level");
                        let is_user = level.contains("User");
                        let home_dir = get_home_dir();
                        let file_path = if is_user {
                            format!("{}/.config/systemd/user/{}", home_dir, service_name)
                        } else {
                            format!("/etc/systemd/system/{}", service_name)
                        };

                        match app.save_systemd_service_file(true, &service_name, &file_path, is_user, &fields) {
                            Ok(_) => {
                                app.services_state.info_message = Some(format!("Đã tạo thành công dịch vụ systemd '{}'!", service_name));
                                app.services_state.wizard = ServicesWizardState::None;
                                app.scan_systemd_services();
                            }
                            Err(e) => {
                                app.services_state.error_message = Some(format!("Lỗi khi lưu cấu hình dịch vụ: {}", e));
                            }
                        }
                    }
                    KeyCode::Insert => {
                        if active_tab == 1 {
                            app.services_state.wizard = ServicesWizardState::CreateSystemdService {
                                fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key: true, new_key_buffer: String::new()
                            };
                        }
                    }
                    KeyCode::Delete | KeyCode::Backspace => {
                        if active_tab == 1 && selected_idx < total_fields_count {
                            let field_to_delete = filtered_fields[selected_idx].0.clone();
                            fields.retain(|(k, _, _, _)| k != &field_to_delete);
                            let new_sel = if selected_idx >= fields.len() && !fields.is_empty() {
                                fields.len() - 1
                            } else {
                                selected_idx
                            };
                            app.services_state.wizard = ServicesWizardState::CreateSystemdService {
                                fields, selected_idx: new_sel, scroll_offset: 0, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                            };
                        }
                    }
                    _ => {}
                }
            }
        }
    }


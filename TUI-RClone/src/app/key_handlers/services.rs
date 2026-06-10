use crate::rclone;
use crate::ui;
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
use std::process::Command;

use crate::app::{
    App, AppEvent, Screen, DeleteTarget, handle_input_key
};

impl App {

    pub(crate) async fn handle_services_key(
        &mut self,
        key: KeyEvent,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let wizard = self.services_state.wizard.clone();
        match wizard {
            ui::services::ServicesWizardState::None => {
                match key.code {
                    KeyCode::Esc => {
                        self.screen = Screen::MainMenu;
                    }
                    KeyCode::Tab => {
                        let limit = if cfg!(target_os = "windows") { 2 } else { 3 };
                        self.services_state.active_focus = (self.services_state.active_focus + 1) % limit;
                    }
                    KeyCode::Up => {
                        match self.services_state.active_focus {
                            0 => self.services_state.prev_menu(),
                            1 => self.services_state.prev_active(),
                            _ => self.services_state.prev_systemd(),
                        }
                    }
                    KeyCode::Down => {
                        match self.services_state.active_focus {
                            0 => self.services_state.next_menu(),
                            1 => self.services_state.next_active(),
                            _ => self.services_state.next_systemd(),
                        }
                    }
                    KeyCode::Enter => {
                        if self.services_state.active_focus == 0 {
                            // Khởi động wizard tạo dịch vụ mới
                            let service_type = match self.services_state.selected_menu_idx {
                                0 => ui::services::ServiceType::Mount,
                                1 => ui::services::ServiceType::NfsMount,
                                2 => ui::services::ServiceType::WebGui,
                                3 => ui::services::ServiceType::Serve,
                                _ => return,
                            };

                            // Web GUI không cần chọn Remote, chuyển thẳng sang hỏi flags
                            if service_type == ui::services::ServiceType::WebGui {
                                self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
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
                            } else if service_type == ui::services::ServiceType::Mount || service_type == ui::services::ServiceType::NfsMount {
                                // Chọn chế độ Simple hay Advanced
                                self.services_state.wizard =
                                    ui::services::ServicesWizardState::AskMode {
                                        service_type,
                                        selected_idx: 0,
                                    };
                            } else {
                                // Cần chọn Remote cho Serve
                                self.services_state.wizard =
                                    ui::services::ServicesWizardState::SelectRemote {
                                        service_type,
                                        remotes: self.connection_state.remotes.clone(),
                                        selected_idx: 0,
                                        is_simple_terminal: false,
                                        is_simple_gui: false,
                                    };
                            }
                        } else if self.services_state.active_focus == 2 {
                            if !self.services_state.systemd_services.is_empty() {
                                let idx = self.services_state.selected_systemd_idx;
                                let svc = &self.services_state.systemd_services[idx];
                                match self.load_systemd_service_fields(&svc.file_path, svc.is_user) {
                                    Ok(fields) => {
                                        self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
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
                                        self.services_state.error_message = Some(format!(
                                            "Lỗi đọc file cấu hình: {}", e
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char(' ') => {
                        if self.services_state.active_focus == 2 {
                            if !self.services_state.systemd_services.is_empty() {
                                let idx = self.services_state.selected_systemd_idx;
                                let svc = &self.services_state.systemd_services[idx];
                                self.services_state.wizard = ui::services::ServicesWizardState::SelectSystemdAction {
                                    service_name: svc.name.clone(),
                                    file_path: svc.file_path.clone(),
                                    is_user: svc.is_user,
                                    selected_idx: 0,
                                };
                            }
                        }
                    }
                    KeyCode::Insert => {
                        if self.services_state.active_focus == 2 {
                            let fields = self.init_create_systemd_service_fields();
                            self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
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
                        if self.services_state.active_focus == 1 {
                            if !self.services_state.active_services.is_empty() {
                                let idx = self.services_state.selected_active_idx;
                                self.delete_confirm = Some(DeleteTarget::Service(idx));
                            }
                        } else if self.services_state.active_focus == 2 {
                            if !self.services_state.systemd_services.is_empty() {
                                let idx = self.services_state.selected_systemd_idx;
                                self.delete_confirm = Some(DeleteTarget::SystemdService(idx));
                            }
                        }
                    }
                    _ => {}
                }
            }
            ui::services::ServicesWizardState::AskMode {
                service_type,
                mut selected_idx,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.services_state.wizard = ui::services::ServicesWizardState::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = 2;
                        } else {
                            selected_idx -= 1;
                        }
                        self.services_state.wizard =
                            ui::services::ServicesWizardState::AskMode {
                                service_type,
                                selected_idx,
                            };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % 3;
                        self.services_state.wizard =
                            ui::services::ServicesWizardState::AskMode {
                                service_type,
                                selected_idx,
                            };
                    }
                    KeyCode::Enter => {
                        let is_simple_terminal = selected_idx == 0;
                        let is_simple_gui = selected_idx == 1;

                        // Tìm remote Secret để preselect nếu có
                        let mut default_idx = 0;
                        for (i, r) in self.connection_state.remotes.iter().enumerate() {
                            if r.to_lowercase() == "secret" {
                                default_idx = i + 1;
                                break;
                            }
                        }

                        self.services_state.wizard =
                            ui::services::ServicesWizardState::SelectRemote {
                                service_type,
                                remotes: self.connection_state.remotes.clone(),
                                selected_idx: default_idx,
                                is_simple_terminal,
                                is_simple_gui,
                            };
                    }
                    _ => {}
                }
            }
            ui::services::ServicesWizardState::SelectRemote {
                service_type,
                remotes,
                mut selected_idx,
                is_simple_terminal,
                is_simple_gui,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.services_state.wizard = ui::services::ServicesWizardState::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = remotes.len();
                        } else {
                            selected_idx -= 1;
                        }
                        self.services_state.wizard =
                            ui::services::ServicesWizardState::SelectRemote {
                                service_type,
                                remotes,
                                selected_idx,
                                is_simple_terminal,
                                is_simple_gui,
                            };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % (remotes.len() + 1);
                        self.services_state.wizard =
                            ui::services::ServicesWizardState::SelectRemote {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote: remote.clone(),
                                current_path: "/".to_string(),
                                items: Vec::new(),
                                selected_idx: 0,
                                loading: true,
                                error_msg: None,
                                creating_folder: None,
                            };
                            self.refresh_wizard_gui_list(tx).await;
                        } else {
                            let default_path = if remote.to_lowercase().starts_with("secret:") {
                                "Khobaomat".to_string()
                            } else {
                                String::new()
                            };
                            self.services_state.edit_cursor_idx = default_path.chars().count();
                            self.services_state.wizard = ui::services::ServicesWizardState::InputPath {
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
            ui::services::ServicesWizardState::InputPath {
                service_type,
                remote,
                mut input_buffer,
                is_simple_terminal,
            } => {
                let mut cursor = self.services_state.edit_cursor_idx;
                if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                    self.services_state.edit_cursor_idx = cursor;
                    self.services_state.wizard = ui::services::ServicesWizardState::InputPath {
                        service_type,
                        remote,
                        input_buffer,
                        is_simple_terminal,
                    };
                } else {
                    match key.code {
                        KeyCode::Esc => {
                            self.services_state.wizard = ui::services::ServicesWizardState::None;
                        }
                        KeyCode::Enter => {
                        let path = input_buffer.trim().to_string();

                        if service_type == ui::services::ServiceType::Mount || service_type == ui::services::ServiceType::NfsMount {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
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
                            self.services_state.wizard =
                                ui::services::ServicesWizardState::SelectProtocol {
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
            ui::services::ServicesWizardState::GuiSelectPath {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
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
                        KeyCode::Backspace => {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                service_type,
                                remote,
                                current_path,
                                items: Vec::new(),
                                selected_idx: 0,
                                loading: true,
                                error_msg: None,
                                creating_folder: None,
                            };
                            self.refresh_wizard_gui_list(tx.clone()).await;
                        }
                        KeyCode::Esc => {
                            if let Some(return_tgt) = self.services_state.systemd_wizard_return.take() {
                                match return_tgt {
                                    ui::services::WizardReturnTarget::CreateSystemd {
                                        fields,
                                        selected_idx,
                                        scroll_offset,
                                        active_tab,
                                        ..
                                    } => {
                                        self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                            fields,
                                            selected_idx,
                                            scroll_offset,
                                            is_editing: false,
                                            input_buffer: String::new(),
                                            active_tab,
                                            adding_new_key: false,
                                            new_key_buffer: String::new(),
                                        };
                                    }
                                    ui::services::WizardReturnTarget::EditSystemd {
                                        service_name,
                                        file_path,
                                        is_user,
                                        fields,
                                        selected_idx,
                                        scroll_offset,
                                        active_tab,
                                        ..
                                    } => {
                                        self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                            service_name,
                                            file_path,
                                            is_user,
                                            fields,
                                            selected_idx,
                                            scroll_offset,
                                            is_editing: false,
                                            input_buffer: String::new(),
                                            active_tab,
                                            adding_new_key: false,
                                            new_key_buffer: String::new(),
                                        };
                                    }
                                }
                            } else {
                                self.services_state.wizard = ui::services::ServicesWizardState::None;
                            }
                        }
                        KeyCode::Up => {
                            if !items.is_empty() {
                                if selected_idx == 0 {
                                    selected_idx = items.len() - 1;
                                } else {
                                    selected_idx -= 1;
                                }
                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
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
                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
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
                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                    service_type,
                                    remote,
                                    current_path,
                                    items: Vec::new(),
                                    selected_idx: 0,
                                    loading: true,
                                    error_msg: None,
                                    creating_folder: None,
                                };
                                self.refresh_wizard_gui_list(tx.clone()).await;
                            }
                        }
                        KeyCode::Insert => {
                            let clean_path = if remote.is_empty() {
                                current_path.clone()
                            } else if current_path == "/" {
                                String::new()
                            } else {
                                current_path.trim_start_matches('/').to_string()
                            };

                            if let Some(return_tgt) = self.services_state.systemd_wizard_return.take() {
                                let val_to_set = if clean_path.is_empty() {
                                    remote.clone()
                                } else {
                                    format!("{}{}", remote, clean_path)
                                };
                                match return_tgt {
                                    ui::services::WizardReturnTarget::CreateSystemd {
                                        mut fields,
                                        selected_idx,
                                        scroll_offset,
                                        active_tab,
                                        target_field,
                                    } => {
                                        if let Some(field) = fields.iter_mut().find(|(k, _, _, _)| k == &target_field) {
                                            field.2 = val_to_set;
                                        }
                                        self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                            fields,
                                            selected_idx,
                                            scroll_offset,
                                            is_editing: false,
                                            input_buffer: String::new(),
                                            active_tab,
                                            adding_new_key: false,
                                            new_key_buffer: String::new(),
                                        };
                                    }
                                    ui::services::WizardReturnTarget::EditSystemd {
                                        service_name,
                                        file_path,
                                        is_user,
                                        mut fields,
                                        selected_idx,
                                        scroll_offset,
                                        active_tab,
                                        target_field,
                                    } => {
                                        if let Some(field) = fields.iter_mut().find(|(k, _, _, _)| k == &target_field) {
                                            field.2 = val_to_set;
                                        }
                                        self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                            service_name,
                                            file_path,
                                            is_user,
                                            fields,
                                            selected_idx,
                                            scroll_offset,
                                            is_editing: false,
                                            input_buffer: String::new(),
                                            active_tab,
                                            adding_new_key: false,
                                            new_key_buffer: String::new(),
                                        };
                                    }
                                }
                            } else {
                                let clean_path_fmt = if clean_path.is_empty() {
                                    "/".to_string()
                                } else {
                                    clean_path
                                };
                                if service_type == ui::services::ServiceType::Mount || service_type == ui::services::ServiceType::NfsMount {
                                    self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                        service_type,
                                        remote: remote.clone(),
                                        remote_path: clean_path_fmt,
                                        current_path: "/".to_string(),
                                        items: Vec::new(),
                                        selected_idx: 0,
                                        loading: true,
                                        error_msg: None,
                                        creating_folder: None,
                                    };
                                    self.refresh_wizard_gui_list(tx.clone()).await;
                                } else {
                                    let flags = vec![
                                        ("mount_point".to_string(), "Nhập thư mục mount cục bộ (Mặc định: /mnt/drive)".to_string(), "/mnt/drive".to_string(), String::new()),
                                    ];
                                    self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
                                        service_type,
                                        remote,
                                        path: clean_path_fmt,
                                        protocol: None,
                                        flags,
                                        current_flag_idx: 0,
                                        input_buffer: String::new(),
                                        is_simple_terminal: true,
                                        is_editing: false,
                                    };
                                }
                            }
                        }
                        KeyCode::F(7) => {
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
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
            ui::services::ServicesWizardState::GuiSelectLocalPath {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
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
                            if let Some(return_tgt) = self.services_state.systemd_wizard_return.take() {
                                match return_tgt {
                                    ui::services::WizardReturnTarget::CreateSystemd {
                                        fields,
                                        selected_idx,
                                        scroll_offset,
                                        active_tab,
                                        ..
                                    } => {
                                        self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                            fields,
                                            selected_idx,
                                            scroll_offset,
                                            is_editing: false,
                                            input_buffer: String::new(),
                                            active_tab,
                                            adding_new_key: false,
                                            new_key_buffer: String::new(),
                                        };
                                    }
                                    ui::services::WizardReturnTarget::EditSystemd {
                                        service_name,
                                        file_path,
                                        is_user,
                                        fields,
                                        selected_idx,
                                        scroll_offset,
                                        active_tab,
                                        ..
                                    } => {
                                        self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                            service_name,
                                            file_path,
                                            is_user,
                                            fields,
                                            selected_idx,
                                            scroll_offset,
                                            is_editing: false,
                                            input_buffer: String::new(),
                                            active_tab,
                                            adding_new_key: false,
                                            new_key_buffer: String::new(),
                                        };
                                    }
                                }
                            } else {
                                self.services_state.wizard = ui::services::ServicesWizardState::None;
                            }
                        }
                        KeyCode::Backspace => {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
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
                            self.refresh_wizard_gui_list(tx.clone()).await;
                        }
                        KeyCode::Up => {
                            if !items.is_empty() {
                                if selected_idx == 0 {
                                    selected_idx = items.len() - 1;
                                } else {
                                    selected_idx -= 1;
                                }
                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
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
                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
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
                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
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
                                self.refresh_wizard_gui_list(tx.clone()).await;
                            }
                        }
                        KeyCode::Insert => {
                            let local_mnt = current_path.clone();
                            if let Some(return_tgt) = self.services_state.systemd_wizard_return.take() {
                                match return_tgt {
                                    ui::services::WizardReturnTarget::CreateSystemd {
                                        mut fields,
                                        selected_idx,
                                        scroll_offset,
                                        active_tab,
                                        target_field,
                                    } => {
                                        if let Some(field) = fields.iter_mut().find(|(k, _, _, _)| k == &target_field) {
                                            field.2 = local_mnt;
                                        }
                                        self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                            fields,
                                            selected_idx,
                                            scroll_offset,
                                            is_editing: false,
                                            input_buffer: String::new(),
                                            active_tab,
                                            adding_new_key: false,
                                            new_key_buffer: String::new(),
                                        };
                                    }
                                    ui::services::WizardReturnTarget::EditSystemd {
                                        service_name,
                                        file_path,
                                        is_user,
                                        mut fields,
                                        selected_idx,
                                        scroll_offset,
                                        active_tab,
                                        target_field,
                                    } => {
                                        if let Some(field) = fields.iter_mut().find(|(k, _, _, _)| k == &target_field) {
                                            field.2 = local_mnt;
                                        }
                                        self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                            service_name,
                                            file_path,
                                            is_user,
                                            fields,
                                            selected_idx,
                                            scroll_offset,
                                            is_editing: false,
                                            input_buffer: String::new(),
                                            active_tab,
                                            adding_new_key: false,
                                            new_key_buffer: String::new(),
                                        };
                                    }
                                }
                            } else {
                                let flags = vec![
                                    ("mount_point".to_string(), String::new(), String::new(), local_mnt),
                                ];
                                self.execute_launch_service(
                                    service_type,
                                    remote,
                                    remote_path,
                                    None,
                                    flags,
                                );
                            }
                        }
                        KeyCode::F(7) => {
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
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
            ui::services::ServicesWizardState::SelectProtocol {
                remote,
                path,
                mut selected_idx,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.services_state.wizard = ui::services::ServicesWizardState::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = 3;
                        } else {
                            selected_idx -= 1;
                        }
                        self.services_state.wizard =
                            ui::services::ServicesWizardState::SelectProtocol {
                                remote,
                                path,
                                selected_idx,
                            };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % 4;
                        self.services_state.wizard =
                            ui::services::ServicesWizardState::SelectProtocol {
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
                        self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
                            service_type: ui::services::ServiceType::Serve,
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
            ui::services::ServicesWizardState::AskFlags {
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
                    let mut cursor = self.services_state.edit_cursor_idx;
                    if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                        self.services_state.edit_cursor_idx = cursor;
                        self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
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
                                self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
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
                                self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
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
                        self.services_state.wizard = ui::services::ServicesWizardState::None;
                    }
                    KeyCode::Up => {
                        if current_flag_idx == 0 {
                            current_flag_idx = total_options - 1;
                        } else {
                            current_flag_idx -= 1;
                        }
                        self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
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
                        self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
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
                            self.services_state.edit_cursor_idx = val.chars().count();
                            self.services_state.wizard = ui::services::ServicesWizardState::AskFlags {
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
                            self.execute_launch_service(
                                service_type,
                                remote,
                                path,
                                protocol,
                                flags,
                            );
                        } else {
                            self.services_state.wizard = ui::services::ServicesWizardState::None;
                        }
                    }
                    _ => {}
                }
            }
            ui::services::ServicesWizardState::SelectSystemdAction {
                service_name,
                file_path,
                is_user,
                mut selected_idx,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.services_state.wizard = ui::services::ServicesWizardState::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = 5;
                        } else {
                            selected_idx -= 1;
                        }
                        self.services_state.wizard = ui::services::ServicesWizardState::SelectSystemdAction {
                            service_name,
                            file_path,
                            is_user,
                            selected_idx,
                        };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % 6;
                        self.services_state.wizard = ui::services::ServicesWizardState::SelectSystemdAction {
                            service_name,
                            file_path,
                            is_user,
                            selected_idx,
                        };
                    }
                    KeyCode::Enter => {
                        self.services_state.wizard = ui::services::ServicesWizardState::None;
                        let cmd_res = match selected_idx {
                            0 => {
                                self.ensure_mount_point_exists_from_service_file(&file_path);
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
                                self.ensure_mount_point_exists_from_service_file(&file_path);
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
                                match self.load_systemd_service_fields(&file_path, is_user) {
                                    Ok(fields) => {
                                        self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
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
                                        self.services_state.error_message = Some(format!(
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
                                self.services_state.info_message = Some("Thực thi lệnh systemd thành công!".to_string());
                            }
                            Ok(status) => {
                                let is_eng = crate::lang::translate("srv_error_title").contains("SERVICE");
                                let mut err_msg = if is_eng {
                                    format!("Error executing systemd command: {}", status)
                                } else {
                                    format!("Lỗi thực thi lệnh systemd: {}", status)
                                };
                                if selected_idx == 0 || selected_idx == 2 {
                                    let logs = self.get_systemd_error_logs(&service_name, is_user);
                                    if !logs.is_empty() {
                                        let detail_hdr = if is_eng {
                                            "\n\nError details from system journal:\n"
                                        } else {
                                            "\n\nChi tiết lỗi từ nhật ký hệ thống:\n"
                                        };
                                        err_msg.push_str(&format!("{}{}", detail_hdr, logs));
                                    }
                                }
                                self.services_state.error_message = Some(err_msg);
                            }
                            Err(e) => {
                                self.services_state.error_message = Some(format!("Không thể chạy systemctl: {}", e));
                            }
                        }
                        self.scan_systemd_services();
                    }
                    _ => {}
                }
            }
            ui::services::ServicesWizardState::EditSystemdService {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key: false, new_key_buffer: String::new()
                            };
                        }
                        KeyCode::Char(c) => {
                            new_key_buffer.push(c);
                            self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key: true, new_key_buffer
                            };
                        }
                        KeyCode::Backspace => {
                            new_key_buffer.pop();
                            self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
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
                                self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
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
                        let remote_list = &self.services_state.all_remotes;
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
                        self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                            service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key, new_key_buffer
                        };
                    } else {
                        let mut cursor = self.services_state.edit_cursor_idx;
                        if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                            self.services_state.edit_cursor_idx = cursor;
                            self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: true, input_buffer, active_tab, adding_new_key, new_key_buffer
                            };
                        } else {
                            match key.code {
                                KeyCode::Esc => {
                                    self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                        service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                    };
                                }
                                KeyCode::Enter => {
                                    let field_to_update = filtered_fields[selected_idx].0.clone();
                                    if let Some(item) = fields.iter_mut().find(|(k, _, _, _)| k == &field_to_update) {
                                        item.2 = input_buffer.clone();
                                    }
                                    self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                        service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                    };
                                }
                                KeyCode::Insert => {
                                    if active_tab == 0 && selected_idx < filtered_fields.len() {
                                        let field_name = filtered_fields[selected_idx].0.as_str();
                                        if field_name == "_remote" || field_name == "_mount_path" {
                                            let mut fields_updated = fields.clone();
                                            if let Some(item) = fields_updated.iter_mut().find(|(k, _, _, _)| k == field_name) {
                                                item.2 = input_buffer.clone();
                                            }
                                            self.services_state.systemd_wizard_return = Some(ui::services::WizardReturnTarget::EditSystemd {
                                                service_name: service_name.clone(),
                                                file_path: file_path.clone(),
                                                is_user,
                                                fields: fields_updated,
                                                selected_idx,
                                                scroll_offset,
                                                active_tab,
                                                target_field: field_name.to_string(),
                                            });

                                            if field_name == "_remote" {
                                                let current_val = input_buffer.clone();
                                                let (remote_part, path_part) = if let Some(pos) = current_val.find(':') {
                                                    (current_val[..pos].to_string(), current_val[pos+1..].to_string())
                                                } else {
                                                    (current_val.clone(), String::new())
                                                };
                                                let current_path = if path_part.is_empty() {
                                                    "/".to_string()
                                                } else if path_part.starts_with('/') {
                                                    path_part
                                                } else {
                                                    format!("/{}", path_part)
                                                };

                                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                                    service_type: ui::services::ServiceType::Mount,
                                                    remote: if remote_part.is_empty() { String::new() } else { format!("{}:", remote_part) },
                                                    current_path,
                                                    items: Vec::new(),
                                                    selected_idx: 0,
                                                    loading: true,
                                                    error_msg: None,
                                                    creating_folder: None,
                                                };
                                                self.refresh_wizard_gui_list(tx.clone()).await;
                                            } else {
                                                let current_val = input_buffer.clone();
                                                let current_path = if current_val.is_empty() {
                                                    "/".to_string()
                                                } else {
                                                    current_val
                                                };

                                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                                    service_type: ui::services::ServiceType::Mount,
                                                    remote: String::new(),
                                                    remote_path: String::new(),
                                                    current_path,
                                                    items: Vec::new(),
                                                    selected_idx: 0,
                                                    loading: true,
                                                    error_msg: None,
                                                    creating_folder: None,
                                                };
                                                self.refresh_wizard_gui_list(tx.clone()).await;
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    return;
                }

                match key.code {
                    KeyCode::Esc => {
                        self.services_state.wizard = ui::services::ServicesWizardState::None;
                    }
                    KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
                        active_tab = if active_tab == 0 { 1 } else { 0 };
                        self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                            service_name, file_path, is_user, fields, selected_idx: 0, scroll_offset: 0, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                        };
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = total_options - 1;
                        } else {
                            selected_idx -= 1;
                        }
                        scroll_offset = ui::calculate_scroll_range(selected_idx, total_options, 15).start;
                        self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                            service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                        };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % total_options;
                        scroll_offset = ui::calculate_scroll_range(selected_idx, total_options, 15).start;
                        self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
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
                                self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                    service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                };
                            } else {
                                self.services_state.edit_cursor_idx = val.chars().count();
                                self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                    service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: true, input_buffer: val, active_tab, adding_new_key, new_key_buffer
                                };
                            }
                        } else if selected_idx == total_fields_count {
                            // Lưu
                            match self.save_systemd_service_file(false, &service_name, &file_path, is_user, &fields) {
                                Ok(_) => {
                                    self.services_state.info_message = Some(format!("Đã cập nhật dịch vụ '{}' thành công!", service_name));
                                    self.services_state.wizard = ui::services::ServicesWizardState::None;
                                    self.scan_systemd_services();
                                }
                                Err(e) => {
                                    self.services_state.error_message = Some(format!("Lỗi khi lưu cấu hình dịch vụ: {}", e));
                                }
                            }
                        } else {
                            // Hủy
                            self.services_state.wizard = ui::services::ServicesWizardState::None;
                        }
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        match self.save_systemd_service_file(false, &service_name, &file_path, is_user, &fields) {
                            Ok(_) => {
                                self.services_state.info_message = Some(format!("Đã cập nhật dịch vụ '{}' thành công!", service_name));
                                self.services_state.wizard = ui::services::ServicesWizardState::None;
                                self.scan_systemd_services();
                            }
                            Err(e) => {
                                self.services_state.error_message = Some(format!("Lỗi khi lưu cấu hình dịch vụ: {}", e));
                            }
                        }
                    }
                    KeyCode::Insert => {
                        if active_tab == 1 {
                            self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key: true, new_key_buffer: String::new()
                            };
                        } else if active_tab == 0 && selected_idx < filtered_fields.len() {
                            let field_name = filtered_fields[selected_idx].0.as_str();
                            if field_name == "_remote" || field_name == "_mount_path" {
                                self.services_state.systemd_wizard_return = Some(ui::services::WizardReturnTarget::EditSystemd {
                                    service_name: service_name.clone(),
                                    file_path: file_path.clone(),
                                    is_user,
                                    fields: fields.clone(),
                                    selected_idx,
                                    scroll_offset,
                                    active_tab,
                                    target_field: field_name.to_string(),
                                });

                                if field_name == "_remote" {
                                    let current_val = filtered_fields[selected_idx].2.clone();
                                    let (remote_part, path_part) = if let Some(pos) = current_val.find(':') {
                                        (current_val[..pos].to_string(), current_val[pos+1..].to_string())
                                    } else {
                                        (current_val.clone(), String::new())
                                    };
                                    let current_path = if path_part.is_empty() {
                                        "/".to_string()
                                    } else if path_part.starts_with('/') {
                                        path_part
                                    } else {
                                        format!("/{}", path_part)
                                    };

                                    self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                        service_type: ui::services::ServiceType::Mount,
                                        remote: if remote_part.is_empty() { String::new() } else { format!("{}:", remote_part) },
                                        current_path,
                                        items: Vec::new(),
                                        selected_idx: 0,
                                        loading: true,
                                        error_msg: None,
                                        creating_folder: None,
                                    };
                                    self.refresh_wizard_gui_list(tx.clone()).await;
                                } else {
                                    let current_val = filtered_fields[selected_idx].2.clone();
                                    let current_path = if current_val.is_empty() {
                                        "/".to_string()
                                    } else {
                                        current_val
                                    };

                                    self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                        service_type: ui::services::ServiceType::Mount,
                                        remote: String::new(),
                                        remote_path: String::new(),
                                        current_path,
                                        items: Vec::new(),
                                        selected_idx: 0,
                                        loading: true,
                                        error_msg: None,
                                        creating_folder: None,
                                    };
                                    self.refresh_wizard_gui_list(tx.clone()).await;
                                }
                            }
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
                            self.services_state.wizard = ui::services::ServicesWizardState::EditSystemdService {
                                service_name, file_path, is_user, fields, selected_idx: new_sel, scroll_offset: 0, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                            };
                        }
                    }
                    _ => {}
                }
            }
            ui::services::ServicesWizardState::CreateSystemdService {
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
                            self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key: false, new_key_buffer: String::new()
                            };
                        }
                        KeyCode::Char(c) => {
                            new_key_buffer.push(c);
                            self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key: true, new_key_buffer
                            };
                        }
                        KeyCode::Backspace => {
                            new_key_buffer.pop();
                            self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
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
                                self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
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
                        let remote_list = &self.services_state.all_remotes;
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
                        self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                            fields, selected_idx, scroll_offset, is_editing, input_buffer, active_tab, adding_new_key, new_key_buffer
                        };
                    } else {
                        let mut cursor = self.services_state.edit_cursor_idx;
                        if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                            self.services_state.edit_cursor_idx = cursor;
                            self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                fields, selected_idx, scroll_offset, is_editing: true, input_buffer, active_tab, adding_new_key, new_key_buffer
                            };
                        } else {
                            match key.code {
                                KeyCode::Esc => {
                                    self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                        fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                    };
                                }
                                KeyCode::Enter => {
                                    let field_to_update = filtered_fields[selected_idx].0.clone();
                                    if let Some(item) = fields.iter_mut().find(|(k, _, _, _)| k == &field_to_update) {
                                        item.2 = input_buffer.clone();
                                    }
                                    self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                        fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                    };
                                }
                                KeyCode::Insert => {
                                    if active_tab == 0 && selected_idx < filtered_fields.len() {
                                        let field_name = filtered_fields[selected_idx].0.as_str();
                                        if field_name == "_remote" || field_name == "_mount_path" {
                                            let mut fields_updated = fields.clone();
                                            if let Some(item) = fields_updated.iter_mut().find(|(k, _, _, _)| k == field_name) {
                                                item.2 = input_buffer.clone();
                                            }
                                            self.services_state.systemd_wizard_return = Some(ui::services::WizardReturnTarget::CreateSystemd {
                                                fields: fields_updated,
                                                selected_idx,
                                                scroll_offset,
                                                active_tab,
                                                target_field: field_name.to_string(),
                                            });

                                            if field_name == "_remote" {
                                                let current_val = input_buffer.clone();
                                                let (remote_part, path_part) = if let Some(pos) = current_val.find(':') {
                                                    (current_val[..pos].to_string(), current_val[pos+1..].to_string())
                                                } else {
                                                    (current_val.clone(), String::new())
                                                };
                                                let current_path = if path_part.is_empty() {
                                                    "/".to_string()
                                                } else if path_part.starts_with('/') {
                                                    path_part
                                                } else {
                                                    format!("/{}", path_part)
                                                };

                                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                                    service_type: ui::services::ServiceType::Mount,
                                                    remote: if remote_part.is_empty() { String::new() } else { format!("{}:", remote_part) },
                                                    current_path,
                                                    items: Vec::new(),
                                                    selected_idx: 0,
                                                    loading: true,
                                                    error_msg: None,
                                                    creating_folder: None,
                                                };
                                                self.refresh_wizard_gui_list(tx.clone()).await;
                                            } else {
                                                let current_val = input_buffer.clone();
                                                let current_path = if current_val.is_empty() {
                                                    "/".to_string()
                                                } else {
                                                    current_val
                                                };

                                                self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                                    service_type: ui::services::ServiceType::Mount,
                                                    remote: String::new(),
                                                    remote_path: String::new(),
                                                    current_path,
                                                    items: Vec::new(),
                                                    selected_idx: 0,
                                                    loading: true,
                                                    error_msg: None,
                                                    creating_folder: None,
                                                };
                                                self.refresh_wizard_gui_list(tx.clone()).await;
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    return;
                }

                match key.code {
                    KeyCode::Esc => {
                        self.services_state.wizard = ui::services::ServicesWizardState::None;
                    }
                    KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
                        active_tab = if active_tab == 0 { 1 } else { 0 };
                        self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                            fields, selected_idx: 0, scroll_offset: 0, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                        };
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = total_options - 1;
                        } else {
                            selected_idx -= 1;
                        }
                        scroll_offset = ui::calculate_scroll_range(selected_idx, total_options, 15).start;
                        self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                            fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                        };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % total_options;
                        scroll_offset = ui::calculate_scroll_range(selected_idx, total_options, 15).start;
                        self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
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
                                self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                    fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                                };
                            } else {
                                self.services_state.edit_cursor_idx = val.chars().count();
                                self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
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
                            let home_dir = crate::app_config::get_home_dir();
                            let file_path = if is_user {
                                format!("{}/.config/systemd/user/{}", home_dir, service_name)
                            } else {
                                format!("/etc/systemd/system/{}", service_name)
                            };

                            match self.save_systemd_service_file(true, &service_name, &file_path, is_user, &fields) {
                                Ok(_) => {
                                    self.services_state.info_message = Some(format!("Đã tạo thành công dịch vụ systemd '{}'!", service_name));
                                    self.services_state.wizard = ui::services::ServicesWizardState::None;
                                    self.scan_systemd_services();
                                }
                                Err(e) => {
                                    self.services_state.error_message = Some(format!("Lỗi khi lưu cấu hình dịch vụ: {}", e));
                                }
                            }
                        } else {
                            self.services_state.wizard = ui::services::ServicesWizardState::None;
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
                        let home_dir = crate::app_config::get_home_dir();
                        let file_path = if is_user {
                            format!("{}/.config/systemd/user/{}", home_dir, service_name)
                        } else {
                            format!("/etc/systemd/system/{}", service_name)
                        };

                        match self.save_systemd_service_file(true, &service_name, &file_path, is_user, &fields) {
                            Ok(_) => {
                                self.services_state.info_message = Some(format!("Đã tạo thành công dịch vụ systemd '{}'!", service_name));
                                self.services_state.wizard = ui::services::ServicesWizardState::None;
                                self.scan_systemd_services();
                            }
                            Err(e) => {
                                self.services_state.error_message = Some(format!("Lỗi khi lưu cấu hình dịch vụ: {}", e));
                            }
                        }
                    }
                    KeyCode::Insert => {
                        if active_tab == 1 {
                            self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                fields, selected_idx, scroll_offset, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key: true, new_key_buffer: String::new()
                            };
                        } else if active_tab == 0 && selected_idx < filtered_fields.len() {
                            let field_name = filtered_fields[selected_idx].0.as_str();
                            if field_name == "_remote" || field_name == "_mount_path" {
                                self.services_state.systemd_wizard_return = Some(ui::services::WizardReturnTarget::CreateSystemd {
                                    fields: fields.clone(),
                                    selected_idx,
                                    scroll_offset,
                                    active_tab,
                                    target_field: field_name.to_string(),
                                });

                                if field_name == "_remote" {
                                    let current_val = filtered_fields[selected_idx].2.clone();
                                    let (remote_part, path_part) = if let Some(pos) = current_val.find(':') {
                                        (current_val[..pos].to_string(), current_val[pos+1..].to_string())
                                    } else {
                                        (current_val.clone(), String::new())
                                    };
                                    let current_path = if path_part.is_empty() {
                                        "/".to_string()
                                    } else if path_part.starts_with('/') {
                                        path_part
                                    } else {
                                        format!("/{}", path_part)
                                    };

                                    self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectPath {
                                        service_type: ui::services::ServiceType::Mount,
                                        remote: if remote_part.is_empty() { String::new() } else { format!("{}:", remote_part) },
                                        current_path,
                                        items: Vec::new(),
                                        selected_idx: 0,
                                        loading: true,
                                        error_msg: None,
                                        creating_folder: None,
                                    };
                                    self.refresh_wizard_gui_list(tx.clone()).await;
                                } else {
                                    let current_val = filtered_fields[selected_idx].2.clone();
                                    let current_path = if current_val.is_empty() {
                                        "/".to_string()
                                    } else {
                                        current_val
                                    };

                                    self.services_state.wizard = ui::services::ServicesWizardState::GuiSelectLocalPath {
                                        service_type: ui::services::ServiceType::Mount,
                                        remote: String::new(),
                                        remote_path: String::new(),
                                        current_path,
                                        items: Vec::new(),
                                        selected_idx: 0,
                                        loading: true,
                                        error_msg: None,
                                        creating_folder: None,
                                    };
                                    self.refresh_wizard_gui_list(tx.clone()).await;
                                }
                            }
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
                            self.services_state.wizard = ui::services::ServicesWizardState::CreateSystemdService {
                                fields, selected_idx: new_sel, scroll_offset: 0, is_editing: false, input_buffer: String::new(), active_tab, adding_new_key, new_key_buffer
                            };
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

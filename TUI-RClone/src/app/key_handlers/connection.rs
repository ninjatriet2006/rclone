use crate::rclone;
use crate::ui;
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::app::{
    App, AppEvent, Screen, DeleteTarget, handle_input_key
};

impl App {

    pub(crate) async fn handle_connection_key(
        &mut self,
        key: KeyEvent,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {


        let wizard = self.connection_state.wizard.clone();
        match wizard {
            ui::connection::WizardState::None => {
                match key.code {
                    KeyCode::Esc => {
                        self.screen = Screen::MainMenu;
                    }
                    KeyCode::Up => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) || key.modifiers.contains(KeyModifiers::ALT) {
                            if !self.connection_state.remotes.is_empty() && self.connection_state.selected_idx > 0 {
                                let idx = self.connection_state.selected_idx;
                                let remote1 = self.connection_state.remotes[idx].clone();
                                let remote2 = self.connection_state.remotes[idx - 1].clone();
                                self.connection_state.remotes.swap(idx, idx - 1);
                                self.connection_state.selected_idx -= 1;
                                let config_path = self.config.get_active_profile_path();
                                let _ = crate::app_config::reorder_ini_sections(&config_path, &remote1, &remote2);
                                self.load_remotes(tx.clone()).await;
                            }
                        } else {
                            self.connection_state.prev();
                        }
                    }
                    KeyCode::Down => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) || key.modifiers.contains(KeyModifiers::ALT) {
                            if !self.connection_state.remotes.is_empty() && self.connection_state.selected_idx < self.connection_state.remotes.len() - 1 {
                                let idx = self.connection_state.selected_idx;
                                let remote1 = self.connection_state.remotes[idx].clone();
                                let remote2 = self.connection_state.remotes[idx + 1].clone();
                                self.connection_state.remotes.swap(idx, idx + 1);
                                self.connection_state.selected_idx += 1;
                                let config_path = self.config.get_active_profile_path();
                                let _ = crate::app_config::reorder_ini_sections(&config_path, &remote1, &remote2);
                                self.load_remotes(tx.clone()).await;
                            }
                        } else {
                            self.connection_state.next();
                        }
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') if key.modifiers.contains(KeyModifiers::ALT) => {
                        if !self.connection_state.remotes.is_empty() {
                            let selected_remote = self.connection_state.remotes[self.connection_state.selected_idx].clone();
                            let mut remotes = self.connection_state.remotes.clone();
                            remotes.sort_by(|a, b| {
                                let type_a = self.remote_types.get(a).map(|s| s.as_str()).unwrap_or("Cloud");
                                let type_b = self.remote_types.get(b).map(|s| s.as_str()).unwrap_or("Cloud");
                                match type_a.to_lowercase().cmp(&type_b.to_lowercase()) {
                                    std::cmp::Ordering::Equal => {
                                        crate::app_config::natural_cmp(a, b)
                                    }
                                    ord => ord,
                                }
                            });
                            let config_path = self.config.get_active_profile_path();
                            if let Ok(_) = crate::app_config::save_sorted_remotes_to_ini(&config_path, &remotes) {
                                self.connection_state.remotes = remotes;
                                if let Some(new_idx) = self.connection_state.remotes.iter().position(|r| r == &selected_remote) {
                                    self.connection_state.selected_idx = new_idx;
                                }
                                self.load_remotes(tx.clone()).await;
                            }
                        }
                    }
                    KeyCode::Insert => {
                        // Thêm kết nối mới: Bước 1 load providers
                        let res = rclone::rpc("config/providers", "{}");
                        if let Ok(rpc_res) = res {
                            if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                                if let Some(prov_arr) =
                                    val.get("providers").and_then(|p| p.as_array())
                                {
                                    let mut providers = Vec::new();
                                    for p_val in prov_arr {
                                        let name = p_val
                                            .get("Name")
                                            .and_then(|n| n.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let desc = p_val
                                            .get("Description")
                                            .and_then(|d| d.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        providers.push((name, desc, false));
                                    }
                                    providers.sort_by(|a, b| a.0.cmp(&b.0));
                                    self.connection_state.wizard =
                                        ui::connection::WizardState::SelectProviders {
                                            providers,
                                            selected_idx: 0,
                                            scroll_offset: 0,
                                        };
                                }
                            }
                        }
                    }
                    KeyCode::Char('i') | KeyCode::Char('I') if key.modifiers.contains(KeyModifiers::ALT) => {
                        self.connection_state.wizard = ui::connection::WizardState::ImportConfigInput {
                            input_buffer: String::new(),
                        };
                    }
                    KeyCode::Char('e') | KeyCode::Char('E') if key.modifiers.contains(KeyModifiers::ALT) || (cfg!(target_os = "macos") && key.modifiers.contains(KeyModifiers::CONTROL)) => {
                        // Chỉnh sửa kết nối
                        if !self.connection_state.remotes.is_empty() {
                            let selected_remote = self.connection_state.remotes
                                [self.connection_state.selected_idx]
                                .clone();
                            let param = json!({"name": selected_remote}).to_string();
                            let res = rclone::rpc("config/get", &param);
                            if let Ok(rpc_res) = res {
                                if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                                    if let Some(current_config) = val.as_object() {
                                        let provider = current_config
                                            .get("type")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();

                                        // Truy vấn tất cả các options được hỗ trợ bởi provider này
                                        let mut fields = Vec::new();
                                        let prov_res = rclone::rpc("config/providers", "{}");
                                        if let Ok(prov_rpc_res) = prov_res {
                                            if let Ok(prov_val) =
                                                serde_json::from_str::<Value>(&prov_rpc_res.output)
                                            {
                                                if let Some(prov_arr) = prov_val
                                                    .get("providers")
                                                    .and_then(|p| p.as_array())
                                                {
                                                    // Tìm provider trùng khớp
                                                    if let Some(prov_obj) =
                                                        prov_arr.iter().find(|p| {
                                                            p.get("Name").and_then(|n| n.as_str())
                                                                == Some(&provider)
                                                        })
                                                    {
                                                        if let Some(opts_arr) = prov_obj
                                                            .get("Options")
                                                            .and_then(|o| o.as_array())
                                                        {
                                                            for opt_val in opts_arr {
                                                                let opt_name = opt_val
                                                                    .get("Name")
                                                                    .and_then(|n| n.as_str())
                                                                    .unwrap_or("")
                                                                    .to_string();
                                                                let opt_help = opt_val
                                                                    .get("Help")
                                                                    .and_then(|h| h.as_str())
                                                                    .unwrap_or("")
                                                                    .to_string();

                                                                // Lấy giá trị cấu hình hiện có của remote (nếu đã có)
                                                                let current_val = current_config
                                                                    .get(&opt_name)
                                                                    .map(|v| match v {
                                                                        Value::String(s) => {
                                                                            s.clone()
                                                                        }
                                                                        Value::Number(num) => {
                                                                            num.to_string()
                                                                        }
                                                                        Value::Bool(b) => {
                                                                            b.to_string()
                                                                        }
                                                                        _ => v.to_string(),
                                                                    })
                                                                    .unwrap_or_default();

                                                                // Loại bỏ trường "type" vì type là cố định của remote
                                                                if opt_name != "type" {
                                                                    let opt_required = opt_val
                                                                        .get("Required")
                                                                        .and_then(|r| r.as_bool())
                                                                        .unwrap_or(false);
                                                                    let opt_type = opt_val
                                                                        .get("Type")
                                                                        .and_then(|t| t.as_str())
                                                                        .unwrap_or("");
                                                                    let mut choices = Vec::new();
                                                                    if opt_type == "bool" {
                                                                        choices.push((
                                                                            "true".to_string(),
                                                                            "Đồng ý / Kích hoạt / Bật".to_string(),
                                                                        ));
                                                                        choices.push((
                                                                            "false".to_string(),
                                                                            "Từ chối / Vô hiệu hóa / Tắt".to_string(),
                                                                        ));
                                                                    }
                                                                    if opt_name == "action_policy" || opt_name == "create_policy" || opt_name == "search_policy" {
                                                                        choices = vec![
                                                                            ("epall".to_string(), "Existing Path, All: Chạy lệnh trên tất cả các cloud thành viên chứa đường dẫn.".to_string()),
                                                                            ("epmfs".to_string(), "Existing Path, Most Free Space: Ghi vào cloud thành viên chứa đường dẫn và có nhiều dung lượng trống nhất.".to_string()),
                                                                            ("eplfs".to_string(), "Existing Path, Least Free Space: Ghi vào cloud thành viên chứa đường dẫn và có ít dung lượng trống nhất.".to_string()),
                                                                            ("eprand".to_string(), "Existing Path, Random: Chọn ngẫu nhiên một cloud thành viên chứa đường dẫn.".to_string()),
                                                                            ("epnewest".to_string(), "Existing Path, Newest: Chọn cloud thành viên chứa đường dẫn có tệp tin mới nhất.".to_string()),
                                                                            ("ff".to_string(), "First Found: Chọn cloud thành viên đầu tiên tìm thấy chứa đường dẫn.".to_string()),
                                                                            ("mfs".to_string(), "Most Free Space: Ghi vào cloud thành viên có nhiều dung lượng trống nhất.".to_string()),
                                                                            ("lfs".to_string(), "Least Free Space: Ghi vào cloud thành viên có ít dung lượng trống nhất.".to_string()),
                                                                            ("rand".to_string(), "Random: Chọn ngẫu nhiên một cloud thành viên.".to_string()),
                                                                            ("all".to_string(), "All: Chạy lệnh trên tất cả các cloud thành viên.".to_string()),
                                                                            ("newest".to_string(), "Newest: Chọn cloud thành viên có tệp tin mới nhất.".to_string()),
                                                                        ];
                                                                    }
                                                                    if let Some(examples_arr) =
                                                                        opt_val
                                                                            .get("Examples")
                                                                            .and_then(|e| {
                                                                                e.as_array()
                                                                            })
                                                                    {
                                                                        for ex in examples_arr {
                                                                            if let Some(val) = ex
                                                                                .get("Value")
                                                                                .and_then(|v| {
                                                                                    v.as_str()
                                                                                })
                                                                            {
                                                                                let help = ex
                                                                                    .get("Help")
                                                                                    .and_then(|h| h.as_str())
                                                                                    .unwrap_or("")
                                                                                    .to_string();
                                                                                choices.push((
                                                                                    val.to_string(),
                                                                                    help,
                                                                                ));
                                                                            }
                                                                        }
                                                                    }
                                                                    let mut unique_choices =
                                                                        Vec::new();
                                                                    for c in choices {
                                                                        if !unique_choices
                                                                            .iter()
                                                                            .any(|(val, _)| val == &c.0)
                                                                        {
                                                                            unique_choices.push(c);
                                                                        }
                                                                    }
                                                                    let choices = unique_choices;
                                                                    fields.push((
                                                                        opt_name,
                                                                        opt_help,
                                                                        current_val,
                                                                        choices,
                                                                        opt_required,
                                                                    ));
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // Dự phòng trong trường hợp không truy vấn được config/providers
                                        if fields.is_empty() {
                                            for (k, v) in current_config {
                                                if k != "type" {
                                                    let val_str = match v {
                                                        Value::String(s) => s.clone(),
                                                        Value::Number(num) => num.to_string(),
                                                        Value::Bool(b) => b.to_string(),
                                                        _ => v.to_string(),
                                                    };
                                                    fields.push((
                                                        k.clone(),
                                                        k.clone(),
                                                        val_str,
                                                        Vec::new(),
                                                        false,
                                                    ));
                                                }
                                            }
                                        }

                                        // Sắp xếp: đưa các tham số có giá trị lên đầu, các tham số chưa cấu hình xuống dưới
                                        fields.sort_by(|a, b| {
                                            let a_has = !a.2.is_empty();
                                            let b_has = !b.2.is_empty();
                                            b_has.cmp(&a_has).then_with(|| a.0.cmp(&b.0))
                                        });

                                        fields.insert(0, (
                                            "_remote_name".to_string(),
                                            "Tên của remote / Name of the remote".to_string(),
                                            selected_remote.clone(),
                                            Vec::new(),
                                            true,
                                        ));

                                        self.connection_state.wizard =
                                            ui::connection::WizardState::EditSetup {
                                                remote_name: selected_remote,
                                                provider,
                                                fields,
                                                selected_idx: 0,
                                                scroll_offset: 0,
                                                is_editing: false,
                                                input_buffer: String::new(),
                                                adding_new_key: false,
                                                new_key_buffer: String::new(),
                                                active_tab: 0,
                                            };
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char('?') => {
                        if !self.connection_state.remotes.is_empty() {
                            let selected_remote = self.connection_state.remotes
                                [self.connection_state.selected_idx]
                                .clone();

                            // 1. Kiểm tra cấu hình xem có phải remote dạng union không
                            let param = json!({"name": selected_remote}).to_string();
                            let mut is_union = false;
                            let mut upstreams = Vec::new();
                            if let Ok(rpc_res) = rclone::rpc_async("config/get".to_string(), param).await {
                                if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                                    if let Some(cfg_obj) = val.as_object() {
                                        if cfg_obj.get("type").and_then(|v| v.as_str()) == Some("union") {
                                            is_union = true;
                                            if let Some(upstreams_str) = cfg_obj.get("upstreams").and_then(|v| v.as_str()) {
                                                for u in upstreams_str.split(|c| c == ' ' || c == ',') {
                                                    let u = u.trim();
                                                    if !u.is_empty() {
                                                        let r_name = match u.find(':') {
                                                            Some(idx) => &u[..idx],
                                                            None => u,
                                                        };
                                                        if !r_name.is_empty() && !upstreams.contains(&r_name.to_string()) {
                                                            upstreams.push(r_name.to_string());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // 2. Truy vấn tính năng của remote (và các remote thành viên nếu là union)
                            let mut remotes_to_check = vec![selected_remote.clone()];
                            if is_union {
                                for u in &upstreams {
                                    if !remotes_to_check.contains(u) {
                                        remotes_to_check.push(u.clone());
                                    }
                                }
                            }

                            let mut remote_features = Vec::new();
                            for r in remotes_to_check {
                                let param = json!({ "fs": format!("{}:", r) }).to_string();
                                if let Ok(res) = rclone::rpc_async("operations/fsinfo".to_string(), param).await {
                                    if res.status == 200 {
                                        if let Ok(val) = serde_json::from_str::<Value>(&res.output) {
                                            if let Some(feats) = val.get("Features").and_then(|f| f.as_object()) {
                                                let mut feat_list = Vec::new();
                                                for (k, v) in feats {
                                                    if let Some(b) = v.as_bool() {
                                                        feat_list.push((k.clone(), b));
                                                    }
                                                }
                                                feat_list.sort_by(|a, b| a.0.cmp(&b.0));
                                                remote_features.push((r, feat_list));
                                            }
                                        }
                                    }
                                }
                            }

                            if !remote_features.is_empty() {
                                let selected_feats = remote_features.iter().find(|(name, _)| name == &selected_remote)
                                    .map(|(_, list)| list.clone()).unwrap_or_default();

                                let union_remotes_features = if is_union {
                                    let mut up_list = Vec::new();
                                    for u in &upstreams {
                                        if let Some((_, list)) = remote_features.iter().find(|(name, _)| name == u) {
                                            up_list.push((u.clone(), list.clone()));
                                        }
                                    }
                                    Some(up_list)
                                } else {
                                    None
                                };

                                self.connection_state.wizard = ui::connection::WizardState::ShowFeatures {
                                    remote_name: selected_remote,
                                    features: selected_feats,
                                    union_remotes_features,
                                };
                            } else {
                                self.connection_state.error_message = Some("Không thể tải thông tin tính năng của remote này.".to_string());
                            }
                        }
                    }
                    KeyCode::Delete => {
                        // Hiện cảnh báo xóa kết nối
                        if !self.connection_state.remotes.is_empty() {
                            let selected_remote = self.connection_state.remotes
                                [self.connection_state.selected_idx]
                                .clone();
                            self.delete_confirm = Some(DeleteTarget::Connection(selected_remote));
                        }
                    }
                    _ => {}
                }
            }
            ui::connection::WizardState::SelectProviders {
                mut providers,
                mut selected_idx,
                mut scroll_offset,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.connection_state.wizard = ui::connection::WizardState::None;
                    }
                    KeyCode::Up => {
                        if selected_idx == 0 {
                            selected_idx = providers.len() - 1;
                        } else {
                            selected_idx -= 1;
                        }
                        let term_h =
                            crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                        let popup_h = term_h * 75 / 100;
                        let list_h = popup_h.saturating_sub(2);

                        scroll_offset = ui::update_scroll_offset(selected_idx, scroll_offset, list_h, providers.len());

                        self.connection_state.wizard =
                            ui::connection::WizardState::SelectProviders {
                                providers,
                                selected_idx,
                                scroll_offset,
                            };
                    }
                    KeyCode::Down => {
                        selected_idx = (selected_idx + 1) % providers.len();
                        let term_h =
                            crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                        let popup_h = term_h * 75 / 100;
                        let list_h = popup_h.saturating_sub(2);

                        scroll_offset = ui::update_scroll_offset(selected_idx, scroll_offset, list_h, providers.len());

                        self.connection_state.wizard =
                            ui::connection::WizardState::SelectProviders {
                                providers,
                                selected_idx,
                                scroll_offset,
                            };
                    }
                    KeyCode::Char(' ') => {
                        // Toggle checkbox chọn provider (Bug 27)
                        providers[selected_idx].2 = !providers[selected_idx].2;
                        self.connection_state.wizard =
                            ui::connection::WizardState::SelectProviders {
                                providers,
                                selected_idx,
                                scroll_offset,
                            };
                    }
                    KeyCode::Enter => {
                        // Lấy các provider được tick chọn
                        let selected: Vec<String> = providers
                            .iter()
                            .filter(|(_, _, checked)| *checked)
                            .map(|(name, _, _)| name.clone())
                            .collect();

                        if !selected.is_empty() {
                            self.advance_connection_wizard(selected, tx.clone()).await;
                        } else {
                            // Nếu không tích chọn gì, lấy luôn cái đang hover làm mặc định
                            let current = providers[selected_idx].0.clone();
                            self.advance_connection_wizard(vec![current], tx.clone())
                                .await;
                        }
                    }
                    _ => {}
                }
            }
            ui::connection::WizardState::InputRemoteName {
                provider,
                mut input_buffer,
                selected_providers,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.connection_state.wizard = ui::connection::WizardState::None;
                    }
                    KeyCode::Char(c) => {
                        input_buffer.push(c);
                        self.connection_state.wizard =
                            ui::connection::WizardState::InputRemoteName {
                                provider,
                                input_buffer,
                                selected_providers,
                            };
                    }
                    KeyCode::Backspace => {
                        input_buffer.pop();
                        self.connection_state.wizard =
                            ui::connection::WizardState::InputRemoteName {
                                provider,
                                input_buffer,
                                selected_providers,
                            };
                    }
                    KeyCode::Enter => {
                        let name = input_buffer.trim().to_string();
                        if !name.is_empty() {
                            // Truy vấn các option của provider từ config/providers
                            let mut has_client_id = false;
                            let mut fields = Vec::new();
                            let prov_res = rclone::rpc("config/providers", "{}");
                            if let Ok(prov_rpc_res) = prov_res {
                                if let Ok(prov_val) =
                                    serde_json::from_str::<Value>(&prov_rpc_res.output)
                                {
                                    if let Some(prov_arr) =
                                        prov_val.get("providers").and_then(|p| p.as_array())
                                    {
                                        if let Some(prov_obj) = prov_arr.iter().find(|p| {
                                            p.get("Name").and_then(|n| n.as_str())
                                                == Some(&provider)
                                        }) {
                                            if let Some(opts_arr) =
                                                prov_obj.get("Options").and_then(|o| o.as_array())
                                            {
                                                for opt_val in opts_arr {
                                                    let opt_name = opt_val
                                                        .get("Name")
                                                        .and_then(|n| n.as_str())
                                                        .unwrap_or("")
                                                        .to_string();
                                                    let opt_help = opt_val
                                                        .get("Help")
                                                        .and_then(|h| h.as_str())
                                                        .unwrap_or("")
                                                        .to_string();
                                                    let opt_default = opt_val
                                                        .get("Default")
                                                        .map(|v| match v {
                                                            Value::String(s) => s.clone(),
                                                            Value::Number(num) => num.to_string(),
                                                            Value::Bool(b) => b.to_string(),
                                                            _ => v.to_string(),
                                                        })
                                                        .unwrap_or_default();

                                                    let opt_required = opt_val
                                                        .get("Required")
                                                        .and_then(|r| r.as_bool())
                                                        .unwrap_or(false);
                                                    let opt_type = opt_val
                                                        .get("Type")
                                                        .and_then(|t| t.as_str())
                                                        .unwrap_or("");
                                                    let mut choices = Vec::new();
                                                    if opt_type == "bool" {
                                                        choices.push((
                                                            "true".to_string(),
                                                            "Đồng ý / Kích hoạt / Bật".to_string(),
                                                        ));
                                                        choices.push((
                                                            "false".to_string(),
                                                            "Từ chối / Vô hiệu hóa / Tắt".to_string(),
                                                        ));
                                                    }
                                                    if opt_name == "action_policy" || opt_name == "create_policy" || opt_name == "search_policy" {
                                                        choices = vec![
                                                            ("epall".to_string(), "Existing Path, All: Chạy lệnh trên tất cả các cloud thành viên chứa đường dẫn.".to_string()),
                                                            ("epmfs".to_string(), "Existing Path, Most Free Space: Ghi vào cloud thành viên chứa đường dẫn và có nhiều dung lượng trống nhất.".to_string()),
                                                            ("eplfs".to_string(), "Existing Path, Least Free Space: Ghi vào cloud thành viên chứa đường dẫn và có ít dung lượng trống nhất.".to_string()),
                                                            ("eprand".to_string(), "Existing Path, Random: Chọn ngẫu nhiên một cloud thành viên chứa đường dẫn.".to_string()),
                                                            ("epnewest".to_string(), "Existing Path, Newest: Chọn cloud thành viên chứa đường dẫn có tệp tin mới nhất.".to_string()),
                                                            ("ff".to_string(), "First Found: Chọn cloud thành viên đầu tiên tìm thấy chứa đường dẫn.".to_string()),
                                                            ("mfs".to_string(), "Most Free Space: Ghi vào cloud thành viên có nhiều dung lượng trống nhất.".to_string()),
                                                            ("lfs".to_string(), "Least Free Space: Ghi vào cloud thành viên có ít dung lượng trống nhất.".to_string()),
                                                            ("rand".to_string(), "Random: Chọn ngẫu nhiên một cloud thành viên.".to_string()),
                                                            ("all".to_string(), "All: Chạy lệnh trên tất cả các cloud thành viên.".to_string()),
                                                            ("newest".to_string(), "Newest: Chọn cloud thành viên có tệp tin mới nhất.".to_string()),
                                                        ];
                                                    }
                                                    if let Some(examples_arr) = opt_val
                                                        .get("Examples")
                                                        .and_then(|e| e.as_array())
                                                    {
                                                        for ex in examples_arr {
                                                            if let Some(val) = ex
                                                                .get("Value")
                                                                .and_then(|v| v.as_str())
                                                            {
                                                                let help = ex
                                                                    .get("Help")
                                                                    .and_then(|h| h.as_str())
                                                                    .unwrap_or("")
                                                                    .to_string();
                                                                choices.push((
                                                                    val.to_string(),
                                                                    help,
                                                                ));
                                                            }
                                                        }
                                                    }
                                                    let mut unique_choices = Vec::new();
                                                    for c in choices {
                                                        if !unique_choices
                                                            .iter()
                                                            .any(|(val, _)| val == &c.0)
                                                        {
                                                            unique_choices.push(c);
                                                        }
                                                    }
                                                    let choices = unique_choices;

                                                    if opt_name == "client_id" {
                                                        has_client_id = true;
                                                    }
                                                    if opt_name != "type" {
                                                        fields.push((
                                                            opt_name,
                                                            opt_help,
                                                            opt_default,
                                                            choices,
                                                            opt_required,
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            if has_client_id {
                                // Nhà cung cấp hỗ trợ OAuth (như drive, dropbox, ...) -> Hỏi chế độ auth
                                self.connection_state.wizard =
                                    ui::connection::WizardState::SelectAuthMode {
                                        provider,
                                        remote_name: name,
                                        selected_idx: 0,
                                        selected_providers,
                                    };
                            } else {
                                // Nhà cung cấp thông thường (như crypt, sftp, local, ...) -> Cấu hình trực tiếp tất cả tham số
                                self.connection_state.wizard =
                                    ui::connection::WizardState::AdvancedSetup {
                                        provider,
                                        remote_name: name,
                                        fields,
                                        selected_field_idx: 0,
                                        scroll_offset: 0,
                                        is_editing: false,
                                        input_buffer: String::new(),
                                        selected_providers,
                                        active_tab: 0,
                                    };
                            }
                        }
                    }
                    _ => {}
                }
            }
            ui::connection::WizardState::SelectAuthMode {
                provider,
                remote_name,
                mut selected_idx,
                selected_providers,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.connection_state.wizard = ui::connection::WizardState::None;
                    }
                    KeyCode::Up => {
                        selected_idx = if selected_idx == 0 { 2 } else { selected_idx - 1 };
                        self.connection_state.wizard =
                            ui::connection::WizardState::SelectAuthMode {
                                provider,
                                remote_name,
                                selected_idx,
                                selected_providers,
                            };
                    }
                    KeyCode::Down | KeyCode::Tab => {
                        selected_idx = (selected_idx + 1) % 3;
                        self.connection_state.wizard =
                            ui::connection::WizardState::SelectAuthMode {
                                provider,
                                remote_name,
                                selected_idx,
                                selected_providers,
                            };
                    }
                    KeyCode::Enter => {
                        if selected_idx == 0 {
                            // Simple OAuth: Tự động mở duyệt xác thực
                            let prov_clone = provider.clone();
                            let remote_clone = remote_name.clone();
                            let providers_clone = selected_providers.clone();

                            if prov_clone.to_lowercase() == "zoho" {
                                self.connection_state.wizard =
                                    ui::connection::WizardState::SelectZohoRegion {
                                        provider: prov_clone,
                                        remote_name: remote_clone,
                                        selected_idx: 0,
                                        selected_providers: providers_clone,
                                    };
                            } else {
                                self.connection_state.wizard =
                                    ui::connection::WizardState::SimpleOAuthLoop {
                                        provider: prov_clone.clone(),
                                        remote_name: remote_clone.clone(),
                                        auth_url:
                                            "Đang yêu cầu máy chủ Google/Rclone cấp link xác thực..."
                                                .to_string(),
                                        selected_providers: providers_clone.clone(),
                                    };

                                let tx_oauth = tx.clone();
                                tokio::spawn(async move {
                                    // Gọi API tạo config tự động
                                    let param = json!({
                                        "name": remote_clone,
                                        "type": prov_clone,
                                        "parameters": {
                                            "config_is_local": "true",
                                            "config_automatic": "true"
                                        }
                                    })
                                    .to_string();

                                    // Ở đây giả lập/gọi RPC Rclone thực tế.
                                    // RPC config/create cho OAuth tự động trả URL trong stdout
                                    let res =
                                        rclone::rpc_async("config/create".to_string(), param).await;
                                    match res {
                                        Ok(_) => {
                                            let _ = tx_oauth
                                                .send(AppEvent::OAuthFinished { result: Ok(()) });
                                        }
                                        Err(e) => {
                                            let _ = tx_oauth
                                                .send(AppEvent::OAuthFinished { result: Err(e) });
                                        }
                                    }
                                });

                                let tx_poll = tx.clone();
                                tokio::spawn(async move {
                                    // Poll config/oauthstatus for 60 seconds (300 iterations * 200ms)
                                    for _ in 0..300 {
                                        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                                        let status_res = rclone::rpc_async("config/oauthstatus".to_string(), "{}".to_string()).await;
                                        if let Ok(res) = status_res {
                                            if let Ok(status_val) = serde_json::from_str::<serde_json::Value>(&res.output) {
                                                if status_val.get("status").and_then(|s| s.as_str()) == Some("running") {
                                                    if let Some(auth_url) = status_val.get("authUrl").and_then(|u| u.as_str()) {
                                                        let _ = tx_poll.send(AppEvent::OAuthUrlReceived { url: auth_url.to_string() });
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                        } else if selected_idx == 1 {
                            // Headless OAuth
                            self.connection_state.wizard =
                                ui::connection::WizardState::HeadlessOAuthInput {
                                    provider: provider.clone(),
                                    remote_name: remote_name.clone(),
                                    client_id: String::new(),
                                    client_secret: String::new(),
                                    token_input: String::new(),
                                    focused_idx: 0,
                                    selected_providers: selected_providers.clone(),
                                };
                        } else {
                            // Advanced Setup: Cấu hình tất cả tham số cho provider OAuth này
                            let mut fields = Vec::new();
                            let prov_res = rclone::rpc("config/providers", "{}");
                            if let Ok(prov_rpc_res) = prov_res {
                                if let Ok(prov_val) =
                                    serde_json::from_str::<Value>(&prov_rpc_res.output)
                                {
                                    if let Some(prov_arr) =
                                        prov_val.get("providers").and_then(|p| p.as_array())
                                    {
                                        if let Some(prov_obj) = prov_arr.iter().find(|p| {
                                            p.get("Name").and_then(|n| n.as_str())
                                                == Some(&provider)
                                        }) {
                                            if let Some(opts_arr) =
                                                prov_obj.get("Options").and_then(|o| o.as_array())
                                            {
                                                for opt_val in opts_arr {
                                                    let opt_name = opt_val
                                                        .get("Name")
                                                        .and_then(|n| n.as_str())
                                                        .unwrap_or("")
                                                        .to_string();
                                                    let opt_help = opt_val
                                                        .get("Help")
                                                        .and_then(|h| h.as_str())
                                                        .unwrap_or("")
                                                        .to_string();
                                                    let opt_default = opt_val
                                                        .get("Default")
                                                        .map(|v| match v {
                                                            Value::String(s) => s.clone(),
                                                            Value::Number(num) => num.to_string(),
                                                            Value::Bool(b) => b.to_string(),
                                                            _ => v.to_string(),
                                                        })
                                                        .unwrap_or_default();

                                                    let opt_required = opt_val
                                                        .get("Required")
                                                        .and_then(|r| r.as_bool())
                                                        .unwrap_or(false);
                                                    let opt_type = opt_val
                                                        .get("Type")
                                                        .and_then(|t| t.as_str())
                                                        .unwrap_or("");
                                                    let mut choices = Vec::new();
                                                    if opt_type == "bool" {
                                                        choices.push((
                                                            "true".to_string(),
                                                            "Đồng ý / Kích hoạt / Bật".to_string(),
                                                        ));
                                                        choices.push((
                                                            "false".to_string(),
                                                            "Từ chối / Vô hiệu hóa / Tắt".to_string(),
                                                        ));
                                                    }
                                                    if opt_name == "action_policy" || opt_name == "create_policy" || opt_name == "search_policy" {
                                                        choices = vec![
                                                            ("epall".to_string(), "Existing Path, All: Chạy lệnh trên tất cả các cloud thành viên chứa đường dẫn.".to_string()),
                                                            ("epmfs".to_string(), "Existing Path, Most Free Space: Ghi vào cloud thành viên chứa đường dẫn và có nhiều dung lượng trống nhất.".to_string()),
                                                            ("eplfs".to_string(), "Existing Path, Least Free Space: Ghi vào cloud thành viên chứa đường dẫn và có ít dung lượng trống nhất.".to_string()),
                                                            ("eprand".to_string(), "Existing Path, Random: Chọn ngẫu nhiên một cloud thành viên chứa đường dẫn.".to_string()),
                                                            ("epnewest".to_string(), "Existing Path, Newest: Chọn cloud thành viên chứa đường dẫn có tệp tin mới nhất.".to_string()),
                                                            ("ff".to_string(), "First Found: Chọn cloud thành viên đầu tiên tìm thấy chứa đường dẫn.".to_string()),
                                                            ("mfs".to_string(), "Most Free Space: Ghi vào cloud thành viên có nhiều dung lượng trống nhất.".to_string()),
                                                            ("lfs".to_string(), "Least Free Space: Ghi vào cloud thành viên có ít dung lượng trống nhất.".to_string()),
                                                            ("rand".to_string(), "Random: Chọn ngẫu nhiên một cloud thành viên.".to_string()),
                                                            ("all".to_string(), "All: Chạy lệnh trên tất cả các cloud thành viên.".to_string()),
                                                            ("newest".to_string(), "Newest: Chọn cloud thành viên có tệp tin mới nhất.".to_string()),
                                                        ];
                                                    }
                                                    if let Some(examples_arr) = opt_val
                                                        .get("Examples")
                                                        .and_then(|e| e.as_array())
                                                    {
                                                        for ex in examples_arr {
                                                            if let Some(val) = ex
                                                                .get("Value")
                                                                .and_then(|v| v.as_str())
                                                            {
                                                                let help = ex
                                                                    .get("Help")
                                                                    .and_then(|h| h.as_str())
                                                                    .unwrap_or("")
                                                                    .to_string();
                                                                choices.push((
                                                                    val.to_string(),
                                                                    help,
                                                                ));
                                                            }
                                                        }
                                                    }
                                                    let mut unique_choices = Vec::new();
                                                    for c in choices {
                                                        if !unique_choices
                                                            .iter()
                                                            .any(|(val, _)| val == &c.0)
                                                        {
                                                            unique_choices.push(c);
                                                        }
                                                    }
                                                    let choices = unique_choices;

                                                    if opt_name != "type" {
                                                        fields.push((
                                                            opt_name,
                                                            opt_help,
                                                            opt_default,
                                                            choices,
                                                            opt_required,
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Sắp xếp: đưa client_id và client_secret lên đầu
                            fields.sort_by(|a, b| {
                                let a_is_oauth = a.0 == "client_id" || a.0 == "client_secret";
                                let b_is_oauth = b.0 == "client_id" || b.0 == "client_secret";
                                b_is_oauth.cmp(&a_is_oauth).then_with(|| a.0.cmp(&b.0))
                            });

                            self.connection_state.wizard =
                                ui::connection::WizardState::AdvancedSetup {
                                    provider,
                                    remote_name,
                                    fields,
                                    selected_field_idx: 0,
                                    scroll_offset: 0,
                                    is_editing: false,
                                    input_buffer: String::new(),
                                    selected_providers,
                                    active_tab: 0,
                                };
                        }
                    }
                    _ => {}
                }
            }
            ui::connection::WizardState::HeadlessOAuthInput {
                provider,
                remote_name,
                mut client_id,
                mut client_secret,
                mut token_input,
                mut focused_idx,
                selected_providers,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.connection_state.wizard = ui::connection::WizardState::None;
                    }
                    KeyCode::Tab => {
                        focused_idx = (focused_idx + 1) % 3;
                        self.connection_state.wizard = ui::connection::WizardState::HeadlessOAuthInput {
                            provider, remote_name, client_id, client_secret, token_input, focused_idx, selected_providers
                        };
                    }
                    KeyCode::Char(c) => {
                        if focused_idx == 0 {
                            client_id.push(c);
                        } else if focused_idx == 1 {
                            client_secret.push(c);
                        } else {
                            token_input.push(c);
                        }
                        self.connection_state.wizard = ui::connection::WizardState::HeadlessOAuthInput {
                            provider, remote_name, client_id, client_secret, token_input, focused_idx, selected_providers
                        };
                    }
                    KeyCode::Backspace => {
                        if focused_idx == 0 {
                            client_id.pop();
                        } else if focused_idx == 1 {
                            client_secret.pop();
                        } else {
                            token_input.pop();
                        }
                        self.connection_state.wizard = ui::connection::WizardState::HeadlessOAuthInput {
                            provider, remote_name, client_id, client_secret, token_input, focused_idx, selected_providers
                        };
                    }
                    KeyCode::Enter => {
                        let token_trimmed = token_input.trim().to_string();
                        if token_trimmed.is_empty() {
                            self.connection_state.error_message = Some("Token không được để trống!".to_string());
                            return;
                        }

                        let mut params = serde_json::Map::new();
                        params.insert("token".to_string(), serde_json::Value::String(token_trimmed));
                        if !client_id.trim().is_empty() {
                            params.insert("client_id".to_string(), serde_json::Value::String(client_id.trim().to_string()));
                        }
                        if !client_secret.trim().is_empty() {
                            params.insert("client_secret".to_string(), serde_json::Value::String(client_secret.trim().to_string()));
                        }

                        let rclone_param = json!({
                            "name": remote_name,
                            "type": provider,
                            "parameters": params,
                            "obscure": true
                        })
                        .to_string();

                        let res = rclone::rpc("config/create", &rclone_param);
                        match res {
                            Ok(rpc_res) if rpc_res.status == 200 => {
                                self.connection_state.info_message = Some(format!("Tạo kết nối '{}' thành công qua Headless OAuth!", remote_name));
                                self.connection_state.wizard = ui::connection::WizardState::None;
                                self.load_remotes(tx.clone()).await;
                            }
                            Ok(rpc_res) => {
                                self.connection_state.error_message = Some(format!("Mã lỗi RPC: {}. Chi tiết: {}", rpc_res.status, rpc_res.output));
                            }
                            Err(e) => {
                                self.connection_state.error_message = Some(format!("Lỗi gọi RPC: {}", e));
                            }
                        }
                    }
                    _ => {}
                }
            }
            ui::connection::WizardState::SelectZohoRegion {
                provider,
                remote_name,
                mut selected_idx,
                selected_providers,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.connection_state.wizard = ui::connection::WizardState::SelectAuthMode {
                            provider,
                            remote_name,
                            selected_idx: 0,
                            selected_providers,
                        };
                    }
                    KeyCode::Up => {
                        selected_idx = if selected_idx == 0 { 5 } else { selected_idx - 1 };
                        self.connection_state.wizard = ui::connection::WizardState::SelectZohoRegion {
                            provider,
                            remote_name,
                            selected_idx,
                            selected_providers,
                        };
                    }
                    KeyCode::Down | KeyCode::Tab => {
                        selected_idx = (selected_idx + 1) % 6;
                        self.connection_state.wizard = ui::connection::WizardState::SelectZohoRegion {
                            provider,
                            remote_name,
                            selected_idx,
                            selected_providers,
                        };
                    }
                    KeyCode::Enter => {
                        let regions = vec!["com", "eu", "in", "jp", "com.cn", "com.au"];
                        let selected_region = regions[selected_idx].to_string();

                        let prov_clone = provider.clone();
                        let remote_clone = remote_name.clone();
                        let providers_clone = selected_providers.clone();

                        self.connection_state.wizard =
                            ui::connection::WizardState::SimpleOAuthLoop {
                                provider: prov_clone.clone(),
                                remote_name: remote_clone.clone(),
                                auth_url:
                                    "Đang yêu cầu máy chủ Google/Rclone cấp link xác thực..."
                                        .to_string(),
                                selected_providers: providers_clone.clone(),
                            };

                        let tx_oauth = tx.clone();
                        tokio::spawn(async move {
                            // Gọi API tạo config tự động với region được chọn
                            let param = json!({
                                "name": remote_clone,
                                "type": prov_clone,
                                "parameters": {
                                    "config_is_local": "true",
                                    "config_automatic": "true",
                                    "region": selected_region
                                }
                            })
                            .to_string();

                            let res =
                                rclone::rpc_async("config/create".to_string(), param).await;
                            match res {
                                Ok(_) => {
                                    let _ = tx_oauth
                                        .send(AppEvent::OAuthFinished { result: Ok(()) });
                                }
                                Err(e) => {
                                    let _ = tx_oauth
                                        .send(AppEvent::OAuthFinished { result: Err(e) });
                                }
                            }
                        });

                        let tx_poll = tx.clone();
                        tokio::spawn(async move {
                            // Poll config/oauthstatus for 60 seconds (300 iterations * 200ms)
                            for _ in 0..300 {
                                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                                let status_res = rclone::rpc_async("config/oauthstatus".to_string(), "{}".to_string()).await;
                                if let Ok(res) = status_res {
                                    if let Ok(status_val) = serde_json::from_str::<serde_json::Value>(&res.output) {
                                        if status_val.get("status").and_then(|s| s.as_str()) == Some("running") {
                                            if let Some(auth_url) = status_val.get("authUrl").and_then(|u| u.as_str()) {
                                                let _ = tx_poll.send(AppEvent::OAuthUrlReceived { url: auth_url.to_string() });
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        });
                    }
                    _ => {}
                }
            }
            ui::connection::WizardState::SimpleOAuthLoop { .. } => {
                if key.code == KeyCode::Esc {
                    // Hủy OAuth
                    self.connection_state.wizard = ui::connection::WizardState::None;
                    tokio::spawn(async move {
                        let _ = rclone::rpc_async("config/oauthstop".to_string(), "{}".to_string()).await;
                    });
                }
            }
            ui::connection::WizardState::AdvancedSetup {
                provider,
                remote_name,
                mut fields,
                mut selected_field_idx,
                mut scroll_offset,
                mut is_editing,
                mut input_buffer,
                selected_providers,
                active_tab,
            } => {
                // Lọc danh sách fields theo tab
                let filtered_fields: Vec<(String, String, String, Vec<(String, String)>, bool)> = fields
                    .iter()
                    .filter(|(name, _, _, _, required)| {
                        if active_tab == 0 {
                            ui::connection::is_basic_field(name, *required)
                        } else {
                            !ui::connection::is_basic_field(name, *required)
                        }
                    })
                    .cloned()
                    .collect();

                let save_idx = filtered_fields.len();
                let cancel_idx = filtered_fields.len() + 1;
                let total_items = filtered_fields.len() + 2;

                if is_editing {
                    let is_remote_field =
                        filtered_fields.get(selected_field_idx).map(|f| f.0.as_str()) == Some("remote");
                    let field_choices = filtered_fields.get(selected_field_idx).map(|f| &f.3);
                    if is_remote_field && (key.code == KeyCode::Up || key.code == KeyCode::Down) {
                        let remote_list = &self.connection_state.remotes;
                        if !remote_list.is_empty() {
                            let current_val = input_buffer.trim_end_matches(':');
                            let current_idx = remote_list.iter().position(|r| r == current_val);
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
                            input_buffer = format!("{}:", remote_list[next_idx]);
                        }
                        self.connection_state.wizard = ui::connection::WizardState::AdvancedSetup {
                            provider,
                            remote_name,
                            fields,
                            selected_field_idx,
                            scroll_offset,
                            is_editing,
                            input_buffer,
                            selected_providers,
                            active_tab,
                        };
                    } else if let Some(choices) = field_choices {
                        if !choices.is_empty()
                            && (key.code == KeyCode::Up || key.code == KeyCode::Down)
                        {
                            let current_idx = choices.iter().position(|(c, _)| c == &input_buffer);
                            let next_idx = match current_idx {
                                Some(idx) => {
                                    if key.code == KeyCode::Up {
                                        if idx == 0 { choices.len() - 1 } else { idx - 1 }
                                    } else {
                                        (idx + 1) % choices.len()
                                    }
                                }
                                None => 0,
                            };
                            input_buffer = choices[next_idx].0.clone();
                            self.connection_state.wizard =
                                ui::connection::WizardState::AdvancedSetup {
                                    provider,
                                    remote_name,
                                    fields,
                                    selected_field_idx,
                                    scroll_offset,
                                    is_editing,
                                    input_buffer,
                                    selected_providers,
                                    active_tab,
                                };
                        } else {
                            let mut cursor = self.connection_state.edit_cursor_idx;
                            if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                                self.connection_state.edit_cursor_idx = cursor;
                                self.connection_state.wizard =
                                    ui::connection::WizardState::AdvancedSetup {
                                        provider,
                                        remote_name,
                                        fields,
                                        selected_field_idx,
                                        scroll_offset,
                                        is_editing,
                                        input_buffer,
                                        selected_providers,
                                        active_tab,
                                    };
                            } else {
                                match key.code {
                                    KeyCode::Esc => {
                                        is_editing = false;
                                        self.connection_state.wizard =
                                            ui::connection::WizardState::AdvancedSetup {
                                                provider,
                                                remote_name,
                                                fields,
                                                selected_field_idx,
                                                scroll_offset,
                                                is_editing,
                                                input_buffer,
                                                selected_providers,
                                                active_tab,
                                            };
                                    }
                                    KeyCode::Enter => {
                                        if let Some(f) = filtered_fields.get(selected_field_idx) {
                                            if let Some(real_idx) = fields.iter().position(|real_f| real_f.0 == f.0) {
                                                fields[real_idx].2 = input_buffer.clone();
                                            }
                                        }
                                        is_editing = false;
                                        self.connection_state.wizard =
                                            ui::connection::WizardState::AdvancedSetup {
                                                provider,
                                                remote_name,
                                                fields,
                                                selected_field_idx,
                                                scroll_offset,
                                                is_editing,
                                                input_buffer,
                                                selected_providers,
                                                active_tab,
                                            };
                                    }
                                    KeyCode::Insert => {
                                        let is_api_key_field = filtered_fields
                                            .get(selected_field_idx)
                                            .map(|f| f.0.as_str())
                                            == Some("api_key");
                                        if provider.to_lowercase() == "filen" && is_api_key_field {
                                            if let Some(key_val) = try_get_filen_api_key() {
                                                input_buffer = key_val;
                                                self.connection_state.edit_cursor_idx = input_buffer.chars().count();
                                                self.connection_state.wizard =
                                                    ui::connection::WizardState::AdvancedSetup {
                                                        provider,
                                                        remote_name,
                                                        fields,
                                                        selected_field_idx,
                                                        scroll_offset,
                                                        is_editing,
                                                        input_buffer,
                                                        selected_providers,
                                                        active_tab,
                                                    };
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                } else {
                    match key.code {
                        KeyCode::Esc => {
                            self.connection_state.wizard = ui::connection::WizardState::None;
                        }
                        KeyCode::Tab | KeyCode::Right | KeyCode::Left => {
                            let new_tab = if active_tab == 0 { 1 } else { 0 };
                            self.connection_state.wizard = ui::connection::WizardState::AdvancedSetup {
                                provider,
                                remote_name,
                                fields,
                                selected_field_idx: 0,
                                scroll_offset: 0,
                                is_editing: false,
                                input_buffer: String::new(),
                                selected_providers,
                                active_tab: new_tab,
                            };
                        }
                        KeyCode::Up => {
                            if selected_field_idx == 0 {
                                selected_field_idx = total_items - 1;
                            } else {
                                selected_field_idx -= 1;
                            }
                            let term_h =
                                crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                            let popup_h = term_h * 75 / 100;
                            let list_h = popup_h.saturating_sub(4);

                            if selected_field_idx < filtered_fields.len() {
                                scroll_offset = ui::update_scroll_offset(selected_field_idx, scroll_offset, list_h, filtered_fields.len());
                            } else {
                                scroll_offset = filtered_fields.len().saturating_sub(list_h);
                            }

                            self.connection_state.wizard =
                                ui::connection::WizardState::AdvancedSetup {
                                    provider,
                                    remote_name,
                                    fields,
                                    selected_field_idx,
                                    scroll_offset,
                                    is_editing,
                                    input_buffer,
                                    selected_providers,
                                    active_tab,
                                };
                        }
                        KeyCode::Down => {
                            selected_field_idx = (selected_field_idx + 1) % total_items;
                            let term_h =
                                crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                            let popup_h = term_h * 75 / 100;
                            let list_h = popup_h.saturating_sub(4);

                            if selected_field_idx < filtered_fields.len() {
                                scroll_offset = ui::update_scroll_offset(selected_field_idx, scroll_offset, list_h, filtered_fields.len());
                            } else {
                                scroll_offset = filtered_fields.len().saturating_sub(list_h);
                            }

                            self.connection_state.wizard =
                                ui::connection::WizardState::AdvancedSetup {
                                    provider,
                                    remote_name,
                                    fields,
                                    selected_field_idx,
                                    scroll_offset,
                                    is_editing,
                                    input_buffer,
                                    selected_providers,
                                    active_tab,
                                };
                        }
                        KeyCode::Enter => {
                            if selected_field_idx < filtered_fields.len() {
                                let f = &filtered_fields[selected_field_idx];
                                let field_name = f.0.clone();
                                if field_name == "upstreams" {
                                    let mut options = Vec::new();
                                    let current_vals: Vec<&str> = f.2.split_whitespace().collect();
                                    for r in &self.connection_state.remotes {
                                        let r_fmt = format!("{}:", r);
                                        let checked = current_vals.iter().any(|&cv| cv == r_fmt || cv.trim_end_matches(':') == r);
                                        options.push((r_fmt, checked));
                                    }
                                    self.connection_state.wizard = ui::connection::WizardState::SelectMultipleChoices {
                                        provider,
                                        remote_name,
                                        fields,
                                        selected_field_idx,
                                        scroll_offset,
                                        active_tab,
                                        selected_providers,
                                        is_edit_mode: false,
                                        field_name,
                                        options,
                                        choices_selected_idx: 0,
                                    };
                                } else {
                                    let mut field_choices = f.3.clone();
                                    let name_lower = field_name.to_lowercase();
                                    if name_lower == "remote" {
                                        for r in &self.connection_state.remotes {
                                            field_choices.push((format!("{}:", r), String::new()));
                                        }
                                    }
                                    if !field_choices.is_empty() {
                                        let current_val = f.2.clone();
                                        let choices_selected_idx = field_choices.iter().position(|(c, _)| c == &current_val).unwrap_or(0);
                                        self.connection_state.wizard = ui::connection::WizardState::SelectOneChoice {
                                            provider,
                                            remote_name,
                                            fields,
                                            selected_field_idx,
                                            scroll_offset,
                                            active_tab,
                                            selected_providers,
                                            is_edit_mode: false,
                                            field_name,
                                            choices: field_choices,
                                            choices_selected_idx,
                                        };
                                    } else {
                                        is_editing = true;
                                        input_buffer = f.2.clone();
                                        self.connection_state.edit_cursor_idx = input_buffer.chars().count();
                                        self.connection_state.wizard = ui::connection::WizardState::AdvancedSetup {
                                            provider,
                                            remote_name,
                                            fields,
                                            selected_field_idx,
                                            scroll_offset,
                                            is_editing,
                                            input_buffer,
                                            selected_providers,
                                            active_tab,
                                        };
                                    }
                                }
                            } else if selected_field_idx == save_idx {
                                // Kiểm tra các trường bắt buộc
                                if let Err(missing) = validate_required_fields(&fields) {
                                    self.connection_state.error_message = Some(format!(
                                        "Vui lòng điền đầy đủ các trường bắt buộc: {}",
                                        missing.join(", ")
                                    ));
                                    return;
                                }
                                // Lưu cấu hình remote mới
                                let mut params = HashMap::new();
                                for (name, _, val, _, _) in fields.iter() {
                                    let val_trimmed = val.trim();
                                    let is_empty_password = (name.to_lowercase().contains("pass")
                                        || name.to_lowercase().contains("salt")
                                        || name.to_lowercase().contains("secret")
                                        || name.to_lowercase().contains("key")
                                        || name.to_lowercase().contains("token")
                                        || name == "password2")
                                        && val_trimmed.is_empty();
                                    if !is_empty_password {
                                        params.insert(name.clone(), val.clone());
                                    }
                                }
                                let rclone_param = json!({
                                    "name": remote_name,
                                    "type": provider,
                                    "parameters": params,
                                    "obscure": true
                                })
                                .to_string();

                                let res = rclone::rpc("config/create", &rclone_param);
                                match res {
                                    Ok(_) => {
                                        self.connection_state.info_message = Some(format!(
                                            "Đã tạo remote '{}' thành công!",
                                            remote_name
                                        ));
                                        self.advance_connection_wizard(
                                            selected_providers,
                                            tx.clone(),
                                        )
                                        .await;
                                        self.load_remotes(tx.clone()).await;
                                    }
                                    Err(e) => {
                                        self.connection_state.error_message =
                                            Some(format!("Lỗi khi tạo remote: {}", e));
                                    }
                                }
                            } else if selected_field_idx == cancel_idx {
                                self.connection_state.wizard = ui::connection::WizardState::None;
                            }
                        }
                        KeyCode::Char('s') | KeyCode::Char('S') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            // Kiểm tra các trường bắt buộc
                            if let Err(missing) = validate_required_fields(&fields) {
                                self.connection_state.error_message = Some(format!(
                                    "Vui lòng điền đầy đủ các trường bắt buộc: {}",
                                    missing.join(", ")
                                ));
                                return;
                            }
                            // Lưu cấu hình remote mới
                            let mut params = HashMap::new();
                            for (name, _, val, _, _) in fields.iter() {
                                let val_trimmed = val.trim();
                                let is_empty_password = (name.to_lowercase().contains("pass")
                                    || name.to_lowercase().contains("salt")
                                    || name.to_lowercase().contains("secret")
                                    || name.to_lowercase().contains("key")
                                    || name.to_lowercase().contains("token")
                                    || name == "password2")
                                    && val_trimmed.is_empty();
                                if !is_empty_password {
                                    params.insert(name.clone(), val.clone());
                                }
                            }
                            let rclone_param = json!({
                            "name": remote_name,
                            "type": provider,
                            "parameters": params,
                            "obscure": true
                            })
                            .to_string();

                            let res = rclone::rpc("config/create", &rclone_param);
                            match res {
                                Ok(_) => {
                                    self.connection_state.info_message = Some(format!(
                                        "Đã tạo remote '{}' thành công!",
                                        remote_name
                                    ));
                                    self.advance_connection_wizard(selected_providers, tx.clone())
                                        .await;
                                    self.load_remotes(tx.clone()).await;
                                }
                                Err(e) => {
                                    self.connection_state.error_message =
                                        Some(format!("Lỗi khi tạo remote: {}", e));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            ui::connection::WizardState::EditSetup {
                remote_name,
                provider,
                mut fields,
                mut selected_idx,
                mut scroll_offset,
                mut is_editing,
                mut input_buffer,
                adding_new_key: _,
                new_key_buffer: _,
                active_tab,
            } => {
                // Lọc danh sách fields theo tab
                let filtered_fields: Vec<(String, String, String, Vec<(String, String)>, bool)> = fields
                    .iter()
                    .filter(|(name, _, _, _, required)| {
                        if active_tab == 0 {
                            ui::connection::is_basic_field(name, *required)
                        } else {
                            !ui::connection::is_basic_field(name, *required)
                        }
                    })
                    .cloned()
                    .collect();

                let save_idx = filtered_fields.len();
                let cancel_idx = filtered_fields.len() + 1;
                let total_items = filtered_fields.len() + 2;

                if is_editing {
                    let is_remote_field =
                        filtered_fields.get(selected_idx).map(|f| f.0.as_str()) == Some("remote");
                    let field_choices = filtered_fields.get(selected_idx).map(|f| &f.3);
                    if is_remote_field && (key.code == KeyCode::Up || key.code == KeyCode::Down) {
                        let remote_list = &self.connection_state.remotes;
                        if !remote_list.is_empty() {
                            let current_val = input_buffer.trim_end_matches(':');
                            let current_idx = remote_list.iter().position(|r| r == current_val);
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
                            input_buffer = format!("{}:", remote_list[next_idx]);
                        }
                        self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                            remote_name,
                            provider,
                            fields,
                            selected_idx,
                            scroll_offset,
                            is_editing,
                            input_buffer,
                            adding_new_key: false,
                            new_key_buffer: String::new(),
                            active_tab,
                        };
                    } else if let Some(choices) = field_choices {
                        if !choices.is_empty()
                            && (key.code == KeyCode::Up || key.code == KeyCode::Down)
                        {
                            let current_idx = choices.iter().position(|(c, _)| c == &input_buffer);
                            let next_idx = match current_idx {
                                Some(idx) => {
                                    if key.code == KeyCode::Up {
                                        if idx == 0 { choices.len() - 1 } else { idx - 1 }
                                    } else {
                                        (idx + 1) % choices.len()
                                    }
                                }
                                None => 0,
                            };
                            input_buffer = choices[next_idx].0.clone();
                            self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                                remote_name,
                                provider,
                                fields,
                                selected_idx,
                                scroll_offset,
                                is_editing,
                                input_buffer,
                                adding_new_key: false,
                                new_key_buffer: String::new(),
                                active_tab,
                            };
                        } else {
                            let mut cursor = self.connection_state.edit_cursor_idx;
                            if handle_input_key(&key, &mut input_buffer, &mut cursor) {
                                self.connection_state.edit_cursor_idx = cursor;
                                self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                                    remote_name,
                                    provider,
                                    fields,
                                    selected_idx,
                                    scroll_offset,
                                    is_editing,
                                    input_buffer,
                                    adding_new_key: false,
                                    new_key_buffer: String::new(),
                                    active_tab,
                                };
                            } else {
                                match key.code {
                                    KeyCode::Esc => {
                                        is_editing = false;
                                        self.connection_state.wizard =
                                            ui::connection::WizardState::EditSetup {
                                                remote_name,
                                                provider,
                                                fields,
                                                selected_idx,
                                                scroll_offset,
                                                is_editing,
                                                input_buffer,
                                                adding_new_key: false,
                                                new_key_buffer: String::new(),
                                                active_tab,
                                            };
                                    }
                                    KeyCode::Enter => {
                                        if let Some(f) = filtered_fields.get(selected_idx) {
                                            if let Some(real_idx) = fields.iter().position(|real_f| real_f.0 == f.0) {
                                                fields[real_idx].2 = input_buffer.clone();
                                            }
                                        }
                                        is_editing = false;
                                        self.connection_state.wizard =
                                            ui::connection::WizardState::EditSetup {
                                                remote_name,
                                                provider,
                                                fields,
                                                selected_idx,
                                                scroll_offset,
                                                is_editing,
                                                input_buffer,
                                                adding_new_key: false,
                                                new_key_buffer: String::new(),
                                                active_tab,
                                            };
                                    }
                                    KeyCode::Insert => {
                                        let is_api_key_field = filtered_fields
                                            .get(selected_idx)
                                            .map(|f| f.0.as_str())
                                            == Some("api_key");
                                        if provider.to_lowercase() == "filen" && is_api_key_field {
                                            if let Some(key_val) = try_get_filen_api_key() {
                                                input_buffer = key_val;
                                                self.connection_state.edit_cursor_idx = input_buffer.chars().count();
                                                self.connection_state.wizard =
                                                    ui::connection::WizardState::EditSetup {
                                                        remote_name,
                                                        provider,
                                                        fields,
                                                        selected_idx,
                                                        scroll_offset,
                                                        is_editing,
                                                        input_buffer,
                                                        adding_new_key: false,
                                                        new_key_buffer: String::new(),
                                                        active_tab,
                                                    };
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                } else {
                    match key.code {
                        KeyCode::Esc => {
                            self.connection_state.wizard = ui::connection::WizardState::None;
                        }
                        KeyCode::Tab | KeyCode::Right | KeyCode::Left => {
                            let new_tab = if active_tab == 0 { 1 } else { 0 };
                            self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                                remote_name,
                                provider,
                                fields,
                                selected_idx: 0,
                                scroll_offset: 0,
                                is_editing: false,
                                input_buffer: String::new(),
                                adding_new_key: false,
                                new_key_buffer: String::new(),
                                active_tab: new_tab,
                            };
                        }
                        KeyCode::Up => {
                            if selected_idx == 0 {
                                selected_idx = total_items - 1;
                            } else {
                                selected_idx -= 1;
                            }
                            let term_h =
                                crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                            let popup_h = term_h * 75 / 100;
                            let list_h = popup_h.saturating_sub(4);

                            if selected_idx < filtered_fields.len() {
                                scroll_offset = ui::update_scroll_offset(selected_idx, scroll_offset, list_h, filtered_fields.len());
                            } else {
                                scroll_offset = filtered_fields.len().saturating_sub(list_h);
                            }

                            self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                                remote_name,
                                provider,
                                fields,
                                selected_idx,
                                scroll_offset,
                                is_editing,
                                input_buffer,
                                adding_new_key: false,
                                new_key_buffer: String::new(),
                                active_tab,
                            };
                        }
                        KeyCode::Down => {
                            selected_idx = (selected_idx + 1) % total_items;
                            let term_h =
                                crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24) as usize;
                            let popup_h = term_h * 75 / 100;
                            let list_h = popup_h.saturating_sub(4);

                            if selected_idx < filtered_fields.len() {
                                scroll_offset = ui::update_scroll_offset(selected_idx, scroll_offset, list_h, filtered_fields.len());
                            } else {
                                scroll_offset = filtered_fields.len().saturating_sub(list_h);
                            }

                            self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                                remote_name,
                                provider,
                                fields,
                                selected_idx,
                                scroll_offset,
                                is_editing,
                                input_buffer,
                                adding_new_key: false,
                                new_key_buffer: String::new(),
                                active_tab,
                            };
                        }
                        KeyCode::Enter => {
                            if selected_idx < filtered_fields.len() {
                                let f = &filtered_fields[selected_idx];
                                let field_name = f.0.clone();
                                if field_name == "upstreams" {
                                    let mut options = Vec::new();
                                    let current_vals: Vec<&str> = f.2.split_whitespace().collect();
                                    for r in &self.connection_state.remotes {
                                        let r_fmt = format!("{}:", r);
                                        let checked = current_vals.iter().any(|&cv| cv == r_fmt || cv.trim_end_matches(':') == r);
                                        options.push((r_fmt, checked));
                                    }
                                    self.connection_state.wizard = ui::connection::WizardState::SelectMultipleChoices {
                                        provider,
                                        remote_name,
                                        fields,
                                        selected_field_idx: selected_idx,
                                        scroll_offset,
                                        active_tab,
                                        selected_providers: Vec::new(),
                                        is_edit_mode: true,
                                        field_name,
                                        options,
                                        choices_selected_idx: 0,
                                    };
                                } else {
                                    let mut field_choices = f.3.clone();
                                    let name_lower = field_name.to_lowercase();
                                    if name_lower == "remote" {
                                        for r in &self.connection_state.remotes {
                                            field_choices.push((format!("{}:", r), String::new()));
                                        }
                                    }
                                    if !field_choices.is_empty() {
                                        let current_val = f.2.clone();
                                        let choices_selected_idx = field_choices.iter().position(|(c, _)| c == &current_val).unwrap_or(0);
                                        self.connection_state.wizard = ui::connection::WizardState::SelectOneChoice {
                                            provider,
                                            remote_name,
                                            fields,
                                            selected_field_idx: selected_idx,
                                            scroll_offset,
                                            active_tab,
                                            selected_providers: Vec::new(),
                                            is_edit_mode: true,
                                            field_name,
                                            choices: field_choices,
                                            choices_selected_idx,
                                        };
                                    } else {
                                        is_editing = true;
                                        input_buffer = f.2.clone();
                                        self.connection_state.edit_cursor_idx = input_buffer.chars().count();
                                        self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                                            remote_name,
                                            provider,
                                            fields,
                                            selected_idx,
                                            scroll_offset,
                                            is_editing,
                                            input_buffer,
                                            adding_new_key: false,
                                            new_key_buffer: String::new(),
                                            active_tab,
                                        };
                                    }
                                }
                            } else if selected_idx == save_idx {
                                // Kiểm tra các trường bắt buộc
                                if let Err(missing) = validate_required_fields(&fields) {
                                    self.connection_state.error_message = Some(format!(
                                        "Vui lòng điền đầy đủ các trường bắt buộc: {}",
                                        missing.join(", ")
                                    ));
                                    return;
                                }
                                let mut params = HashMap::new();
                                let mut new_remote_name = remote_name.clone();
                                for (name, _, val, _, _) in fields.iter() {
                                    if name == "_remote_name" {
                                        new_remote_name = val.trim().to_string();
                                    } else {
                                        let val_trimmed = val.trim();
                                        let is_empty_password = (name.to_lowercase().contains("pass")
                                            || name.to_lowercase().contains("salt")
                                            || name.to_lowercase().contains("secret")
                                            || name.to_lowercase().contains("key")
                                            || name.to_lowercase().contains("token")
                                            || name == "password2")
                                            && val_trimmed.is_empty();
                                        if !is_empty_password {
                                            params.insert(name.clone(), val.clone());
                                        }
                                    }
                                }

                                if new_remote_name.is_empty() {
                                    self.connection_state.error_message =
                                        Some("Tên remote không được để trống!".to_string());
                                } else if new_remote_name != remote_name {
                                    let rclone_param = json!({
                                        "name": new_remote_name,
                                        "type": provider,
                                        "parameters": params,
                                        "obscure": true
                                    })
                                    .to_string();

                                    let create_res = rclone::rpc("config/create", &rclone_param);
                                    match create_res {
                                        Ok(_) => {
                                            let delete_param = json!({
                                                "name": remote_name
                                            })
                                            .to_string();
                                            let _ = rclone::rpc("config/delete", &delete_param);

                                            self.connection_state.info_message = Some(format!(
                                                "Đã đổi tên remote thành '{}' thành công!",
                                                new_remote_name
                                            ));
                                            self.connection_state.wizard =
                                                ui::connection::WizardState::None;
                                            self.load_remotes(tx.clone()).await;
                                        }
                                        Err(e) => {
                                            self.connection_state.error_message =
                                                Some(format!("Lỗi khi đổi tên remote: {}", e));
                                        }
                                    }
                                } else {
                                    let rclone_param = json!({
                                        "name": remote_name,
                                        "parameters": params,
                                        "obscure": true
                                    })
                                    .to_string();

                                    let rpc_res = rclone::rpc("config/update", &rclone_param);
                                    match rpc_res {
                                        Ok(_) => {
                                            self.connection_state.info_message = Some(format!(
                                                "Đã cập nhật remote '{}' thành công!",
                                                remote_name
                                            ));
                                            self.connection_state.wizard =
                                                ui::connection::WizardState::None;
                                            self.load_remotes(tx.clone()).await;
                                        }
                                        Err(e) => {
                                            self.connection_state.error_message =
                                                Some(format!("Lỗi khi cập nhật remote: {}", e));
                                        }
                                    }
                                }
                            } else if selected_idx == cancel_idx {
                                self.connection_state.wizard = ui::connection::WizardState::None;
                            }
                        }
                        KeyCode::Char('s') | KeyCode::Char('S') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            // Kiểm tra các trường bắt buộc
                            if let Err(missing) = validate_required_fields(&fields) {
                                self.connection_state.error_message = Some(format!(
                                    "Vui lòng điền đầy đủ các trường bắt buộc: {}",
                                    missing.join(", ")
                                ));
                                return;
                            }
                            let mut params = HashMap::new();
                            let mut new_remote_name = remote_name.clone();
                            for (name, _, val, _, _) in fields.iter() {
                                if name == "_remote_name" {
                                    new_remote_name = val.trim().to_string();
                                } else {
                                    let val_trimmed = val.trim();
                                    let is_empty_password = (name.to_lowercase().contains("pass")
                                        || name.to_lowercase().contains("salt")
                                        || name.to_lowercase().contains("secret")
                                        || name.to_lowercase().contains("key")
                                        || name.to_lowercase().contains("token")
                                        || name == "password2")
                                        && val_trimmed.is_empty();
                                    if !is_empty_password {
                                        params.insert(name.clone(), val.clone());
                                    }
                                }
                            }

                            if new_remote_name.is_empty() {
                                self.connection_state.error_message =
                                    Some("Tên remote không được để trống!".to_string());
                            } else if new_remote_name != remote_name {
                                let rclone_param = json!({
                                    "name": new_remote_name,
                                    "type": provider,
                                    "parameters": params,
                                    "obscure": true
                                })
                                .to_string();

                                let create_res = rclone::rpc("config/create", &rclone_param);
                                match create_res {
                                    Ok(_) => {
                                        let delete_param = json!({
                                            "name": remote_name
                                        })
                                        .to_string();
                                        let _ = rclone::rpc("config/delete", &delete_param);

                                        self.connection_state.info_message = Some(format!(
                                            "Đã đổi tên remote thành '{}' thành công!",
                                            new_remote_name
                                        ));
                                        self.connection_state.wizard =
                                            ui::connection::WizardState::None;
                                        self.load_remotes(tx.clone()).await;
                                    }
                                    Err(e) => {
                                        self.connection_state.error_message =
                                            Some(format!("Lỗi khi đổi tên remote: {}", e));
                                    }
                                }
                            } else {
                                let rclone_param = json!({
                                    "name": remote_name,
                                    "parameters": params,
                                    "obscure": true
                                })
                                .to_string();

                                let rpc_res = rclone::rpc("config/update", &rclone_param);
                                match rpc_res {
                                    Ok(_) => {
                                        self.connection_state.info_message = Some(format!(
                                            "Đã cập nhật remote '{}' thành công!",
                                            remote_name
                                        ));
                                        self.connection_state.wizard =
                                            ui::connection::WizardState::None;
                                        self.load_remotes(tx.clone()).await;
                                    }
                                    Err(e) => {
                                        self.connection_state.error_message =
                                            Some(format!("Lỗi khi cập nhật remote: {}", e));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            ui::connection::WizardState::ShowFeatures { .. } => {
                match key.code {
                    KeyCode::Esc | KeyCode::Enter => {
                        self.connection_state.wizard = ui::connection::WizardState::None;
                    }
                    _ => {}
                }
            }
            ui::connection::WizardState::SelectOneChoice {
                provider,
                remote_name,
                mut fields,
                selected_field_idx,
                scroll_offset,
                active_tab,
                selected_providers,
                is_edit_mode,
                field_name,
                choices,
                mut choices_selected_idx,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        if is_edit_mode {
                            self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                                remote_name,
                                provider,
                                fields,
                                selected_idx: selected_field_idx,
                                scroll_offset,
                                is_editing: false,
                                input_buffer: String::new(),
                                adding_new_key: false,
                                new_key_buffer: String::new(),
                                active_tab,
                            };
                        } else {
                            self.connection_state.wizard = ui::connection::WizardState::AdvancedSetup {
                                provider,
                                remote_name,
                                fields,
                                selected_field_idx,
                                scroll_offset,
                                is_editing: false,
                                input_buffer: String::new(),
                                selected_providers,
                                active_tab,
                            };
                        }
                    }
                    KeyCode::Up => {
                        if choices_selected_idx == 0 {
                            choices_selected_idx = choices.len() - 1;
                        } else {
                            choices_selected_idx -= 1;
                        }
                        self.connection_state.wizard = ui::connection::WizardState::SelectOneChoice {
                            provider,
                            remote_name,
                            fields,
                            selected_field_idx,
                            scroll_offset,
                            active_tab,
                            selected_providers,
                            is_edit_mode,
                            field_name,
                            choices,
                            choices_selected_idx,
                        };
                    }
                    KeyCode::Down => {
                        choices_selected_idx = (choices_selected_idx + 1) % choices.len();
                        self.connection_state.wizard = ui::connection::WizardState::SelectOneChoice {
                            provider,
                            remote_name,
                            fields,
                            selected_field_idx,
                            scroll_offset,
                            active_tab,
                            selected_providers,
                            is_edit_mode,
                            field_name,
                            choices,
                            choices_selected_idx,
                        };
                    }
                    KeyCode::Enter => {
                        let (selected_val, _) = &choices[choices_selected_idx];
                        if let Some(real_idx) = fields.iter().position(|f| f.0 == field_name) {
                            fields[real_idx].2 = selected_val.clone();
                        }
                        if is_edit_mode {
                            self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                                remote_name,
                                provider,
                                fields,
                                selected_idx: selected_field_idx,
                                scroll_offset,
                                is_editing: false,
                                input_buffer: String::new(),
                                adding_new_key: false,
                                new_key_buffer: String::new(),
                                active_tab,
                            };
                        } else {
                            self.connection_state.wizard = ui::connection::WizardState::AdvancedSetup {
                                provider,
                                remote_name,
                                fields,
                                selected_field_idx,
                                scroll_offset,
                                is_editing: false,
                                input_buffer: String::new(),
                                selected_providers,
                                active_tab,
                            };
                        }
                    }
                    _ => {}
                }
            }
            ui::connection::WizardState::SelectMultipleChoices {
                provider,
                remote_name,
                mut fields,
                selected_field_idx,
                scroll_offset,
                active_tab,
                selected_providers,
                is_edit_mode,
                field_name,
                mut options,
                mut choices_selected_idx,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        if is_edit_mode {
                            self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                                remote_name,
                                provider,
                                fields,
                                selected_idx: selected_field_idx,
                                scroll_offset,
                                is_editing: false,
                                input_buffer: String::new(),
                                adding_new_key: false,
                                new_key_buffer: String::new(),
                                active_tab,
                            };
                        } else {
                            self.connection_state.wizard = ui::connection::WizardState::AdvancedSetup {
                                provider,
                                remote_name,
                                fields,
                                selected_field_idx,
                                scroll_offset,
                                is_editing: false,
                                input_buffer: String::new(),
                                selected_providers,
                                active_tab,
                            };
                        }
                    }
                    KeyCode::Up => {
                        if choices_selected_idx == 0 {
                            choices_selected_idx = options.len().saturating_sub(1);
                        } else {
                            choices_selected_idx -= 1;
                        }
                        self.connection_state.wizard = ui::connection::WizardState::SelectMultipleChoices {
                            provider,
                            remote_name,
                            fields,
                            selected_field_idx,
                            scroll_offset,
                            active_tab,
                            selected_providers,
                            is_edit_mode,
                            field_name,
                            options,
                            choices_selected_idx,
                        };
                    }
                    KeyCode::Down => {
                        if !options.is_empty() {
                            choices_selected_idx = (choices_selected_idx + 1) % options.len();
                        }
                        self.connection_state.wizard = ui::connection::WizardState::SelectMultipleChoices {
                            provider,
                            remote_name,
                            fields,
                            selected_field_idx,
                            scroll_offset,
                            active_tab,
                            selected_providers,
                            is_edit_mode,
                            field_name,
                            options,
                            choices_selected_idx,
                        };
                    }
                    KeyCode::Char(' ') => {
                        if choices_selected_idx < options.len() {
                            options[choices_selected_idx].1 = !options[choices_selected_idx].1;
                        }
                        self.connection_state.wizard = ui::connection::WizardState::SelectMultipleChoices {
                            provider,
                            remote_name,
                            fields,
                            selected_field_idx,
                            scroll_offset,
                            active_tab,
                            selected_providers,
                            is_edit_mode,
                            field_name,
                            options,
                            choices_selected_idx,
                        };
                    }
                    KeyCode::Enter => {
                        let selected_vals: Vec<String> = options
                            .iter()
                            .filter(|(_, checked)| *checked)
                            .map(|(val, _)| val.clone())
                            .collect();
                        let new_val = selected_vals.join(" ");

                        if let Some(real_idx) = fields.iter().position(|f| f.0 == field_name) {
                            fields[real_idx].2 = new_val;
                        }

                        if is_edit_mode {
                            self.connection_state.wizard = ui::connection::WizardState::EditSetup {
                                remote_name,
                                provider,
                                fields,
                                selected_idx: selected_field_idx,
                                scroll_offset,
                                is_editing: false,
                                input_buffer: String::new(),
                                adding_new_key: false,
                                new_key_buffer: String::new(),
                                active_tab,
                            };
                        } else {
                            self.connection_state.wizard = ui::connection::WizardState::AdvancedSetup {
                                provider,
                                remote_name,
                                fields,
                                selected_field_idx,
                                scroll_offset,
                                is_editing: false,
                                input_buffer: String::new(),
                                selected_providers,
                                active_tab,
                            };
                        }
                    }
                    _ => {}
                }
            }
            ui::connection::WizardState::ImportConfigInput { mut input_buffer } => {
                match key.code {
                    KeyCode::Esc => {
                        self.connection_state.wizard = ui::connection::WizardState::None;
                    }
                    KeyCode::Char(c) => {
                        input_buffer.push(c);
                        self.connection_state.wizard = ui::connection::WizardState::ImportConfigInput { input_buffer };
                    }
                    KeyCode::Backspace => {
                        input_buffer.pop();
                        self.connection_state.wizard = ui::connection::WizardState::ImportConfigInput { input_buffer };
                    }
                    KeyCode::Enter => {
                        let path = input_buffer.trim().to_string();
                        if !path.is_empty() {
                            self.execute_import_config_tui(path, tx.clone()).await;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

impl App {
    pub(crate) async fn execute_import_config_tui(
        &mut self,
        path: String,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let path_buf = std::path::PathBuf::from(&path);
        if !path_buf.exists() {
            self.connection_state.error_message = Some("Tệp cấu hình không tồn tại hoặc đường dẫn không đúng.".to_string());
            self.connection_state.wizard = ui::connection::WizardState::None;
            return;
        }

        let content = match std::fs::read_to_string(&path_buf) {
            Ok(c) => c,
            Err(e) => {
                self.connection_state.error_message = Some(format!("Không thể đọc tệp cấu hình: {}", e));
                self.connection_state.wizard = ui::connection::WizardState::None;
                return;
            }
        };

        let remotes_to_import = parse_rclone_conf_tui(&content);
        if remotes_to_import.is_empty() {
            self.connection_state.error_message = Some("Không tìm thấy cấu hình remote hợp lệ nào trong tệp.".to_string());
            self.connection_state.wizard = ui::connection::WizardState::None;
            return;
        }

        let mut success_count = 0;
        let mut imported_details = Vec::new();
        let mut error_messages = Vec::new();

        let current_remotes = self.connection_state.remotes.clone();

        for (name, remote) in remotes_to_import {
            let mut final_name = name.clone();
            let mut counter = 1;
            while current_remotes.contains(&final_name) || self.connection_state.remotes.contains(&final_name) {
                final_name = format!("{}_{}", name, counter);
                counter += 1;
            }

            let create_param = json!({
                "name": final_name,
                "type": remote.remote_type,
                "parameters": Value::Object(remote.parameters),
            }).to_string();

            match rclone::rpc_async("config/create".to_string(), create_param).await {
                Ok(res) => {
                    if res.status == 200 {
                        success_count += 1;
                        if final_name != name {
                            imported_details.push(format!("{} -> {}", name, final_name));
                        } else {
                            imported_details.push(final_name);
                        }
                    } else {
                        let err_msg = serde_json::from_str::<Value>(&res.output)
                            .ok()
                            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(|s| s.to_string()))
                            .unwrap_or_else(|| format!("Lỗi RPC {}", res.status));
                        error_messages.push(format!("Remote '{}': {}", name, err_msg));
                    }
                }
                Err(e) => {
                    error_messages.push(format!("Remote '{}': {}", name, e));
                }
            }
        }

        self.load_remotes(tx).await;

        let mut msg = format!("Đã nhập thành công {} cấu hình remote.\n", success_count);
        if !imported_details.is_empty() {
            msg.push_str(&format!("Các remote đã nhập: {}\n", imported_details.join(", ")));
        }
        if !error_messages.is_empty() {
            msg.push_str(&format!("\nCác lỗi xảy ra:\n{}", error_messages.join("\n")));
            self.connection_state.error_message = Some(msg);
        } else {
            self.connection_state.info_message = Some(msg);
        }
        self.connection_state.wizard = ui::connection::WizardState::None;
    }
}

struct RemoteConfigTui {
    remote_type: String,
    parameters: serde_json::Map<String, Value>,
}

fn parse_rclone_conf_tui(content: &str) -> HashMap<String, RemoteConfigTui> {
    let mut remotes = HashMap::new();
    let mut current_section: Option<String> = None;
    let mut current_type: Option<String> = None;
    let mut current_params = serde_json::Map::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            if let Some(sec_name) = current_section.take() {
                if let Some(r_type) = current_type.take() {
                    remotes.insert(sec_name, RemoteConfigTui {
                        remote_type: r_type,
                        parameters: current_params,
                    });
                }
                current_params = serde_json::Map::new();
            }
            let sec_name = &line[1..line.len() - 1];
            current_section = Some(sec_name.trim().to_string());
        } else if let Some(ref _sec) = current_section {
            if let Some(pos) = line.find('=') {
                let key = line[..pos].trim().to_string();
                let val = line[pos + 1..].trim().to_string();
                if key == "type" {
                    current_type = Some(val);
                } else {
                    current_params.insert(key, Value::String(val));
                }
            }
        }
    }

    if let Some(sec_name) = current_section {
        if let Some(r_type) = current_type {
            remotes.insert(sec_name, RemoteConfigTui {
                remote_type: r_type,
                parameters: current_params,
            });
        }
    }

    remotes
}

fn try_get_filen_api_key() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let filen_path = std::path::Path::new(&home).join(".filen-cli/bin/filen");
    if !filen_path.exists() {
        return None;
    }
    
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("echo 'y' | '{}' export-api-key", filen_path.to_string_lossy()))
        .output()
        .ok()?;
        
    if !output.status.success() {
        return None;
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(pos) = line.find("API Key for ") {
            if let Some(colon_pos) = line[pos..].find(':') {
                let actual_colon_pos = pos + colon_pos;
                let key = line[actual_colon_pos + 1..].trim().to_string();
                if !key.is_empty() {
                    return Some(key);
                }
            }
        }
    }
    None
}

fn validate_required_fields(
    fields: &[(String, String, String, Vec<(String, String)>, bool)],
) -> Result<(), Vec<String>> {
    let mut missing_fields = Vec::new();
    for (name, _, val, _, _) in fields.iter() {
        let is_req = ui::connection::is_field_required(name, fields, 9999, false, "");
        if is_req && val.trim().is_empty() {
            let (friendly_name, _) = ui::connection::translate_field(name, "");
            missing_fields.push(friendly_name);
        }
    }
    if missing_fields.is_empty() {
        Ok(())
    } else {
        Err(missing_fields)
    }
}

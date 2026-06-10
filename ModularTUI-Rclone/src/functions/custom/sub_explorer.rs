use crate::app::{App, AppEvent};
use crate::functions::*;
use serde_json::{Value, json};

pub fn refresh_tui_selector_list(
    app: &mut App,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
) {
    if let ExplorerPopup::TuiExplorerSelector {
        ref mut loading,
        ref remote,
        ref path,
        ..
    } = app.explorer_state.popup
    {
        *loading = true;
        let remote = remote.clone();
        let path = path.clone();
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            if remote.is_empty() && path.is_empty() {
                let res = rpc_async("config/listremotes".to_string(), "{}".to_string()).await;
                match res {
                    Ok(rpc_res) => {
                        if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                            if let Some(remotes_arr) = val.get("remotes").and_then(|r| r.as_array()) {
                                let mut items = vec![FileItem {
                                    name: "[Local System]".to_string(),
                                    size: 0,
                                    is_dir: true,
                                    mod_time: "---".to_string(),
                                    id: None,
                                }];
                                for r_val in remotes_arr {
                                    if let Some(r_str) = r_val.as_str() {
                                        items.push(FileItem {
                                            name: r_str.to_string(),
                                            size: 0,
                                            is_dir: true,
                                            mod_time: "---".to_string(),
                                            id: None,
                                        });
                                    }
                                }
                                let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                    result: Ok(items),
                                });
                                return;
                            }
                        }
                        let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                            result: Err("Không thể phân tích danh sách remote".to_string()),
                        });
                    }
                    Err(e) => {
                        let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                            result: Err(e),
                        });
                    }
                }
            } else {
                let fs_target = if remote.is_empty() {
                    path.clone()
                } else {
                    let clean_remote = remote.trim_end_matches(':');
                    let clean_path = if path.starts_with('/') {
                        path.clone()
                    } else {
                        format!("/{}", path)
                    };
                    format!("{}:{}", clean_remote, clean_path)
                };

                let input_param = json!({
                    "fs": fs_target,
                    "remote": "",
                })
                .to_string();

                let list_future = rpc_async("operations/list".to_string(), input_param);
                let res = match tokio::time::timeout(std::time::Duration::from_secs(15), list_future).await {
                    Ok(inner_res) => inner_res,
                    Err(_) => Err("Hết thời gian chờ phản hồi từ Cloud (Timeout)".to_string()),
                };

                match res {
                    Ok(rpc_res) => {
                        if rpc_res.status == 200 {
                            if let Ok(val) = serde_json::from_str::<Value>(&rpc_res.output) {
                                if let Some(err_str) = val.get("error").and_then(|e| e.as_str()) {
                                    let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                        result: Err(err_str.to_string()),
                                    });
                                } else if let Some(list_arr) = val.get("list").and_then(|l| l.as_array()) {
                                    let mut items = Vec::new();
                                    for item_val in list_arr {
                                        let name = item_val
                                            .get("Name")
                                            .and_then(|n| n.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let size =
                                            item_val.get("Size").and_then(|s| s.as_u64()).unwrap_or(0);
                                        let is_dir = item_val
                                            .get("IsDir")
                                            .and_then(|d| d.as_bool())
                                            .unwrap_or(false);
                                        let mod_time = item_val
                                            .get("ModTime")
                                            .and_then(|m| m.as_str())
                                            .unwrap_or("")
                                            .to_string();

                                        let id = item_val
                                            .get("ID")
                                            .and_then(|i| i.as_str())
                                            .map(|s| s.to_string());

                                        let cleaned_time = mod_time
                                            .chars()
                                            .take(19)
                                            .collect::<String>()
                                            .replace("T", " ");

                                        if is_dir {
                                            items.push(FileItem {
                                                name,
                                                size,
                                                is_dir: true,
                                                mod_time: cleaned_time,
                                                id,
                                            });
                                        }
                                    }
                                    items.sort_by(|a, b| a.name.cmp(&b.name));

                                    let at_root = if remote.is_empty() {
                                        path == "/" || path.is_empty()
                                    } else {
                                        path.is_empty()
                                    };

                                    if !at_root {
                                        items.insert(
                                            0,
                                            FileItem {
                                                name: "..".to_string(),
                                                size: 0,
                                                is_dir: true,
                                                mod_time: "---".to_string(),
                                                id: None,
                                            },
                                        );
                                    }

                                    let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                        result: Ok(items),
                                    });
                                }
                            }
                        } else {
                            let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                                result: Err(rpc_res.output),
                            });
                        }
                    }
                    Err(e) => {
                        let _ = tx_clone.send(AppEvent::TuiSelectorListResult {
                            result: Err(e),
                        });
                    }
                }
            }
        });
    }
}

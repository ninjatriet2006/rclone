use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::Notify;

#[path = "../rclone.rs"]
mod rclone;

struct ScanState {
    queue: Vec<String>,
    active_tasks: usize,
    files: Vec<String>,
    restricted_files: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rclone::initialize();
    
    // Enable Metadata option globally
    let set_param = json!({
        "main": {
            "Metadata": true
        }
    }).to_string();
    let _ = rclone::rpc_async("options/set".to_string(), set_param).await;
    
    let folder_fs = "Main,root_folder_id=1PhGrE7vEXU_BGdHJORguECYU5Z1ISoss:/STM32 ".to_string();
    println!("=== STARTING CONCURRENT SCAN OF {} ===", folder_fs);
    
    let start_time = Instant::now();
    
    let state = Arc::new(Mutex::new(ScanState {
        queue: vec!["".to_string()],
        active_tasks: 0,
        files: Vec::new(),
        restricted_files: Vec::new(),
    }));
    
    let notify = Arc::new(Notify::new());
    let max_concurrency = 8;
    
    loop {
        let mut to_spawn = 0;
        let mut finished = false;
        
        {
            let s = state.lock().unwrap();
            if s.queue.is_empty() && s.active_tasks == 0 {
                finished = true;
            } else {
                let available_slots = max_concurrency - s.active_tasks;
                to_spawn = available_slots.min(s.queue.len());
            }
        }
        
        if finished {
            break;
        }
        
        if to_spawn == 0 {
            // Wait for a task to finish and wake us up
            notify.notified().await;
            continue;
        }
        
        for _ in 0..to_spawn {
            let dir = {
                let mut s = state.lock().unwrap();
                s.active_tasks += 1;
                s.queue.remove(0)
            };
            
            let state_clone = Arc::clone(&state);
            let notify_clone = Arc::clone(&notify);
            let folder_fs_clone = folder_fs.clone();
            
            tokio::spawn(async move {
                let list_param = json!({
                    "fs": folder_fs_clone,
                    "remote": dir,
                    "opt": {
                        "recurse": false,
                        "metadata": true
                    }
                }).to_string();
                
                let mut new_dirs = Vec::new();
                let mut new_files = Vec::new();
                let mut new_restricted = Vec::new();
                
                if let Ok(res) = rclone::rpc_async("operations/list".to_string(), list_param).await {
                    if res.status == 200 {
                        if let Ok(val) = serde_json::from_str::<Value>(&res.output) {
                            if let Some(list_arr) = val.get("list").and_then(|l| l.as_array()) {
                                for item in list_arr {
                                    let is_dir = item.get("IsDir").and_then(|d| d.as_bool()).unwrap_or(false);
                                    let path = item.get("Path").and_then(|p| p.as_str()).unwrap_or("");
                                    
                                    if dir.is_empty() {
                                        println!("Top-level item found: name={:?}, path={:?}", item.get("Name"), path);
                                    }
                                    
                                    if is_dir {
                                        new_dirs.push(path.to_string());
                                    } else {
                                        new_files.push(path.to_string());
                                        let is_restricted = if let Some(meta) = item.get("Metadata") {
                                            meta.get("copy-requires-writer-permission")
                                                .and_then(|v| v.as_str())
                                                == Some("true")
                                        } else {
                                            false
                                        };
                                        let mime_type = item.get("MimeType").and_then(|m| m.as_str()).unwrap_or("");
                                        let is_dangling = mime_type.contains("shortcut.dangling");
                                        
                                        if is_restricted || is_dangling {
                                            new_restricted.push(path.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                {
                    let mut s = state_clone.lock().unwrap();
                    s.queue.extend(new_dirs);
                    s.files.extend(new_files);
                    s.restricted_files.extend(new_restricted);
                    s.active_tasks -= 1;
                    
                    println!("Progress: Scanned {} files, {} restricted", s.files.len(), s.restricted_files.len());
                }
                
                notify_clone.notify_one();
            });
        }
    }
    
    let duration = start_time.elapsed();
    let final_state = state.lock().unwrap();
    println!("=== SCAN FINISHED ===");
    println!("Total files found: {}", final_state.files.len());
    println!("Total restricted files: {}", final_state.restricted_files.len());
    println!("Scanning took: {:?}", duration);
    
    rclone::finalize();
    Ok(())
}

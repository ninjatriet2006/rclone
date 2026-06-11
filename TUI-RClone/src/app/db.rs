use std::path::PathBuf;
use rusqlite::{Connection, params};
use super::{ActiveOperation, FileTask, TaskStatus};

fn get_db_path() -> PathBuf {
    crate::app_config::AppConfig::config_dir().join("active_ops.db")
}

fn get_connection() -> Result<Connection, rusqlite::Error> {
    let db_path = get_db_path();
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    Connection::open(db_path)
}

pub fn init_db() -> Result<(), rusqlite::Error> {
    let conn = get_connection()?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS operations (
            id TEXT PRIMARY KEY,
            action_type TEXT NOT NULL,
            src TEXT NOT NULL,
            dest TEXT NOT NULL,
            is_dir INTEGER NOT NULL,
            use_checksum INTEGER NOT NULL,
            is_copy INTEGER NOT NULL,
            items TEXT NOT NULL,
            completed_items TEXT,
            transfers INTEGER,
            checkers INTEGER
        )",
        [],
    )?;
    // Hỗ trợ cập nhật cơ sở dữ liệu cũ:
    let _ = conn.execute("ALTER TABLE operations ADD COLUMN transfers INTEGER", []);
    let _ = conn.execute("ALTER TABLE operations ADD COLUMN checkers INTEGER", []);
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tasks (
            op_id TEXT NOT NULL,
            name TEXT NOT NULL,
            size INTEGER NOT NULL,
            status TEXT NOT NULL,
            error TEXT,
            PRIMARY KEY (op_id, name),
            FOREIGN KEY (op_id) REFERENCES operations (id) ON DELETE CASCADE
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tasks_op_id_name ON tasks(op_id, name)",
        [],
    )?;
    Ok(())
}

pub fn save_active_operation(op: &ActiveOperation) -> Result<(), rusqlite::Error> {
    let mut conn = get_connection()?;
    let tx = conn.transaction()?;

    let items_json = serde_json::to_string(&op.items).unwrap_or_else(|_| "[]".to_string());
    let completed_items_json = op.completed_items.as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string()))
        .unwrap_or_else(|| "[]".to_string());

    tx.execute(
        "INSERT OR REPLACE INTO operations (id, action_type, src, dest, is_dir, use_checksum, is_copy, items, completed_items, transfers, checkers)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            op.id,
            op.action_type,
            op.src,
            op.dest,
            if op.is_dir { 1 } else { 0 },
            if op.use_checksum { 1 } else { 0 },
            if op.is_copy { 1 } else { 0 },
            items_json,
            completed_items_json,
            op.transfers,
            op.checkers,
        ],
    )?;

    if let Some(ref tasks) = op.tasks {
        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO tasks (op_id, name, size, status, error) VALUES (?1, ?2, ?3, ?4, ?5)"
        )?;
        for task in tasks {
            let status_str = match task.status {
                TaskStatus::Pending => "Pending",
                TaskStatus::Transferring => "Transferring",
                TaskStatus::Completed => "Completed",
                TaskStatus::Failed => "Failed",
                TaskStatus::Skipped => "Skipped",
            };
            stmt.execute(params![
                op.id,
                task.name,
                task.size,
                status_str,
                task.error,
            ])?;
        }
    }

    tx.commit()?;
    Ok(())
}

pub fn complete_item_in_active_operation(id: &str, item_name: &str) -> Result<(), rusqlite::Error> {
    let mut conn = get_connection()?;
    let tx = conn.transaction()?;

    // Cập nhật trạng thái task trong SQLite
    tx.execute(
        "UPDATE tasks SET status = 'Completed' WHERE op_id = ?1 AND name = ?2",
        params![id, item_name],
    )?;

    // Load, chỉnh sửa và lưu lại danh sách items/completed_items của operation
    let mut op_row: Option<(String, String)> = None;
    {
        let mut stmt = tx.prepare("SELECT items, completed_items FROM operations WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let items: String = row.get(0)?;
            let completed: String = row.get(1)?;
            op_row = Some((items, completed));
        }
    }

    if let Some((items_str, completed_str)) = op_row {
        let mut items: Vec<String> = serde_json::from_str(&items_str).unwrap_or_default();
        let mut completed: Vec<String> = serde_json::from_str(&completed_str).unwrap_or_default();

        let mut modified = false;
        if let Some(pos) = items.iter().position(|x| x == item_name) {
            items.remove(pos);
            if !completed.contains(&item_name.to_string()) {
                completed.push(item_name.to_string());
            }
            modified = true;
        }

        if modified {
            let items_json = serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string());
            let completed_json = serde_json::to_string(&completed).unwrap_or_else(|_| "[]".to_string());
            tx.execute(
                "UPDATE operations SET items = ?1, completed_items = ?2 WHERE id = ?3",
                params![items_json, completed_json, id],
            )?;
        }
    }

    tx.commit()?;
    Ok(())
}

pub fn complete_items_in_active_operation(id: &str, item_names: &[String]) -> Result<(), rusqlite::Error> {
    let mut conn = get_connection()?;
    let tx = conn.transaction()?;

    {
        let mut stmt = tx.prepare("UPDATE tasks SET status = 'Completed' WHERE op_id = ?1 AND name = ?2")?;
        for name in item_names {
            stmt.execute(params![id, name])?;
        }
    }

    let mut op_row: Option<(String, String)> = None;
    {
        let mut stmt = tx.prepare("SELECT items, completed_items FROM operations WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let items: String = row.get(0)?;
            let completed: String = row.get(1)?;
            op_row = Some((items, completed));
        }
    }

    if let Some((items_str, completed_str)) = op_row {
        let mut items: Vec<String> = serde_json::from_str(&items_str).unwrap_or_default();
        let mut completed: Vec<String> = serde_json::from_str(&completed_str).unwrap_or_default();

        let mut modified = false;
        for item_name in item_names {
            if let Some(pos) = items.iter().position(|x| x == item_name) {
                items.remove(pos);
                if !completed.contains(item_name) {
                    completed.push(item_name.clone());
                }
                modified = true;
            }
        }

        if modified {
            let items_json = serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string());
            let completed_json = serde_json::to_string(&completed).unwrap_or_else(|_| "[]".to_string());
            tx.execute(
                "UPDATE operations SET items = ?1, completed_items = ?2 WHERE id = ?3",
                params![items_json, completed_json, id],
            )?;
        }
    }

    tx.commit()?;
    Ok(())
}

pub fn update_tasks_status_in_active_operation(
    id: &str,
    item_names: &[String],
    status: TaskStatus,
    error: Option<String>
) -> Result<(), rusqlite::Error> {
    let mut conn = get_connection()?;
    let tx = conn.transaction()?;

    let status_str = match status {
        TaskStatus::Pending => "Pending",
        TaskStatus::Transferring => "Transferring",
        TaskStatus::Completed => "Completed",
        TaskStatus::Failed => "Failed",
        TaskStatus::Skipped => "Skipped",
    };

    {
        let mut stmt = tx.prepare("UPDATE tasks SET status = ?1, error = ?2 WHERE op_id = ?3 AND name = ?4")?;
        for name in item_names {
            stmt.execute(params![status_str, error, id, name])?;
        }
    }

    if status == TaskStatus::Completed {
        let mut op_row: Option<(String, String)> = None;
        {
            let mut stmt = tx.prepare("SELECT items, completed_items FROM operations WHERE id = ?1")?;
            let mut rows = stmt.query(params![id])?;
            if let Some(row) = rows.next()? {
                let items: String = row.get(0)?;
                let completed: String = row.get(1)?;
                op_row = Some((items, completed));
            }
        }

        if let Some((items_str, completed_str)) = op_row {
            let mut items: Vec<String> = serde_json::from_str(&items_str).unwrap_or_default();
            let mut completed: Vec<String> = serde_json::from_str(&completed_str).unwrap_or_default();

            let mut modified = false;
            for item_name in item_names {
                if let Some(pos) = items.iter().position(|x| x == item_name) {
                    items.remove(pos);
                    if !completed.contains(item_name) {
                        completed.push(item_name.clone());
                    }
                    modified = true;
                }
            }

            if modified {
                let items_json = serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string());
                let completed_json = serde_json::to_string(&completed).unwrap_or_else(|_| "[]".to_string());
                tx.execute(
                    "UPDATE operations SET items = ?1, completed_items = ?2 WHERE id = ?3",
                    params![items_json, completed_json, id],
                )?;
            }
        }
    }

    tx.commit()?;
    Ok(())
}

pub fn update_tasks_individual_status_in_active_operation(
    id: &str,
    updates: &[(&str, TaskStatus, Option<String>)]
) -> Result<(), rusqlite::Error> {
    let mut conn = get_connection()?;
    let tx = conn.transaction()?;

    let mut completed_items = Vec::new();

    {
        let mut stmt = tx.prepare("UPDATE tasks SET status = ?1, error = ?2 WHERE op_id = ?3 AND name = ?4")?;
        for &(name, ref status, ref error) in updates {
            let status_str = match status {
                TaskStatus::Pending => "Pending",
                TaskStatus::Transferring => "Transferring",
                TaskStatus::Completed => "Completed",
                TaskStatus::Failed => "Failed",
                TaskStatus::Skipped => "Skipped",
            };
            stmt.execute(params![status_str, error, id, name])?;
            if *status == TaskStatus::Completed {
                completed_items.push(name.to_string());
            }
        }
    }

    if !completed_items.is_empty() {
        let mut op_row: Option<(String, String)> = None;
        {
            let mut stmt = tx.prepare("SELECT items, completed_items FROM operations WHERE id = ?1")?;
            let mut rows = stmt.query(params![id])?;
            if let Some(row) = rows.next()? {
                let items: String = row.get(0)?;
                let completed: String = row.get(1)?;
                op_row = Some((items, completed));
            }
        }

        if let Some((items_str, completed_str)) = op_row {
            let mut items: Vec<String> = serde_json::from_str(&items_str).unwrap_or_default();
            let mut completed: Vec<String> = serde_json::from_str(&completed_str).unwrap_or_default();

            let mut modified = false;
            for item_name in completed_items {
                if let Some(pos) = items.iter().position(|x| x == &item_name) {
                    items.remove(pos);
                    if !completed.contains(&item_name) {
                        completed.push(item_name);
                    }
                    modified = true;
                }
            }

            if modified {
                let items_json = serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string());
                let completed_json = serde_json::to_string(&completed).unwrap_or_else(|_| "[]".to_string());
                tx.execute(
                    "UPDATE operations SET items = ?1, completed_items = ?2 WHERE id = ?3",
                    params![items_json, completed_json, id],
                )?;
            }
        }
    }

    tx.commit()?;
    Ok(())
}

pub fn append_tasks_to_active_operation(id: &str, new_tasks: &[FileTask]) -> Result<(), rusqlite::Error> {
    let mut conn = get_connection()?;
    let tx = conn.transaction()?;

    // Đầu tiên, chèn/ghi đè các tasks vào DB
    {
        let mut stmt = tx.prepare(
            "INSERT OR IGNORE INTO tasks (op_id, name, size, status, error) VALUES (?1, ?2, ?3, ?4, ?5)"
        )?;
        for task in new_tasks {
            let status_str = match task.status {
                TaskStatus::Pending => "Pending",
                TaskStatus::Transferring => "Transferring",
                TaskStatus::Completed => "Completed",
                TaskStatus::Failed => "Failed",
                TaskStatus::Skipped => "Skipped",
            };
            stmt.execute(params![
                id,
                task.name,
                task.size,
                status_str,
                task.error,
            ])?;
        }
    }

    // Tiếp theo, cập nhật danh sách items và completed_items của operation tương ứng
    let mut op_row: Option<(String, String)> = None;
    {
        let mut stmt = tx.prepare("SELECT items, completed_items FROM operations WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let items: String = row.get(0)?;
            let completed: String = row.get(1)?;
            op_row = Some((items, completed));
        }
    }

    if let Some((items_str, completed_str)) = op_row {
        let mut items: Vec<String> = serde_json::from_str(&items_str).unwrap_or_default();
        let mut completed: Vec<String> = serde_json::from_str(&completed_str).unwrap_or_default();

        let mut modified = false;
        for task in new_tasks {
            if task.status == TaskStatus::Completed || task.status == TaskStatus::Skipped {
                if !completed.contains(&task.name) {
                    completed.push(task.name.clone());
                    // Xóa khỏi items nếu có
                    if let Some(pos) = items.iter().position(|x| x == &task.name) {
                        items.remove(pos);
                    }
                    modified = true;
                }
            } else {
                if !items.contains(&task.name) {
                    items.push(task.name.clone());
                    // Xóa khỏi completed nếu có (phòng hờ reset trạng thái)
                    if let Some(pos) = completed.iter().position(|x| x == &task.name) {
                        completed.remove(pos);
                    }
                    modified = true;
                }
            }
        }

        if modified {
            let items_json = serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string());
            let completed_json = serde_json::to_string(&completed).unwrap_or_else(|_| "[]".to_string());
            tx.execute(
                "UPDATE operations SET items = ?1, completed_items = ?2 WHERE id = ?3",
                params![items_json, completed_json, id],
            )?;
        }
    }

    tx.commit()?;
    Ok(())
}

pub fn prepare_active_operation_for_resume(id: &str) -> Result<(), rusqlite::Error> {
    let conn = get_connection()?;
    conn.execute(
        "UPDATE tasks SET status = 'Pending', error = NULL 
         WHERE op_id = ?1 AND (status = 'Transferring' OR status = 'Failed')",
        params![id],
    )?;
    Ok(())
}

pub fn remove_active_operation(id: &str) -> Result<(), rusqlite::Error> {
    let conn = get_connection()?;
    // Do CASCADE delete được cấu hình, xoá operation sẽ tự xoá tasks liên quan
    conn.execute("DELETE FROM operations WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn load_active_operations() -> Result<Vec<ActiveOperation>, rusqlite::Error> {
    let conn = get_connection()?;
    
    // Đọc tất cả operations
    let mut stmt_ops = conn.prepare(
        "SELECT id, action_type, src, dest, is_dir, use_checksum, is_copy, items, completed_items, transfers, checkers FROM operations"
    )?;
    let mut rows_ops = stmt_ops.query([])?;
    
    let mut ops = Vec::new();
    while let Some(row) = rows_ops.next()? {
        let op_id: String = row.get(0)?;
        let action_type: String = row.get(1)?;
        let src: String = row.get(2)?;
        let dest: String = row.get(3)?;
        let is_dir: i32 = row.get(4)?;
        let use_checksum: i32 = row.get(5)?;
        let is_copy: i32 = row.get(6)?;
        let items_str: String = row.get(7)?;
        let completed_str: String = row.get(8)?;
        let transfers: Option<u64> = row.get(9)?;
        let checkers: Option<u64> = row.get(10)?;

        let items: Vec<String> = serde_json::from_str(&items_str).unwrap_or_default();
        let completed_items: Option<Vec<String>> = Some(serde_json::from_str(&completed_str).unwrap_or_default());

        // Đọc các tasks của operation này
        let mut stmt_tasks = conn.prepare(
            "SELECT name, size, status, error FROM tasks WHERE op_id = ?1"
        )?;
        let mut rows_tasks = stmt_tasks.query(params![op_id])?;
        
        let mut tasks = Vec::new();
        while let Some(t_row) = rows_tasks.next()? {
            let t_name: String = t_row.get(0)?;
            let t_size: u64 = t_row.get(1)?;
            let t_status_str: String = t_row.get(2)?;
            let t_error: Option<String> = t_row.get(3)?;

            let t_status = match t_status_str.as_str() {
                "Transferring" => TaskStatus::Transferring,
                "Completed" => TaskStatus::Completed,
                "Failed" => TaskStatus::Failed,
                "Skipped" => TaskStatus::Skipped,
                _ => TaskStatus::Pending,
            };

            tasks.push(FileTask {
                name: t_name,
                size: t_size,
                status: t_status,
                error: t_error,
            });
        }

        let tasks_opt = if tasks.is_empty() {
            // Xem lúc đầu tasks là None hay Some(empty)?
            // Trong mod.rs ban đầu, một số op không có tasks (ví dụ delete, purge). 
            // Nếu ban đầu structure op.tasks là None, khi lưu ta không thêm task nào vào bảng tasks.
            // Do đó khi query, nếu không tìm thấy task nào, ta có thể trả về None hoặc Some(empty) dựa trên logic.
            // Thực tế: trong mod.rs, op.tasks có thể là None. 
            // Để phân biệt, nếu action_type là "delete" hoặc "purge" thì tasks thường là None.
            if action_type == "delete" || action_type == "purge" {
                None
            } else {
                Some(Vec::new())
            }
        } else {
            Some(tasks)
        };

        ops.push(ActiveOperation {
            id: op_id,
            action_type,
            src,
            dest,
            items,
            is_dir: is_dir != 0,
            use_checksum: use_checksum != 0,
            is_copy: is_copy != 0,
            completed_items,
            tasks: tasks_opt,
            transfers,
            checkers,
        });
    }

    Ok(ops)
}

pub fn clear_active_operations() -> Result<(), rusqlite::Error> {
    let conn = get_connection()?;
    conn.execute("DELETE FROM tasks", [])?;
    conn.execute("DELETE FROM operations", [])?;
    Ok(())
}

pub fn update_active_operation_threads(id: &str, transfers: u64, checkers: u64) -> Result<(), rusqlite::Error> {
    let conn = get_connection()?;
    conn.execute(
        "UPDATE operations SET transfers = ?1, checkers = ?2 WHERE id = ?3",
        params![transfers as i64, checkers as i64, id],
    )?;
    Ok(())
}

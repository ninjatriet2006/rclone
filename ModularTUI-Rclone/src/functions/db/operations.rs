use rusqlite::params;
use super::get_connection;
use crate::functions::{ActiveOperation, FileTask, TaskStatus};

pub fn save_active_operation(op: &ActiveOperation) -> Result<(), rusqlite::Error> {
    let mut conn = get_connection()?;
    let tx = conn.transaction()?;

    let items_json = serde_json::to_string(&op.items).unwrap_or_else(|_| "[]".to_string());
    let completed_items_json = op.completed_items.as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string()))
        .unwrap_or_else(|| "[]".to_string());

    tx.execute(
        "INSERT OR REPLACE INTO operations (id, action_type, src, dest, is_dir, use_checksum, is_copy, items, completed_items)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
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

    tx.execute(
        "UPDATE tasks SET status = 'Completed' WHERE op_id = ?1 AND name = ?2",
        params![id, item_name],
    )?;

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

pub fn remove_active_operation(id: &str) -> Result<(), rusqlite::Error> {
    let conn = get_connection()?;
    conn.execute("DELETE FROM operations WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn load_active_operations() -> Result<Vec<ActiveOperation>, rusqlite::Error> {
    let conn = get_connection()?;
    let mut stmt_ops = conn.prepare(
        "SELECT id, action_type, src, dest, is_dir, use_checksum, is_copy, items, completed_items FROM operations"
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

        let items: Vec<String> = serde_json::from_str(&items_str).unwrap_or_default();
        let completed_items: Option<Vec<String>> = Some(serde_json::from_str(&completed_str).unwrap_or_default());

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

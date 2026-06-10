use rusqlite::params;
use super::get_connection;
use crate::functions::{FileTask, TaskStatus};

pub fn update_task_status_in_active_operation(
    id: &str,
    item_name: &str,
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

    tx.execute(
        "UPDATE tasks SET status = ?1, error = ?2 WHERE op_id = ?3 AND name = ?4",
        params![status_str, error, id, item_name],
    )?;

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

pub fn append_tasks_to_active_operation(id: &str, new_tasks: &[FileTask]) -> Result<(), rusqlite::Error> {
    let mut conn = get_connection()?;
    let tx = conn.transaction()?;

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
                    if let Some(pos) = items.iter().position(|x| x == &task.name) {
                        items.remove(pos);
                    }
                    modified = true;
                }
            } else {
                if !items.contains(&task.name) {
                    items.push(task.name.clone());
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

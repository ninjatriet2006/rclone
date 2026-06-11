use super::get_connection;

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

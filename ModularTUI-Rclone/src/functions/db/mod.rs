use std::path::PathBuf;
use rusqlite::Connection;

pub mod init;
pub mod operations;
pub mod tasks;

pub use init::init_db;
pub use operations::{
    save_active_operation,
    complete_item_in_active_operation,
    complete_items_in_active_operation,
    remove_active_operation,
    load_active_operations,
    clear_active_operations,
};
pub use tasks::{
    update_task_status_in_active_operation,
    update_tasks_status_in_active_operation,
    append_tasks_to_active_operation,
    prepare_active_operation_for_resume,
};

pub fn get_db_path() -> PathBuf {
    crate::functions::AppConfig::config_dir().join("active_ops.db")
}

pub fn get_connection() -> Result<Connection, rusqlite::Error> {
    let db_path = get_db_path();
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    Connection::open(db_path)
}

pub mod log;
pub mod app_config;
pub mod lang;
pub mod rclone;
pub mod db;

pub mod fs;
pub mod daemon;
pub mod ui_helpers;
pub mod widgets;
pub mod keys;
pub mod custom;

pub use log::{log_info, log_rpc, log_info_start, log_info_end, initialize_logger, LogScope};
pub use app_config::{AppConfig, ExportResult, get_home_dir, get_default_rclone_conf, get_rclone_tui_conf};
pub use db::{
    init_db,
    save_active_operation,
    complete_item_in_active_operation,
    complete_items_in_active_operation,
    remove_active_operation,
    load_active_operations,
    clear_active_operations,
    update_task_status_in_active_operation,
    update_tasks_status_in_active_operation,
    append_tasks_to_active_operation,
    prepare_active_operation_for_resume,
};

pub use lang::{init_languages, load_translation, get_available_languages, translate, translate_desc, translate_tip};
pub use rclone::{initialize, finalize, SafeRpcResult, rpc, rpc_async, run_rpc_job_async, run_rpc_job_async_with_progress, register_job_description, get_job_description, get_underlying_remote, JobDirection, register_job_direction, get_job_direction, register_job_real_size, get_job_real_size, inject_optimal_thread_config, get_directory_stats, calculate_optimal_threads, calculate_optimal_threads_v2, check_and_apply_rate_limiting, get_remote_type};
pub use fs::{check_fuse_dependency, check_terminal_wrapping, parse_parent_and_child, copy_to_system_clipboard, parse_cmdline, get_rclone_cmd, join_fs_path, strip_archive_extensions};
pub use daemon::{detect_systemd_service, parse_rclone_args, scan_running_services, scan_systemd_services, kill_process_by_pid, kill_all_active_services};
pub use ui_helpers::{format_size, update_scroll_offset, calculate_scroll_range, centered_rect, draw_popup, parse_help_line, estimate_wrapped_lines, make_input_spans_with_cursor, format_display_name};

// We will add re-exports of widgets, keys, and custom modules inside their respective mod.rs files and link them here.
pub use widgets::*;
pub use keys::*;
pub use custom::*;

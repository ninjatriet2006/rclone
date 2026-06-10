pub mod mod_bindings;
pub mod structs;
pub mod rpc;
pub mod rpc_async;
pub mod run_rpc_job_async;
pub mod run_rpc_job_async_with_progress;
pub mod get_job_description;
pub mod get_underlying_remote;
pub mod job_direction;
pub mod thread_optimizer;

pub use mod_bindings::{initialize, finalize};
pub use structs::SafeRpcResult;
pub use rpc::rpc;
pub use rpc_async::rpc_async;
pub use run_rpc_job_async::run_rpc_job_async;
pub use run_rpc_job_async_with_progress::run_rpc_job_async_with_progress;
pub use get_job_description::{register_job_description, get_job_description};
pub use get_underlying_remote::get_underlying_remote;
pub use job_direction::{JobDirection, register_job_direction, get_job_direction, register_job_real_size, get_job_real_size};
pub use thread_optimizer::{get_directory_stats, calculate_optimal_threads, calculate_optimal_threads_v2, inject_optimal_thread_config, check_and_apply_rate_limiting, get_remote_type};



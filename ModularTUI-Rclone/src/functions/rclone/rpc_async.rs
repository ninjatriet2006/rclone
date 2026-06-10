use crate::functions::rclone::rpc::rpc;
use crate::functions::rclone::structs::SafeRpcResult;

pub async fn rpc_async(method: String, input_json: String) -> Result<SafeRpcResult, String> {
    // Để giữ nguyên thread-local INDENT_LEVEL khi gọi spawn_blocking, ta đọc nó và truyền vào thread mới.
    let indent = crate::functions::log::scope::INDENT_LEVEL.with(|l| *l.borrow());
    
    tokio::task::spawn_blocking(move || {
        // Khôi phục mức thụt lề cho thread chạy blocking
        crate::functions::log::scope::INDENT_LEVEL.with(|l| *l.borrow_mut() = indent);
        rpc(&method, &input_json)
    })
    .await
    .map_err(|e| e.to_string())?
}

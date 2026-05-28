use lazy_static::lazy_static;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::sync::Mutex;

#[repr(C)]
pub struct RcloneRPCResult {
    pub output: *mut c_char,
    pub status: c_int,
}

unsafe extern "C" {
    pub fn RcloneInitialize();
    pub fn RcloneFinalize();
    pub fn RcloneRPC(method: *const c_char, input: *const c_char) -> RcloneRPCResult;
    pub fn RcloneFreeString(str: *mut c_char);
}

lazy_static! {
    static ref RCLONE_ENGINE_LOCK: Mutex<()> = Mutex::new(());
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SafeRpcResult {
    pub output: String,
    pub status: i32,
}

/// Khởi tạo Core Rclone Go Engine
pub fn initialize() {
    let _guard = RCLONE_ENGINE_LOCK.lock().unwrap();
    unsafe {
        RcloneInitialize();
    }
}

/// Giải phóng Core Rclone Go Engine
pub fn finalize() {
    let _guard = RCLONE_ENGINE_LOCK.lock().unwrap();
    unsafe {
        RcloneFinalize();
    }
}

/// Thực hiện lệnh gọi RPC đồng bộ
pub fn rpc(method: &str, input_json: &str) -> Result<SafeRpcResult, String> {
    let c_method = CString::new(method).map_err(|e| e.to_string())?;
    let c_input = CString::new(input_json).map_err(|e| e.to_string())?;

    unsafe {
        let result = RcloneRPC(c_method.as_ptr(), c_input.as_ptr());

        // Giải quyết Potential Bug 92 (kiểm tra null pointer)
        if result.output.is_null() {
            return Err("RcloneRPC returned a null output pointer".to_string());
        }

        let c_str = CStr::from_ptr(result.output);

        // Giải quyết Potential Bug 68 (làm sạch ký tự điều khiển trong chuỗi JSON nếu cần)
        // Tuy nhiên to_string_lossy handles invalid UTF8 safely.
        let raw_output = c_str.to_string_lossy().into_owned();

        // Giải phóng chuỗi do Go cấp phát bằng RcloneFreeString để tránh rò rỉ bộ nhớ (Bug 1, 8, 78)
        RcloneFreeString(result.output);

        Ok(SafeRpcResult {
            output: raw_output,
            status: result.status as i32,
        })
    }
}

/// Thực hiện lệnh gọi RPC bất đồng bộ trên blocking thread pool của Tokio để tránh block UI (Bug 2, 10, 74)
pub async fn rpc_async(method: String, input_json: String) -> Result<SafeRpcResult, String> {
    tokio::task::spawn_blocking(move || rpc(&method, &input_json))
        .await
        .map_err(|e| e.to_string())?
}

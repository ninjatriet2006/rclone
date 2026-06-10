use std::ffi::{CStr, CString};
use crate::functions::rclone::mod_bindings::{RcloneRPC, RcloneFreeString, RCLONE_ENGINE_LOCK};
use crate::functions::rclone::structs::SafeRpcResult;

pub fn rpc(method: &str, input_json: &str) -> Result<SafeRpcResult, String> {
    let start_time = std::time::Instant::now();
    let c_method = CString::new(method).map_err(|e| e.to_string())?;
    let c_input = CString::new(input_json).map_err(|e| e.to_string())?;

    let result = unsafe {
        let _guard = RCLONE_ENGINE_LOCK.lock().unwrap();
        let res = RcloneRPC(c_method.as_ptr(), c_input.as_ptr());
        if res.output.is_null() {
            return Err("RcloneRPC returned a null output pointer".to_string());
        }
        let c_str = CStr::from_ptr(res.output);
        let raw_output = c_str.to_string_lossy().into_owned();
        RcloneFreeString(res.output);
        SafeRpcResult {
            output: raw_output,
            status: res.status as i32,
        }
    };

    let elapsed = start_time.elapsed().as_millis();
    crate::functions::log::log_rpc(method, input_json, Some(result.status as u16), elapsed);

    Ok(result)
}

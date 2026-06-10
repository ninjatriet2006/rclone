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

lazy_static::lazy_static! {
    pub static ref RCLONE_ENGINE_LOCK: Mutex<()> = Mutex::new(());
}

pub fn initialize() {
    let _guard = RCLONE_ENGINE_LOCK.lock().unwrap();
    unsafe {
        RcloneInitialize();
    }
}

pub fn finalize() {
    let _guard = RCLONE_ENGINE_LOCK.lock().unwrap();
    unsafe {
        RcloneFinalize();
    }
}

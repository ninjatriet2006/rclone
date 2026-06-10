use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JobDirection {
    Upload,
    Download,
    Local,
    RemoteToRemote,
}

lazy_static::lazy_static! {
    pub(crate) static ref JOB_DIRECTIONS: Mutex<HashMap<i64, JobDirection>> = Mutex::new(HashMap::new());
    pub(crate) static ref JOB_REAL_SIZES: Mutex<HashMap<i64, u64>> = Mutex::new(HashMap::new());
}

pub fn register_job_direction(job_id: i64, direction: JobDirection) {
    if let Ok(mut map) = JOB_DIRECTIONS.lock() {
        map.insert(job_id, direction);
    }
}

pub fn get_job_direction(job_id: i64) -> Option<JobDirection> {
    if let Ok(map) = JOB_DIRECTIONS.lock() {
        map.get(&job_id).cloned()
    } else {
        None
    }
}

pub fn register_job_real_size(job_id: i64, size: u64) {
    if let Ok(mut map) = JOB_REAL_SIZES.lock() {
        map.insert(job_id, size);
    }
}

pub fn get_job_real_size(job_id: i64) -> Option<u64> {
    if let Ok(map) = JOB_REAL_SIZES.lock() {
        map.get(&job_id).cloned()
    } else {
        None
    }
}

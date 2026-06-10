use std::collections::HashMap;
use std::sync::Mutex;

lazy_static::lazy_static! {
    pub static ref JOB_DESCRIPTIONS: Mutex<HashMap<i64, String>> = Mutex::new(HashMap::new());
}

pub fn register_job_description(job_id: i64, description: String) {
    if let Ok(mut map) = JOB_DESCRIPTIONS.lock() {
        map.insert(job_id, description);
    }
}

pub fn get_job_description(job_id: i64) -> Option<String> {
    if let Ok(map) = JOB_DESCRIPTIONS.lock() {
        map.get(&job_id).cloned()
    } else {
        None
    }
}

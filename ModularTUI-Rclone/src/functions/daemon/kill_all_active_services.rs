use crate::functions::ActiveService;
use crate::functions::daemon::kill_process_by_pid::kill_process_by_pid;

pub fn kill_all_active_services(active_services: &mut Vec<ActiveService>) {
    for s in active_services.iter() {
        let is_mount = s.service_type_str == "Mount" || s.service_type_str.contains("Mount");
        let _ = kill_process_by_pid(s.pid, is_mount, &s.path);
    }
    active_services.clear();
}

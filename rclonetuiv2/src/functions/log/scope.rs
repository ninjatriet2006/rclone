use std::cell::RefCell;

thread_local! {
    pub static INDENT_LEVEL: RefCell<usize> = RefCell::new(0);
}

pub struct LogScope {
    module: &'static str,
    action: &'static str,
}

impl LogScope {
    pub fn new(module: &'static str, action: &'static str, desc: &str) -> Self {
        super::write::log_info_start(module, action, desc);
        INDENT_LEVEL.with(|level| {
            *level.borrow_mut() += 1;
        });
        LogScope { module, action }
    }
}

impl Drop for LogScope {
    fn drop(&mut self) {
        INDENT_LEVEL.with(|level| {
            let mut val = level.borrow_mut();
            if *val > 0 {
                *val -= 1;
            }
        });
        super::write::log_info_end(self.module, self.action);
    }
}

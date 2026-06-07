use ratatui::Frame;
use crate::functions::*;

pub fn draw_copy_progress_popup(frame: &mut Frame, src: &str, dest: &str, pct: f64) {
    let msg = translate("exp_copy_msg")
        .replacen("{}", src, 1)
        .replacen("{}", dest, 1)
        .replace("{:.1}", &format!("{:.1}", pct));
    draw_popup(frame, &translate("exp_copy_title"), &msg, 60, 35);
}

pub fn draw_move_progress_popup(frame: &mut Frame, src: &str, dest: &str, pct: f64) {
    let msg = translate("exp_move_msg")
        .replacen("{}", src, 1)
        .replacen("{}", dest, 1)
        .replace("{:.1}", &format!("{:.1}", pct));
    draw_popup(frame, &translate("exp_move_title"), &msg, 60, 35);
}

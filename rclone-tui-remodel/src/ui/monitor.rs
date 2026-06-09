use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

#[derive(Debug, Clone, PartialEq)]
pub enum MonitorPane {
    ActiveJobs,
    PendingJobs,
    FailedFiles,
}

#[derive(Debug, Clone)]
pub struct FailedCopyItem {
    pub src: String,
    pub dest: String,
    pub error: String,
    pub time: String,
    pub is_copy: bool,
}

#[derive(Debug, Clone)]
pub struct PendingCopyJob {
    pub src: String,
    pub dest: String,
    pub is_dir: bool,
    pub total_files: usize,
    pub restricted_files: Vec<String>,
    pub status: String,
    pub items: Option<Vec<crate::ui::explorer::ClipboardItem>>,
    pub use_checksum: bool,
}

#[derive(Debug, Clone)]
pub struct JobFile {
    pub path: String,       // Relative path
    pub size: u64,
    pub bytes: u64,
    pub speed: u64,
    pub percentage: u16,
    pub eta: i64,
    pub status: String,     // "completed", "running", "checking", "failed"
    pub error: String,      // Error message if failed
}

#[derive(Debug, Clone)]
pub struct VisibleNode {
    pub id: String,          // Unique ID: e.g. "job/1585", "job/1585:path/to/file"
    pub name: String,        // Display name
    pub is_dir: bool,
    pub is_job: bool,
    pub depth: usize,
    pub expanded: bool,
    pub status: String,      // "completed", "running", "checking", "failed", "" for directory
    pub size: u64,
    pub bytes: u64,
    pub speed: u64,
    pub eta: i64,
    pub percentage: u16,
    pub job_id: Option<i64>,  // The job ID it belongs to
    pub job_name: String,     // Job name for stop confirmation
    pub error: String,        // Error message if failed
}

#[derive(Debug, Clone)]
pub struct TransferJob {
    pub name: String,
    pub size: u64,
    pub bytes: u64,
    pub speed: u64,
    pub percentage: u16,
    pub eta: i64,
    pub job_id: Option<i64>,
    pub start_time: String,
    pub duration: f64,
    pub group: String,
    pub description: String,
    pub files: Vec<JobFile>,
}

pub struct MonitorState {
    pub speed: f64,
    pub upload_speed: f64,
    pub download_speed: f64,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub active_jobs: Vec<TransferJob>,
    pub history: Vec<String>,
    pub confirm_stop_job: Option<TransferJob>,
    pub failed_files: Vec<FailedCopyItem>,
    pub pending_jobs: Vec<PendingCopyJob>,
    pub selected_pending_idx: usize,
    pub selected_failed_idx: usize,
    pub active_pane: MonitorPane,
    pub max_bandwidth: u64,
    pub active_transfers: usize,
    pub active_checks: usize,
    pub transfers_limit: usize,
    pub checkers_limit: usize,
    pub bottleneck_reason: String,
    // tree states
    pub expanded_paths: std::collections::HashSet<String>,
    pub visible_nodes: Vec<VisibleNode>,
    pub selected_node_idx: usize,
    pub active_scroll_offset: usize,
    pub pending_scroll_offset: usize,
    pub failed_scroll_offset: usize,
}

struct TempTreeNode {
    name: String,
    is_dir: bool,
    path: String,
    status: String,
    size: u64,
    bytes: u64,
    speed: u64,
    eta: i64,
    percentage: u16,
    error: String,
    children: std::collections::BTreeMap<String, TempTreeNode>,
}

impl TempTreeNode {
    fn new(name: String, is_dir: bool, path: String) -> Self {
        TempTreeNode {
            name,
            is_dir,
            path,
            status: String::new(),
            size: 0,
            bytes: 0,
            speed: 0,
            eta: -1,
            percentage: 0,
            error: String::new(),
            children: std::collections::BTreeMap::new(),
        }
    }

    fn insert(&mut self, parts: &[&str], file: &JobFile) {
        if parts.is_empty() {
            return;
        }
        let current_part = parts[0].to_string();
        if parts.len() == 1 {
            let mut node = TempTreeNode::new(current_part.clone(), false, file.path.clone());
            node.status = file.status.clone();
            node.size = file.size;
            node.bytes = file.bytes;
            node.speed = file.speed;
            node.eta = file.eta;
            node.percentage = file.percentage;
            node.error = file.error.clone();
            self.children.insert(current_part, node);
        } else {
            let child_path = if self.path.is_empty() {
                current_part.clone()
            } else {
                format!("{}/{}", self.path, current_part)
            };
            let dir_node = self.children.entry(current_part.clone()).or_insert_with(|| {
                TempTreeNode::new(current_part, true, child_path)
            });
            dir_node.insert(&parts[1..], file);
        }
    }
}

fn flatten_tree(
    node: &TempTreeNode,
    depth: usize,
    job_id_str: &str,
    job_id: Option<i64>,
    job_name: &str,
    expanded_paths: &std::collections::HashSet<String>,
    visible_nodes: &mut Vec<VisibleNode>,
) {
    for (_, child) in &node.children {
        let node_id = format!("{}:{}", job_id_str, child.path);
        let expanded = expanded_paths.contains(&node_id);
        
        let visible_node = VisibleNode {
            id: node_id.clone(),
            name: child.name.clone(),
            is_dir: child.is_dir,
            is_job: false,
            depth,
            expanded,
            status: child.status.clone(),
            size: child.size,
            bytes: child.bytes,
            speed: child.speed,
            eta: child.eta,
            percentage: child.percentage,
            job_id,
            job_name: job_name.to_string(),
            error: child.error.clone(),
        };
        
        visible_nodes.push(visible_node);
        
        if child.is_dir && expanded {
            flatten_tree(
                child,
                depth + 1,
                job_id_str,
                job_id,
                job_name,
                expanded_paths,
                visible_nodes,
            );
        }
    }
}

fn get_parent_id(id: &str, active_jobs: &[TransferJob]) -> Option<String> {
    if let Some(colon_idx) = id.find(':') {
        let job_prefix = &id[..colon_idx];
        let rel_path = &id[colon_idx + 1..];
        if let Some(last_slash) = rel_path.rfind('/') {
            Some(format!("{}:{}", job_prefix, &rel_path[..last_slash]))
        } else {
            Some(job_prefix.to_string())
        }
    } else if id.starts_with("job/") {
        if let Ok(job_id) = id["job/".len()..].parse::<i64>() {
            if let Some(job) = active_jobs.iter().find(|j| j.job_id == Some(job_id)) {
                if job.description.contains("Tác vụ nền") {
                    return Some("group/background_tasks".to_string());
                }
            }
        }
        None
    } else if id.starts_with("op/") {
        let parts: Vec<&str> = id.split('/').collect();
        if parts.len() > 2 {
            return Some(format!("op/{}", parts[1]));
        }
        None
    } else {
        None
    }
}

#[allow(dead_code)]
fn find_active_op_for_job(job: &TransferJob, ops: &[crate::app::ActiveOperation]) -> Option<crate::app::ActiveOperation> {
    for op in ops {
        if job.description.contains(&op.dest) {
            return Some(op.clone());
        }
        for item in &op.items {
            if job.description.contains(item) {
                return Some(op.clone());
            }
        }
        if let Some(ref completed) = op.completed_items {
            for item in completed {
                if job.description.contains(item) {
                    return Some(op.clone());
                }
            }
        }
    }
    None
}

impl MonitorState {
    pub fn new() -> Self {
        let mut expanded_paths = std::collections::HashSet::new();
        expanded_paths.insert("group/background_tasks".to_string());
        MonitorState {
            speed: 0.0,
            upload_speed: 0.0,
            download_speed: 0.0,
            bytes_transferred: 0,
            total_bytes: 0,
            active_jobs: Vec::new(),
            history: Vec::new(),
            confirm_stop_job: None,
            failed_files: Vec::new(),
            pending_jobs: Vec::new(),
            selected_pending_idx: 0,
            selected_failed_idx: 0,
            active_pane: MonitorPane::ActiveJobs,
            max_bandwidth: 12_500_000,
            active_transfers: 0,
            active_checks: 0,
            transfers_limit: 8,
            checkers_limit: 16,
            bottleneck_reason: "Tốc độ tối ưu / Bình thường (Optimal)".to_string(),
            expanded_paths,
            visible_nodes: Vec::new(),
            selected_node_idx: 0,
            active_scroll_offset: 0,
            pending_scroll_offset: 0,
            failed_scroll_offset: 0,
        }
    }

    pub fn rebuild_visible_nodes(&mut self) {
        self.visible_nodes.clear();
        
        let mut running_paths = std::collections::HashSet::new();
        for j in &self.active_jobs {
            for f in &j.files {
                running_paths.insert(f.path.clone());
                let mut path = f.path.as_str();
                while let Some(idx) = path.rfind('/') {
                    path = &path[..idx];
                    if !path.is_empty() {
                        running_paths.insert(path.to_string());
                    }
                }
            }
        }
        
        let mut bg_jobs = Vec::new();
        let mut user_jobs = Vec::new();
        for job in &self.active_jobs {
            if job.description.contains("Tác vụ nền") {
                bg_jobs.push(job);
            } else {
                user_jobs.push(job);
            }
        }

        // 1. Luôn luôn hiển thị thư mục "Tác vụ nền" ở đầu để tránh nhảy giao diện
        let total_size: u64 = bg_jobs.iter().map(|j| j.size).sum();
        let total_bytes: u64 = bg_jobs.iter().map(|j| j.bytes).sum();
        let total_speed: u64 = bg_jobs.iter().map(|j| j.speed).sum();
        let total_pct = if total_size > 0 {
            ((total_bytes as f64 / total_size as f64) * 100.0) as u16
        } else {
            0
        };

        let bg_folder_expanded = self.expanded_paths.contains("group/background_tasks");
        let bg_folder_node = VisibleNode {
            id: "group/background_tasks".to_string(),
            name: "Tác vụ nền".to_string(),
            is_dir: true,
            is_job: false,
            depth: 0,
            expanded: bg_folder_expanded,
            status: String::new(),
            size: total_size,
            bytes: total_bytes,
            speed: total_speed,
            eta: -1,
            percentage: total_pct,
            job_id: None,
            job_name: "Tác vụ nền".to_string(),
            error: String::new(),
        };
        self.visible_nodes.push(bg_folder_node);

        if bg_folder_expanded {
            for job in bg_jobs {
                let job_id_str = format!("job/{}", job.job_id.unwrap_or(0));
                let job_expanded = self.expanded_paths.contains(&job_id_str);
                
                let job_node = VisibleNode {
                    id: job_id_str.clone(),
                    name: job.name.clone(),
                    is_dir: false,
                    is_job: true,
                    depth: 1,
                    expanded: job_expanded,
                    status: String::new(),
                    size: job.size,
                    bytes: job.bytes,
                    speed: job.speed,
                    eta: job.eta,
                    percentage: job.percentage,
                    job_id: job.job_id,
                    job_name: job.name.clone(),
                    error: String::new(),
                };
                self.visible_nodes.push(job_node);

                if job_expanded {
                    let mut root = TempTreeNode::new(String::new(), true, String::new());
                    for file in &job.files {
                        let parts: Vec<&str> = file.path.split('/').filter(|s| !s.is_empty()).collect();
                        root.insert(&parts, file);
                    }
                    
                    flatten_tree(
                        &root,
                        2,
                        &job_id_str,
                        job.job_id,
                        &job.name,
                        &self.expanded_paths,
                        &mut self.visible_nodes,
                    );
                }
            }
        }

        // 2. Hiển thị từng tác vụ hoạt động từ active_ops.json như một thư mục tiến trình
        let active_ops = crate::app::load_active_operations();
        let pre_ops = crate::app::load_pre_operations();
        for op in &active_ops {
            let total_items = op.items.len() + op.completed_items.as_ref().map(|c| c.len()).unwrap_or(0);
            if total_items == 0 {
                continue;
            }
            
            let done_items = op.completed_items.as_ref().map(|c| c.len()).unwrap_or(0);
            let pct = if total_items > 0 {
                ((done_items as f64 / total_items as f64) * 100.0) as u16
            } else {
                100
            };
            
            let op_folder_id = format!("op/{}", op.id);
            let op_expanded = self.expanded_paths.contains(&op_folder_id);
            let is_scanning = pre_ops.iter().any(|po| po.id == op.id && po.status == "scanning");
            let task_name = if is_scanning {
                let trans_checking = crate::lang::translate("mon_action_checking");
                if trans_checking != "mon_action_checking" {
                    trans_checking
                } else {
                    "Kiểm tra".to_string()
                }
            } else if op.action_type == "copy" {
                "Sao chép".to_string()
            } else if op.action_type == "move" {
                "Di chuyển".to_string()
            } else {
                "Xóa".to_string()
            };
            
            let op_node = VisibleNode {
                id: op_folder_id.clone(),
                name: format!("{} - {}", op.id, task_name),
                is_dir: true,
                is_job: false,
                depth: 0,
                expanded: op_expanded,
                status: if is_scanning { "checking".to_string() } else { String::new() },
                size: total_items as u64,
                bytes: done_items as u64,
                speed: 0,
                eta: -1,
                percentage: pct,
                job_id: None,
                job_name: op.id.clone(),
                error: String::new(),
            };
            self.visible_nodes.push(op_node);
            
            if op_expanded {
                let mut root = TempTreeNode::new(String::new(), true, String::new());
                
                let task_map = op.tasks.as_ref().map(|tasks| {
                    tasks.iter()
                        .map(|t| (t.name.as_str(), t))
                        .collect::<std::collections::HashMap<&str, &crate::app::FileTask>>()
                });

                let completed_iter = op.completed_items.as_ref()
                    .map(|v| v.iter().map(|item| (item, true)))
                    .into_iter()
                    .flatten();
                let queued_iter = op.items.iter().map(|item| (item, false));
                
                for (item, is_completed) in completed_iter.chain(queued_iter) {
                    let mut size = 0;
                    let mut bytes = 0;
                    let mut status = if is_completed {
                        "completed".to_string()
                    } else if is_scanning {
                        "checking".to_string()
                    } else if running_paths.contains(item) {
                        "running".to_string()
                    } else {
                        "queued".to_string()
                    };
                    let mut error = String::new();

                    if let Some(ref map) = task_map {
                        if let Some(task) = map.get(item.as_str()) {
                            size = task.size;
                            status = match task.status {
                                crate::app::TaskStatus::Pending => {
                                    if is_scanning {
                                        "checking".to_string()
                                    } else {
                                        "queued".to_string()
                                    }
                                }
                                crate::app::TaskStatus::Transferring => "running".to_string(),
                                crate::app::TaskStatus::Completed => "completed".to_string(),
                                crate::app::TaskStatus::Failed => "failed".to_string(),
                                crate::app::TaskStatus::Skipped => "skipped".to_string(),
                            };
                            if status == "completed" || status == "skipped" {
                                bytes = size;
                            }
                            if let Some(ref err) = task.error {
                                error = err.clone();
                            }
                        }
                    } else if is_completed {
                        status = "completed".to_string();
                    }

                    let percentage = if size > 0 {
                        ((bytes as f64 / size as f64) * 100.0) as u16
                    } else if is_completed || status == "completed" || status == "skipped" {
                        100
                    } else {
                        0
                    };

                    let job_file = JobFile {
                        path: item.to_string(),
                        size,
                        bytes,
                        speed: 0,
                        percentage,
                        eta: -1,
                        status,
                        error,
                    };

                    let parts: Vec<&str> = item.split('/').filter(|s| !s.is_empty()).collect();
                    root.insert(&parts, &job_file);
                }

                flatten_tree(
                    &root,
                    1,
                    &op_folder_id,
                    None,
                    &op.id,
                    &self.expanded_paths,
                    &mut self.visible_nodes,
                );
            }
        }

        // 3. Hiển thị các job trực tiếp của người dùng
        for job in user_jobs {
            let job_id_str = format!("job/{}", job.job_id.unwrap_or(0));
            let job_expanded = self.expanded_paths.contains(&job_id_str);
            
            let job_node = VisibleNode {
                id: job_id_str.clone(),
                name: job.name.clone(),
                is_dir: false,
                is_job: true,
                depth: 0,
                expanded: job_expanded,
                status: String::new(),
                size: job.size,
                bytes: job.bytes,
                speed: job.speed,
                eta: job.eta,
                percentage: job.percentage,
                job_id: job.job_id,
                job_name: job.name.clone(),
                error: String::new(),
            };
            self.visible_nodes.push(job_node);

            if job_expanded {
                let mut root = TempTreeNode::new(String::new(), true, String::new());
                for file in &job.files {
                    let parts: Vec<&str> = file.path.split('/').filter(|s| !s.is_empty()).collect();
                    root.insert(&parts, file);
                }
                
                flatten_tree(
                    &root,
                    1,
                    &job_id_str,
                    job.job_id,
                    &job.name,
                    &self.expanded_paths,
                    &mut self.visible_nodes,
                );
            }
        }
    }

    pub fn toggle_expand(&mut self) {
        if self.active_pane == MonitorPane::ActiveJobs && !self.visible_nodes.is_empty() {
            if self.selected_node_idx >= self.visible_nodes.len() {
                self.selected_node_idx = 0;
            }
            let node = &self.visible_nodes[self.selected_node_idx];
            if node.is_job || node.is_dir {
                if self.expanded_paths.contains(&node.id) {
                    self.expanded_paths.remove(&node.id);
                } else {
                    self.expanded_paths.insert(node.id.clone());
                }
                self.rebuild_visible_nodes();
            }
        }
    }

    pub fn expand_node(&mut self) {
        if self.active_pane == MonitorPane::ActiveJobs && !self.visible_nodes.is_empty() {
            if self.selected_node_idx >= self.visible_nodes.len() {
                self.selected_node_idx = 0;
            }
            let node = &self.visible_nodes[self.selected_node_idx];
            if (node.is_job || node.is_dir) && !self.expanded_paths.contains(&node.id) {
                self.expanded_paths.insert(node.id.clone());
                self.rebuild_visible_nodes();
            }
        }
    }

    pub fn collapse_node(&mut self) {
        if self.active_pane == MonitorPane::ActiveJobs && !self.visible_nodes.is_empty() {
            if self.selected_node_idx >= self.visible_nodes.len() {
                self.selected_node_idx = 0;
            }
            let node = self.visible_nodes[self.selected_node_idx].clone();
            if (node.is_job || node.is_dir) && self.expanded_paths.contains(&node.id) {
                self.expanded_paths.remove(&node.id);
                self.rebuild_visible_nodes();
            } else {
                if let Some(parent_id) = get_parent_id(&node.id, &self.active_jobs) {
                    if let Some(idx) = self.visible_nodes.iter().position(|n| n.id == parent_id) {
                        self.selected_node_idx = idx;
                    }
                }
            }
        }
    }

    pub fn next(&mut self) {
        match self.active_pane {
            MonitorPane::ActiveJobs => {
                if !self.visible_nodes.is_empty() {
                    self.selected_node_idx = (self.selected_node_idx + 1) % self.visible_nodes.len();
                }
            }
            MonitorPane::PendingJobs => {
                if !self.pending_jobs.is_empty() {
                    self.selected_pending_idx = (self.selected_pending_idx + 1) % self.pending_jobs.len();
                }
            }
            MonitorPane::FailedFiles => {
                if !self.failed_files.is_empty() {
                    self.selected_failed_idx = (self.selected_failed_idx + 1) % self.failed_files.len();
                }
            }
        }
    }

    pub fn prev(&mut self) {
        match self.active_pane {
            MonitorPane::ActiveJobs => {
                if !self.visible_nodes.is_empty() {
                    if self.selected_node_idx == 0 {
                        self.selected_node_idx = self.visible_nodes.len() - 1;
                    } else {
                        self.selected_node_idx -= 1;
                    }
                }
            }
            MonitorPane::PendingJobs => {
                if !self.pending_jobs.is_empty() {
                    if self.selected_pending_idx == 0 {
                        self.selected_pending_idx = self.pending_jobs.len() - 1;
                    } else {
                        self.selected_pending_idx -= 1;
                    }
                }
            }
            MonitorPane::FailedFiles => {
                if !self.failed_files.is_empty() {
                    if self.selected_failed_idx == 0 {
                        self.selected_failed_idx = self.failed_files.len() - 1;
                    } else {
                        self.selected_failed_idx -= 1;
                    }
                }
            }
        }
    }
}

fn make_colored_progress_bar(
    percentage: u16,
    width: usize,
    is_active: bool,
    is_error: bool,
    cursor_color: Color,
) -> Vec<Span<'static>> {
    let percentage = percentage.min(100) as usize;
    let mut spans = vec![Span::styled("[", Style::default().fg(Color::Gray))];

    let filled_green = (percentage * width) / 100;

    if is_error {
        let filled_red = width - filled_green;
        if filled_green > 0 {
            spans.push(Span::styled("█".repeat(filled_green), Style::default().fg(Color::Green)));
        }
        if filled_red > 0 {
            spans.push(Span::styled("█".repeat(filled_red), Style::default().fg(Color::Red)));
        }
    } else if is_active {
        let filled_yellow = if filled_green < width { 1 } else { 0 };
        let filled_white = width - filled_green - filled_yellow;

        if filled_green > 0 {
            spans.push(Span::styled("█".repeat(filled_green), Style::default().fg(Color::Green)));
        }
        if filled_yellow > 0 {
            spans.push(Span::styled("█", Style::default().fg(cursor_color)));
        }
        if filled_white > 0 {
            spans.push(Span::styled("░".repeat(filled_white), Style::default().fg(Color::White)));
        }
    } else {
        let filled_white = width - filled_green;
        if filled_green > 0 {
            spans.push(Span::styled("█".repeat(filled_green), Style::default().fg(Color::Green)));
        }
        if filled_white > 0 {
            spans.push(Span::styled("░".repeat(filled_white), Style::default().fg(Color::White)));
        }
    }

    spans.push(Span::styled("]", Style::default().fg(Color::Gray)));
    spans
}

pub fn draw(state: &mut MonitorState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),      // Tổng quan tiến trình (Global Stats)
            Constraint::Percentage(55), // Khung ở giữa (Active, Pending & Failed)
            Constraint::Min(5),         // Chi tiết tác vụ đang chọn
            Constraint::Length(3),      // Help bar
        ])
        .split(area);

    // Split the middle section horizontally: Left column (Active + Pending Jobs), Right column (Failed Files)
    let mid_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), // Active & Pending Jobs
            Constraint::Percentage(40), // Failed & Restricted Files
        ])
        .split(chunks[1]);

    // Split Left column vertically: Active Jobs (60%), Pending Jobs (40%)
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60), // Active Jobs
            Constraint::Percentage(40), // Pending Jobs
        ])
        .split(mid_chunks[0]);

    // 1. Vẽ Tổng quan tiến trình
    let speed_str = format!("{}/s", super::format_size(state.speed as u64));
    let upload_str = format!("{}/s", super::format_size(state.upload_speed as u64));
    let download_str = format!("{}/s", super::format_size(state.download_speed as u64));
    let max_bw_str = format!("{}/s", super::format_size(state.max_bandwidth));
    let progress_pct = if state.active_jobs.is_empty() {
        100.0
    } else if state.total_bytes > 0 {
        (state.bytes_transferred as f64 / state.total_bytes as f64) * 100.0
    } else {
        0.0
    };

    let first_line_spans = vec![
        Span::raw(crate::lang::translate("mon_speed_label")),
        Span::styled(
            speed_str,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(crate::lang::translate("mon_upload_speed_label")),
        Span::styled(
            upload_str,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(crate::lang::translate("mon_download_speed_label")),
        Span::styled(
            download_str,
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(crate::lang::translate("mon_max_bandwidth_label")),
        Span::styled(
            max_bw_str,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    let mut pct_line_spans = vec![
        Span::raw(crate::lang::translate("mon_total_pct_label")),
    ];
    pct_line_spans.extend(make_colored_progress_bar(
        progress_pct as u16,
        25,
        !state.active_jobs.is_empty(),
        !state.failed_files.is_empty(),
        Color::Yellow,
    ));
    pct_line_spans.push(Span::styled(
        format!(" {:.1}%", progress_pct),
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    ));
    pct_line_spans.push(Span::raw(crate::lang::translate("mon_transferred_label")));
    pct_line_spans.push(Span::styled(
        super::format_size(state.bytes_transferred),
        Style::default().fg(Color::Green),
    ));
    pct_line_spans.push(Span::raw(" / "));
    pct_line_spans.push(Span::styled(
        super::format_size(state.total_bytes),
        Style::default().fg(Color::Cyan),
    ));

    let third_line_spans = vec![
        Span::raw(crate::lang::translate("mon_active_transfers_label")),
        Span::styled(
            state.active_transfers.to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(crate::lang::translate("mon_active_checkers_label")),
        Span::styled(
            state.active_checks.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | PID Engine: "),
        Span::styled(
            std::process::id().to_string(),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    let fourth_line_spans = vec![
        Span::raw(" Phân tích nghẽn (Bottleneck): "),
        Span::styled(
            state.bottleneck_reason.clone(),
            Style::default()
                .fg(if state.bottleneck_reason.contains("Bình thường") || state.bottleneck_reason.contains("Optimal") {
                    Color::Green
                } else if state.bottleneck_reason.contains("băng thông") || state.bottleneck_reason.contains("Limit") {
                    Color::Yellow
                } else {
                    Color::Red
                })
                .add_modifier(Modifier::BOLD),
        ),
    ];

    let stats_text = vec![
        Line::from(first_line_spans),
        Line::from(pct_line_spans),
        Line::from(third_line_spans),
        Line::from(fourth_line_spans),
    ];

    let stats_block = Block::default()
        .title(Span::styled(
            crate::lang::translate("mon_stats_title"),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let stats_paragraph = Paragraph::new(stats_text).block(stats_block);
    frame.render_widget(stats_paragraph, chunks[0]);

    // 2. Vẽ các Job đang chạy (Active Jobs)
    let is_active_focused = state.active_pane == MonitorPane::ActiveJobs;
    let active_border_color = if is_active_focused { Color::Yellow } else { Color::DarkGray };

    let active_height = left_chunks[0].height.saturating_sub(2) as usize;
    if state.visible_nodes.is_empty() {
        state.active_scroll_offset = 0;
    } else {
        if state.selected_node_idx >= state.visible_nodes.len() {
            state.selected_node_idx = 0;
        }
        if state.selected_node_idx < state.active_scroll_offset {
            state.active_scroll_offset = state.selected_node_idx;
        } else if state.selected_node_idx >= state.active_scroll_offset + active_height {
            state.active_scroll_offset = state.selected_node_idx - active_height + 1;
        }
    }

    let active_items: Vec<ListItem> = state
        .visible_nodes
        .iter()
        .enumerate()
        .skip(state.active_scroll_offset)
        .take(active_height)
        .map(|(i, node)| {
            let style = if is_active_focused && i == state.selected_node_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let indent = "  ".repeat(node.depth);

            let line = if node.is_job {
                let expand_marker = if node.expanded { "▼ " } else { "▶ " };
                let bar_spans = make_colored_progress_bar(node.percentage, 12, true, false, Color::Yellow);
                let eta_str = if node.eta >= 0 {
                    format!("ETA: {}s", node.eta)
                } else {
                    "ETA: --".to_string()
                };

                let mut spans = vec![
                    Span::raw(indent),
                    Span::styled(expand_marker, Style::default().fg(Color::Yellow)),
                    Span::styled("⚡ ", Style::default().fg(Color::Yellow)),
                ];
                spans.extend(bar_spans);
                spans.push(Span::styled(
                    format!(" {}% ", node.percentage),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    format!("{} ", node.name),
                    Style::default().add_modifier(Modifier::BOLD),
                ));

                if node.size > 0 {
                    spans.push(Span::styled(
                        format!(
                            "({} / {}) ",
                            super::format_size(node.bytes),
                            super::format_size(node.size)
                        ),
                        Style::default().fg(Color::DarkGray),
                    ));
                }

                spans.push(Span::styled(
                    format!("{} - ", eta_str),
                    Style::default().fg(Color::Magenta),
                ));
                spans.push(Span::styled(
                    format!("{}/s", super::format_size(node.speed)),
                    Style::default().fg(Color::Yellow),
                ));

                Line::from(spans)
            } else if node.is_dir {
                let expand_marker = if node.expanded { "▼ " } else { "▶ " };
                let line = if node.id == "group/background_tasks" {
                    let mut spans = vec![
                        Span::raw(indent),
                        Span::styled(expand_marker, Style::default().fg(Color::Yellow)),
                        Span::styled("📁 ", Style::default().fg(Color::Cyan)),
                        Span::styled(
                            format!("{} ", node.name),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                    ];
                    
                    if node.size > 0 {
                        let bar_spans = make_colored_progress_bar(node.percentage, 12, true, false, Color::Yellow);
                        spans.push(Span::raw(" "));
                        spans.extend(bar_spans);
                        spans.push(Span::styled(
                            format!(" {}% ", node.percentage),
                            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                        ));
                        spans.push(Span::styled(
                            format!(
                                "({} / {}) ",
                                super::format_size(node.bytes),
                                super::format_size(node.size)
                            ),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    
                    if node.speed > 0 {
                        spans.push(Span::styled(
                            format!("{}/s", super::format_size(node.speed)),
                            Style::default().fg(Color::Yellow),
                        ));
                    }
                    
                    Line::from(spans)
                } else if node.id.starts_with("op/") {
                    let is_op_checking = node.status == "checking";
                    let mut spans = vec![
                        Span::raw(indent),
                        Span::styled(expand_marker, Style::default().fg(Color::Yellow)),
                        Span::styled("📁 ", Style::default().fg(Color::Cyan)),
                        Span::styled(
                            format!("{} ", node.name),
                            if is_op_checking {
                                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().add_modifier(Modifier::BOLD)
                            },
                        ),
                    ];
                    
                    if node.size > 0 {
                        let bar_color = if is_op_checking { Color::Cyan } else { Color::Yellow };
                        let bar_spans = make_colored_progress_bar(node.percentage, 12, true, false, bar_color);
                        spans.push(Span::raw(" "));
                        spans.extend(bar_spans);
                        spans.push(Span::styled(
                            format!(" {}% ", node.percentage),
                            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                        ));
                        spans.push(Span::styled(
                            format!(
                                "({} / {} mục) ",
                                node.bytes,
                                node.size
                            ),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    Line::from(spans)
                } else {
                    let spans = vec![
                        Span::raw(indent),
                        Span::styled(expand_marker, Style::default().fg(Color::Yellow)),
                        Span::styled("📁 ", Style::default().fg(Color::Cyan)),
                        Span::styled(
                            format!("{} ", node.name),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                    ];
                    Line::from(spans)
                };
                line
            } else {
                let expand_marker = "  ";
                let icon = match node.status.as_str() {
                    "completed" => ("🟢 ", Color::Green),
                    "running" => ("⏳ ", Color::Yellow),
                    "checking" => ("🔍 ", Color::Cyan),
                    "failed" => ("🔴 ", Color::Red),
                    "queued" => ("🕒 ", Color::DarkGray),
                    _ => ("📄 ", Color::Gray),
                };

                let mut spans = vec![
                    Span::raw(indent),
                    Span::raw(expand_marker),
                    Span::styled(icon.0, Style::default().fg(icon.1)),
                    Span::styled(
                        node.name.clone(),
                        if node.status == "checking" {
                            Style::default().fg(Color::Cyan)
                        } else {
                            Style::default()
                        }
                    ),
                ];

                if node.status == "failed" {
                    spans.push(Span::styled(
                        format!(" (Lỗi: {})", node.error),
                        Style::default().fg(Color::Red),
                    ));
                } else if node.status == "running" {
                    let bar_spans = make_colored_progress_bar(node.percentage, 10, true, false, Color::Yellow);
                    spans.push(Span::raw(" "));
                    spans.extend(bar_spans);
                    spans.push(Span::styled(
                        format!(" {}% ", node.percentage),
                        Style::default().fg(Color::Green),
                    ));
                    if node.size > 0 {
                        spans.push(Span::styled(
                            format!(
                                "({} / {}) ",
                                super::format_size(node.bytes),
                                super::format_size(node.size)
                            ),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    if node.speed > 0 {
                        spans.push(Span::styled(
                            format!("{}/s ", super::format_size(node.speed)),
                            Style::default().fg(Color::Yellow),
                        ));
                    }
                    if node.eta >= 0 {
                        spans.push(Span::styled(
                            format!("ETA: {}s", node.eta),
                            Style::default().fg(Color::Magenta),
                        ));
                    }
                } else if node.status == "completed" {
                    if node.size > 0 {
                        spans.push(Span::styled(
                            format!(" ({})", super::format_size(node.size)),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                }

                Line::from(spans)
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let active_block = Block::default()
        .title(Span::styled(
            format!(" {} ", crate::lang::translate("mon_active_title")),
            Style::default()
                .fg(active_border_color)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(active_border_color));
    let active_list = List::new(active_items).block(active_block);
    frame.render_widget(active_list, left_chunks[0]);

    // 3. Vẽ các tác vụ đang chờ xác nhận (Pending Jobs)
    let is_pending_focused = state.active_pane == MonitorPane::PendingJobs;
    let pending_border_color = if is_pending_focused { Color::Yellow } else { Color::DarkGray };

    let pending_height = left_chunks[1].height.saturating_sub(2) as usize;
    if state.pending_jobs.is_empty() {
        state.pending_scroll_offset = 0;
    } else {
        if state.selected_pending_idx >= state.pending_jobs.len() {
            state.selected_pending_idx = 0;
        }
        if state.selected_pending_idx < state.pending_scroll_offset {
            state.pending_scroll_offset = state.selected_pending_idx;
        } else if state.selected_pending_idx >= state.pending_scroll_offset + pending_height {
            state.pending_scroll_offset = state.selected_pending_idx - pending_height + 1;
        }
    }

    let pending_items: Vec<ListItem> = state
        .pending_jobs
        .iter()
        .enumerate()
        .skip(state.pending_scroll_offset)
        .take(pending_height)
        .map(|(i, job)| {
            let style = if is_pending_focused && i == state.selected_pending_idx {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let status_tag = match job.status.as_str() {
                "Bypassed" => "[ĐÃ BỎ QUA - CẦN QUYẾT ĐỊNH]",
                "Scanned (Has Restrictions)" => "[CÓ FILE BỊ CHẶN TẢI]",
                "Scanned (No Restrictions)" => "[AN TOÀN - KHÔNG BỊ CHẶN]",
                _ => "[CHỜ]",
            };

            let status_color = match job.status.as_str() {
                "Scanned (Has Restrictions)" => Color::Red,
                "Scanned (No Restrictions)" => Color::Green,
                _ => Color::Yellow,
            };

            let has_errors = job.status == "Scanned (Has Restrictions)";
            let bar_spans = make_colored_progress_bar(0, 12, false, has_errors, Color::Yellow);

            let mut spans = vec![
                Span::styled("⚠️ ", Style::default().fg(Color::Yellow)),
            ];
            spans.extend(bar_spans);
            spans.push(Span::styled(format!(" {} ", status_tag), Style::default().fg(status_color).add_modifier(Modifier::BOLD)));
            spans.push(Span::styled(format!("{} -> {} ", job.src, job.dest), Style::default().add_modifier(Modifier::BOLD)));
            spans.push(Span::styled(
                format!("(Bị chặn {}/{} file)", job.restricted_files.len(), job.total_files),
                Style::default().fg(Color::LightRed)
            ));

            let line = Line::from(spans);

            ListItem::new(line).style(style)
        })
        .collect();

    let pending_block = Block::default()
        .title(Span::styled(
            " TÁC VỤ SAO CHÉP CHỜ XÁC NHẬN (PENDING) ",
            Style::default()
                .fg(pending_border_color)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(pending_border_color));
    let pending_list = List::new(pending_items).block(pending_block);
    frame.render_widget(pending_list, left_chunks[1]);

    // 4. Vẽ danh sách các file lỗi hoặc file bị hạn chế (Failed / Restricted Files)
    let is_failed_focused = state.active_pane == MonitorPane::FailedFiles;
    let failed_border_color = if is_failed_focused { Color::Yellow } else { Color::Red };

    let failed_height = mid_chunks[1].height.saturating_sub(2) as usize;

    let (right_panel_title, right_panel_items) = if state.active_pane == MonitorPane::PendingJobs && !state.pending_jobs.is_empty() {
        let job = &state.pending_jobs[state.selected_pending_idx];
        let title = format!(" FILE BỊ KHÓA CỦA JOB ĐANG CHỌN ({}) ", job.restricted_files.len());
        
        let list_len = job.restricted_files.len();
        if state.failed_scroll_offset >= list_len {
            state.failed_scroll_offset = 0;
        }
        
        let items: Vec<ListItem> = job.restricted_files.iter()
            .skip(state.failed_scroll_offset)
            .take(failed_height)
            .map(|f| {
                ListItem::new(Line::from(vec![
                    Span::styled("🔒 ", Style::default().fg(Color::Red)),
                    Span::styled(f.clone(), Style::default().fg(Color::White)),
                ]))
            }).collect();
        (title, items)
    } else {
        let title = " CÁC FILE BỊ LỖI QUYỀN / KHÔNG SAO CHÉP ĐƯỢC ".to_string();
        if state.failed_files.is_empty() {
            state.failed_scroll_offset = 0;
        } else {
            if state.selected_failed_idx >= state.failed_files.len() {
                state.selected_failed_idx = 0;
            }
            if state.selected_failed_idx < state.failed_scroll_offset {
                state.failed_scroll_offset = state.selected_failed_idx;
            } else if state.selected_failed_idx >= state.failed_scroll_offset + failed_height {
                state.failed_scroll_offset = state.selected_failed_idx - failed_height + 1;
            }
        }
        let items: Vec<ListItem> = state
            .failed_files
            .iter()
            .enumerate()
            .skip(state.failed_scroll_offset)
            .take(failed_height)
            .map(|(i, item)| {
                let is_selected = is_failed_focused && i == state.selected_failed_idx;
                let text_style = if is_selected {
                    Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let err_style = if is_selected {
                    Style::default().bg(Color::Yellow).fg(Color::Red).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Red)
                };
                let time_style = if is_selected {
                    Style::default().bg(Color::Yellow).fg(Color::DarkGray).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                let line = Line::from(vec![
                    Span::styled("❌ ", Style::default().fg(Color::Red)),
                    Span::styled(format!("[{}] ", item.time), time_style),
                    Span::styled(format!("{} ", item.src), text_style),
                    Span::styled(format!("(Lỗi: {})", item.error), err_style),
                ]);
                ListItem::new(line)
            })
            .collect();
        (title, items)
    };

    let right_block = Block::default()
        .title(Span::styled(
            right_panel_title,
            Style::default()
                .fg(failed_border_color)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(failed_border_color));
    let right_list = List::new(right_panel_items).block(right_block);
    frame.render_widget(right_list, mid_chunks[1]);

    // 3. Vẽ nhật ký debug/chi tiết tác vụ đang chọn
    let details_block = Block::default()
        .title(Span::styled(
            " NHẬT KÝ DEBUG & CHI TIẾT TÁC VỤ ĐANG CHỌN ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let details_text = if state.active_jobs.is_empty() {
        vec![Line::from(vec![
            Span::styled("[DEBUG] Không có tác vụ rclone nào đang chạy.", Style::default().fg(Color::Black)),
        ])]
    } else {
        let selected_job = if state.selected_node_idx < state.visible_nodes.len() {
            let node = &state.visible_nodes[state.selected_node_idx];
            state.active_jobs.iter().find(|j| j.job_id == node.job_id)
        } else {
            None
        }.or_else(|| state.active_jobs.first());

        if let Some(job) = selected_job {
            let mut lines = Vec::new();

            lines.push(Line::from(vec![
                Span::styled(
                    format!(
                        "[DEBUG] Tác vụ: {} | Job ID: {}",
                        job.name,
                        job.job_id
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| "Không có".to_string())
                    ),
                    Style::default().fg(Color::Black),
                ),
            ]));

            if job.job_id.is_some() {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!(
                            "[DEBUG] Nhóm thống kê: {} | Bắt đầu: {} | Đã chạy: {:.1}s",
                            job.group, job.start_time, job.duration
                        ),
                        Style::default().fg(Color::Black),
                    ),
                ]));
            }

            lines.push(Line::from(vec![
                Span::styled(
                    format!(
                        "[DEBUG] Tiến độ: {}% | Tốc độ: {}/s | Truyền tải: {} / {}",
                        job.percentage,
                        super::format_size(job.speed),
                        super::format_size(job.bytes),
                        super::format_size(job.size)
                    ),
                    Style::default().fg(Color::Black),
                ),
            ]));

            if !job.description.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("[DEBUG] Lệnh/Mô tả đầy đủ: {}", job.description),
                        Style::default().fg(Color::Black),
                    ),
                ]));
            }

            lines
        } else {
            vec![Line::from(vec![
                Span::styled("[DEBUG] Vui lòng chọn một tác vụ phía trên.", Style::default().fg(Color::Black)),
            ])]
        }
    };

    let details_paragraph = Paragraph::new(details_text).block(details_block);
    frame.render_widget(details_paragraph, chunks[2]);

    // Help Bar
    let help_text = if state.active_pane == MonitorPane::PendingJobs {
        " [Tab] Chuyển Khung | [Up/Down] Chọn | [Enter/C] Giải quyết | [Delete/D] Xóa Job | [Esc] Quay lại "
    } else if state.active_pane == MonitorPane::FailedFiles {
        " [Tab] Chuyển Khung | [Up/Down] Chọn | [Alt+R] Thử lại tệp lỗi | [Esc] Quay lại "
    } else {
        " [Left/Right/Space] Thu nhỏ/Mở rộng | [Up/Down] Chọn | [Delete/D] Dừng Job | [Tab] Chuyển Khung | [Esc] Quay lại "
    };
    let help_paragraph = Paragraph::new(
        super::parse_help_line(help_text),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(help_paragraph, chunks[3]);

    // Vẽ popup xác nhận dừng tác vụ
    if let Some(job) = &state.confirm_stop_job {
        let overlay_area = super::centered_rect(60, 25, area);
        frame.render_widget(Clear, overlay_area);

        let popup_block = Block::default()
            .title(Span::styled(
                " XÁC NHẬN HỦY BỎ TÁC VỤ ",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));

        let text = vec![
            Line::from(vec![
                Span::raw("Bạn có chắc chắn muốn dừng và hủy bỏ tác vụ sau:"),
            ]),
            Line::from(vec![
                Span::styled(
                    format!(" {}", job.name),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" [Enter] Đồng ý ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" | "),
                Span::styled(" [Esc] Hủy bỏ ", Style::default().fg(Color::Gray)),
            ]),
        ];

        let paragraph = Paragraph::new(text)
            .block(popup_block)
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(paragraph, overlay_area);
    }
}

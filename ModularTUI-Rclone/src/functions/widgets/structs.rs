use serde::{Deserialize, Serialize};
use std::collections::{HashSet, HashMap, BTreeMap};
use ratatui::layout::Rect;

// ==========================================
// 1. Menu State
// ==========================================
pub struct MenuState {
    pub selected_idx: usize,
    pub options: Vec<&'static str>,
}

impl MenuState {
    pub fn new() -> Self {
        MenuState {
            selected_idx: 0,
            options: vec![
                "menu_1",
                "menu_2",
                "menu_3",
                "menu_4",
                "menu_5",
                "menu_6",
                "menu_install_dep",
                "menu_7",
            ],
        }
    }

    pub fn next(&mut self) {
        self.selected_idx = (self.selected_idx + 1) % self.options.len();
    }

    pub fn prev(&mut self) {
        if self.selected_idx == 0 {
            self.selected_idx = self.options.len() - 1;
        } else {
            self.selected_idx -= 1;
        }
    }
}

// ==========================================
// 2. Connection Manager State
// ==========================================
#[derive(Debug, Clone, PartialEq)]
pub enum WizardState {
    None,
    SelectProviders {
        providers: Vec<(String, String, bool)>, // (Name, Description, Checked)
        selected_idx: usize,
        scroll_offset: usize,
    },
    InputRemoteName {
        provider: String,
        input_buffer: String,
        selected_providers: Vec<String>,
    },
    SelectAuthMode {
        provider: String,
        remote_name: String,
        selected_idx: usize, // 0: Simple OAuth, 1: Headless OAuth, 2: Advanced Setup
        selected_providers: Vec<String>,
    },
    HeadlessOAuthInput {
        provider: String,
        remote_name: String,
        client_id: String,
        client_secret: String,
        token_input: String,
        focused_idx: usize, // 0: client_id, 1: client_secret, 2: token_input
        selected_providers: Vec<String>,
    },
    SimpleOAuthLoop {
        provider: String,
        remote_name: String,
        auth_url: String,
        selected_providers: Vec<String>,
    },
    AdvancedSetup {
        provider: String,
        remote_name: String,
        fields: Vec<(String, String, String, Vec<String>)>, // (Tên trường, Mô tả, Giá trị, Lựa chọn)
        selected_field_idx: usize,
        scroll_offset: usize,
        is_editing: bool,
        input_buffer: String,
        selected_providers: Vec<String>,
        active_tab: usize,
    },
    EditSetup {
        remote_name: String,
        provider: String,
        fields: Vec<(String, String, String, Vec<String>)>, // (Tên trường, Mô tả, Giá trị, Lựa chọn)
        selected_idx: usize,
        scroll_offset: usize,
        is_editing: bool,
        input_buffer: String,
        adding_new_key: bool,
        new_key_buffer: String,
        active_tab: usize,
    },
    ShowFeatures {
        remote_name: String,
        features: Vec<(String, bool)>,
        union_remotes_features: Option<Vec<(String, Vec<(String, bool)>)>>,
    },
    ImportConfigInput {
        input_buffer: String,
    },
}

pub struct ConnectionState {
    pub remotes: Vec<String>,
    pub selected_idx: usize,
    pub wizard: WizardState,
    pub error_message: Option<String>,
    pub info_message: Option<String>,
    pub remote_statuses: HashMap<String, String>,
    pub edit_cursor_idx: usize,
}

impl ConnectionState {
    pub fn new() -> Self {
        ConnectionState {
            remotes: Vec::new(),
            selected_idx: 0,
            wizard: WizardState::None,
            error_message: None,
            info_message: None,
            remote_statuses: HashMap::new(),
            edit_cursor_idx: 0,
        }
    }

    pub fn next(&mut self) {
        if !self.remotes.is_empty() {
            self.selected_idx = (self.selected_idx + 1) % self.remotes.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.remotes.is_empty() {
            if self.selected_idx == 0 {
                self.selected_idx = self.remotes.len() - 1;
            } else {
                self.selected_idx -= 1;
            }
        }
    }
}

// ==========================================
// 3. File Explorer State
// ==========================================
#[derive(Debug, Clone, PartialEq)]
pub struct FileItem {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
    pub mod_time: String,
    pub id: Option<String>,
}

pub struct ExplorerPane {
    pub remote: String, // Rỗng nghĩa là Local Path, ngược lại là tên Remote
    pub path: String,   // Đường dẫn hiện tại
    pub items: Vec<FileItem>,
    pub selected_idx: usize,
    pub scroll_offset: usize,
    pub loading: bool,
    pub selected_names: HashSet<String>,
    pub shift_anchor: Option<usize>,
    pub alt_anchor: Option<usize>,
    pub shift_active: bool,
    pub alt_active: bool,
}

impl ExplorerPane {
    pub fn new(remote: &str) -> Self {
        ExplorerPane {
            remote: remote.to_string(),
            path: if remote.is_empty() {
                crate::functions::get_home_dir()
            } else {
                "".to_string()
            },
            items: Vec::new(),
            selected_idx: 0,
            scroll_offset: 0,
            loading: false,
            selected_names: HashSet::new(),
            shift_anchor: None,
            alt_anchor: None,
            shift_active: false,
            alt_active: false,
        }
    }

    pub fn next(&mut self) {
        if !self.items.is_empty() {
            self.selected_idx = (self.selected_idx + 1) % self.items.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.items.is_empty() {
            if self.selected_idx == 0 {
                self.selected_idx = self.items.len() - 1;
            } else {
                self.selected_idx -= 1;
            }
        }
    }

    pub fn adjust_scroll(&mut self, height: usize) {
        if self.items.is_empty() {
            self.scroll_offset = 0;
            return;
        }
        if self.selected_idx < self.scroll_offset {
            self.scroll_offset = self.selected_idx;
        } else if self.selected_idx >= self.scroll_offset + height {
            self.scroll_offset = self.selected_idx - height + 1;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActivePane {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FallbackAction {
    MoveNative { src: String, dest: String },
    MoveCopyDelete { src: String, dest: String },
    MoveLocalTransfer { src: String, dest: String },
    CopyNative { src: String, dest: String, use_checksum: bool },
    CopyLocalTransfer { src: String, dest: String, use_checksum: bool },
    DeleteNative { target: String, is_dir: bool },
    DeleteIndividual { target: String },
    RenameCopyDelete { src: String, dest: String, is_dir: bool },
    RenameLocalTransfer { src: String, dest: String, is_dir: bool },
    CleanupCloud { fs: String },
    Rmdir { fs: String, remote: String },
    Rmdirs { fs: String, remote: String },
    Cancel,
    PermissionCancel,
    PermissionCopyAsMuchAsPossible { src: String, dest: String, is_dir: bool, restricted_files: Vec<String>, use_checksum: bool },
    PermissionRestrictedCopy { src: String, dest: String, is_dir: bool, restricted_files: Vec<String>, use_checksum: bool },
    MultiPermissionCopyAsMuchAsPossible { items: Vec<ClipboardItem>, dest_remote: String, dest_path: String, restricted_files: Vec<String>, use_checksum: bool },
    MultiPermissionRestrictedCopy { items: Vec<ClipboardItem>, dest_remote: String, dest_path: String, restricted_files: Vec<String>, use_checksum: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExplorerPopup {
    None,
    InputNewFolder {
        input_buffer: String,
    },
    CopyProgress {
        src: String,
        dest: String,
        pct: f64,
        job_id: Option<i64>,
    },
    MoveProgress {
        src: String,
        dest: String,
        pct: f64,
        job_id: Option<i64>,
    },
    SelectRemote {
        remotes: Vec<String>,
        selected_idx: usize,
    },
    ConfirmFallback {
        title: String,
        options: Vec<String>,
        selected_idx: usize,
        actions: Vec<FallbackAction>,
        restricted_files: Option<Vec<String>>,
        restricted_scroll: usize,
        focus_files: bool,
    },
    InputRename {
        old_name: String,
        input_buffer: String,
        is_dir: bool,
    },
    SpecialActionsMenu {
        selected_idx: usize,
    },
    ViewFile {
        file_name: String,
        content: Vec<String>,
        scroll_offset: usize,
    },
    ChecksumTypeSelect {
        selected_idx: usize,
    },
    CryptdecodeForm {
        remote_input: String,
        encrypted_input: String,
        is_remote_focused: bool,
        output_result: Option<String>,
    },
    DecompressModeSelect {
        archive_path: String,
        selected_idx: usize,
    },
    DecompressPathInput {
        archive_path: String,
        selected_idx: usize,
    },
    DecompressPathManualInput {
        archive_path: String,
        input_buffer: String,
    },
    TuiExplorerSelector {
        archive_path: String,
        remote: String,
        path: String,
        items: Vec<FileItem>,
        selected_idx: usize,
        scroll_offset: usize,
        loading: bool,
    },
    SpecialActionMessage {
        title: String,
        message: String,
    },
    InputPasteRename {
        input_buffer: String,
    },
    InputSharedLink {
        input_buffer: String,
    },
    SelectBaseRemote {
        remotes: Vec<String>,
        selected_idx: usize,
        folder_id: String,
    },
    PermissionScanning {
        src: String,
        dest: String,
        is_dir: bool,
        scanned_count: usize,
        total_files: usize,
        restricted_count: usize,
    },
    DedupeModeSelect {
        by_hash: bool,
        selected_idx: usize,
    },
    CopyModeSelect {
        src: String,
        dest: String,
        is_dir: bool,
        is_multi: bool,
        clipboard_items: Option<Vec<ClipboardItem>>,
        action_type: String, // "copy" or "sync"
        selected_idx: usize,
    },
    MergeSimilarDestinationSelect {
        folders: Vec<FileItem>,
        selected_idx: usize,
    },
    MergeSimilarScanning {
        folders_count: usize,
        scanned_count: usize,
    },
    MergeSimilarPreview {
        summary_report: Vec<String>,
        tree_root: TreeNode,
        expanded_paths: HashSet<String>,
        selected_rel_path: String,
        scroll_offset: usize,
        folders: Vec<FileItem>,
        destination_idx: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    pub name: String,
    pub rel_path: String,
    pub is_dir: bool,
    pub action: Option<String>,
    pub children: BTreeMap<String, TreeNode>,
}

impl TreeNode {
    pub fn new(name: String, rel_path: String, is_dir: bool) -> Self {
        TreeNode {
            name,
            rel_path,
            is_dir,
            action: None,
            children: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, path_parts: &[&str], is_dir: bool, action: Option<String>) {
        if path_parts.is_empty() {
            return;
        }
        let current_name = path_parts[0];
        let is_last = path_parts.len() == 1;

        let child_rel_path = if self.rel_path.is_empty() {
            current_name.to_string()
        } else {
            format!("{}/{}", self.rel_path, current_name)
        };

        let child = self.children.entry(current_name.to_string()).or_insert_with(|| {
            TreeNode::new(current_name.to_string(), child_rel_path, if is_last { is_dir } else { true })
        });

        if is_last {
            child.is_dir = is_dir;
            if action.is_some() {
                child.action = action;
            }
        } else {
            child.insert(&path_parts[1..], is_dir, action);
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClipboardItem {
    pub remote: String,
    pub path: String,
    pub name: String,
    pub is_dir: bool,
}

pub struct ExplorerState {
    pub left_pane: ExplorerPane,
    pub right_pane: ExplorerPane,
    pub active_pane: ActivePane,
    pub popup: ExplorerPopup,
    pub notification: Option<(String, String)>,
    pub clipboard: Option<ClipboardItem>,
    pub clipboard_items: Option<Vec<ClipboardItem>>,
    pub edit_cursor_idx: usize,
}

impl ExplorerState {
    pub fn new() -> Self {
        ExplorerState {
            left_pane: ExplorerPane::new(""),
            right_pane: ExplorerPane::new(""),
            active_pane: ActivePane::Left,
            popup: ExplorerPopup::None,
            notification: None,
            clipboard: None,
            clipboard_items: None,
            edit_cursor_idx: 0,
        }
    }

    pub fn get_active_pane(&self) -> &ExplorerPane {
        match self.active_pane {
            ActivePane::Left => &self.left_pane,
            ActivePane::Right => &self.right_pane,
        }
    }

    pub fn get_active_pane_mut(&mut self) -> &mut ExplorerPane {
        match self.active_pane {
            ActivePane::Left => &mut self.left_pane,
            ActivePane::Right => &mut self.right_pane,
        }
    }

    pub fn toggle_pane(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::Left => ActivePane::Right,
            ActivePane::Right => ActivePane::Left,
        };
    }
}

// ==========================================
// 4. Job Monitor State
// ==========================================
#[derive(Debug, Clone, PartialEq)]
pub enum MonitorPane {
    ActiveJobs,
    PendingJobs,
    FailedFiles,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileTask {
    pub name: String,
    pub size: u64,
    pub status: TaskStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TaskStatus {
    Pending,
    Transferring,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActiveOperation {
    pub id: String,
    pub action_type: String, // "copy", "move", "delete", "purge", etc.
    pub src: String,
    pub dest: String,
    pub items: Vec<String>,
    pub is_dir: bool,
    pub use_checksum: bool,
    pub is_copy: bool,
    pub completed_items: Option<Vec<String>>,
    pub tasks: Option<Vec<FileTask>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreOperation {
    pub id: String,
    pub action_type: String, // "copy" or "move"
    pub src: String,
    pub dest: String,
    pub is_dir: bool,
    pub use_checksum: bool,
    pub items: Option<Vec<ClipboardItem>>,
    pub scanned_count: usize,
    pub total_files: usize,
    pub restricted_count: usize,
    pub status: String, // "scanning", "done", "failed", "bypassed"
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
    pub items: Option<Vec<ClipboardItem>>,
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
    pub direct_total_children: usize,
    pub direct_completed_children: usize,
    pub recursive_total_size: u64,
    pub recursive_completed_bytes: u64,
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
    pub selected_job_idx: usize,
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
    pub bottleneck_reason: String,
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
    direct_total_children: usize,
    direct_completed_children: usize,
    recursive_total_size: u64,
    recursive_completed_bytes: u64,
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
            direct_total_children: 0,
            direct_completed_children: 0,
            recursive_total_size: 0,
            recursive_completed_bytes: 0,
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
            node.recursive_total_size = file.size;
            node.recursive_completed_bytes = file.bytes;
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

    fn aggregate_totals(&mut self) -> (usize, usize, u64, u64, bool) {
        if !self.is_dir {
            let is_completed = self.status == "completed" || self.status == "skipped";
            let comp_bytes = if is_completed { self.size } else { self.bytes };
            self.bytes = comp_bytes;
            self.recursive_completed_bytes = comp_bytes;
            self.percentage = if self.size > 0 {
                ((comp_bytes as f64 / self.size as f64) * 100.0) as u16
            } else if is_completed {
                100
            } else {
                0
            };
            return (0, 0, self.size, comp_bytes, is_completed);
        }

        let mut total_size = 0;
        let mut total_bytes = 0;
        let mut completed_children = 0;
        let total_children = self.children.len();

        for child in self.children.values_mut() {
            let (_, _, c_size, c_bytes, c_is_completed) = child.aggregate_totals();
            total_size += c_size;
            total_bytes += c_bytes;
            if c_is_completed {
                completed_children += 1;
            }
        }

        self.direct_total_children = total_children;
        self.direct_completed_children = completed_children;
        self.recursive_total_size = total_size;
        self.recursive_completed_bytes = total_bytes;

        let is_all_completed = total_children > 0 && completed_children == total_children;
        
        self.size = total_size;
        self.bytes = total_bytes;
        self.percentage = if total_size > 0 {
            ((total_bytes as f64 / total_size as f64) * 100.0) as u16
        } else if is_all_completed {
            100
        } else {
            0
        };

        (total_children, completed_children, total_size, total_bytes, is_all_completed)
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
            direct_total_children: child.direct_total_children,
            direct_completed_children: child.direct_completed_children,
            recursive_total_size: child.recursive_total_size,
            recursive_completed_bytes: child.recursive_completed_bytes,
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
            selected_job_idx: 0,
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
            direct_total_children: bg_jobs.len(),
            direct_completed_children: bg_jobs.iter().filter(|j| j.percentage >= 100).count(),
            recursive_total_size: total_size,
            recursive_completed_bytes: total_bytes,
        };
        self.visible_nodes.push(bg_folder_node);

        if bg_folder_expanded {
            for job in bg_jobs {
                let job_id_str = format!("job/{}", job.job_id.unwrap_or(0));
                let job_expanded = self.expanded_paths.contains(&job_id_str);
                
                let mut root = TempTreeNode::new(String::new(), true, String::new());
                for file in &job.files {
                    let parts: Vec<&str> = file.path.split('/').filter(|s| !s.is_empty()).collect();
                    root.insert(&parts, file);
                }
                root.aggregate_totals();

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
                    direct_total_children: root.direct_total_children,
                    direct_completed_children: root.direct_completed_children,
                    recursive_total_size: root.recursive_total_size,
                    recursive_completed_bytes: root.recursive_completed_bytes,
                };
                self.visible_nodes.push(job_node);

                if job_expanded {
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

        // 2. Hiển thị từng tác vụ hoạt động từ active_ops như một thư mục tiến trình
        let active_ops = crate::app::load_active_operations();
        let pre_ops = crate::app::load_pre_operations();
        for op in &active_ops {
            let is_scanning = pre_ops.iter().any(|po| po.id == op.id && po.status == "scanning");
            let op_folder_id = format!("op/{}", op.id);
            let op_expanded = self.expanded_paths.contains(&op_folder_id);
            let task_name = if is_scanning {
                let trans_checking = crate::functions::translate("mon_action_checking");
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

            let mut root = TempTreeNode::new(String::new(), true, String::new());
            
            let task_map = op.tasks.as_ref().map(|tasks| {
                tasks.iter()
                    .map(|t| (t.name.as_str(), t))
                    .collect::<std::collections::HashMap<&str, &crate::functions::widgets::structs::FileTask>>()
            });

            let completed_iter = op.completed_items.as_ref()
                .map(|v| v.iter().map(|item| (item, true)))
                .into_iter()
                .flatten();
            let queued_iter = op.items.iter().map(|item| (item, false));
            
            for (item, is_completed) in completed_iter.chain(queued_iter) {
                let size = task_map.as_ref()
                    .and_then(|map| map.get(item.as_str()))
                    .map(|t| t.size)
                    .unwrap_or(0);

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
                let mut bytes = 0;

                if let Some(ref map) = task_map {
                    if let Some(task) = map.get(item.as_str()) {
                        status = match task.status {
                            crate::functions::TaskStatus::Pending => {
                                if is_scanning {
                                    "checking".to_string()
                                } else {
                                    "queued".to_string()
                                }
                            }
                            crate::functions::TaskStatus::Transferring => "running".to_string(),
                            crate::functions::TaskStatus::Completed => "completed".to_string(),
                            crate::functions::TaskStatus::Failed => "failed".to_string(),
                            crate::functions::TaskStatus::Skipped => "skipped".to_string(),
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
                    bytes = size;
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

            root.aggregate_totals();

            let op_node = VisibleNode {
                id: op_folder_id.clone(),
                name: format!("{} - {}", op.id, task_name),
                is_dir: true,
                is_job: false,
                depth: 0,
                expanded: op_expanded,
                status: if is_scanning { "checking".to_string() } else { String::new() },
                size: root.recursive_total_size,
                bytes: root.recursive_completed_bytes,
                speed: 0,
                eta: -1,
                percentage: root.percentage,
                job_id: None,
                job_name: op.id.clone(),
                error: String::new(),
                direct_total_children: root.direct_total_children,
                direct_completed_children: root.direct_completed_children,
                recursive_total_size: root.recursive_total_size,
                recursive_completed_bytes: root.recursive_completed_bytes,
            };
            self.visible_nodes.push(op_node);
            
            if op_expanded {
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
            
            let mut root = TempTreeNode::new(String::new(), true, String::new());
            for file in &job.files {
                let parts: Vec<&str> = file.path.split('/').filter(|s| !s.is_empty()).collect();
                root.insert(&parts, file);
            }
            root.aggregate_totals();

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
                direct_total_children: root.direct_total_children,
                direct_completed_children: root.direct_completed_children,
                recursive_total_size: root.recursive_total_size,
                recursive_completed_bytes: root.recursive_completed_bytes,
            };
            self.visible_nodes.push(job_node);

            if job_expanded {
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

// ==========================================
// 5. Config Profile State
// ==========================================
#[derive(Debug, Clone, PartialEq)]
pub enum ImportWizardState {
    None,
    InputProfileName {
        input_buffer: String,
    },
    SelectImportType {
        profile_name: String,
        selected_idx: usize,
    },
    InputSource {
        profile_name: String,
        import_type: usize,
        input_buffer: String,
    },
    ConfirmImportOverwrite {
        profile_name: String,
        source_path_or_url: String,
        import_type: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportPopupState {
    None,
    ConfirmOverwrite { profile_name: String },
    Success { path: String },
}

pub struct ProfileState {
    pub profiles: Vec<(String, String)>,
    pub selected_idx: usize,
    pub wizard: ImportWizardState,
    pub export_popup: ExportPopupState,
    pub error_message: Option<String>,
}

impl ProfileState {
    pub fn new() -> Self {
        ProfileState {
            profiles: Vec::new(),
            selected_idx: 0,
            wizard: ImportWizardState::None,
            export_popup: ExportPopupState::None,
            error_message: None,
        }
    }

    pub fn next(&mut self) {
        if !self.profiles.is_empty() {
            self.selected_idx = (self.selected_idx + 1) % self.profiles.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.profiles.is_empty() {
            if self.selected_idx == 0 {
                self.selected_idx = self.profiles.len() - 1;
            } else {
                self.selected_idx -= 1;
            }
        }
    }
}

// ==========================================
// 6. Services & Mounts State
// ==========================================
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceType {
    Mount,
    NfsMount,
    WebGui,
    Serve,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServicesWizardState {
    None,
    AskMode {
        service_type: ServiceType,
        selected_idx: usize,
    },
    SelectRemote {
        service_type: ServiceType,
        remotes: Vec<String>,
        selected_idx: usize,
        is_simple_terminal: bool,
        is_simple_gui: bool,
    },
    InputPath {
        service_type: ServiceType,
        remote: String,
        input_buffer: String,
        is_simple_terminal: bool,
    },
    GuiSelectPath {
        service_type: ServiceType,
        remote: String,
        current_path: String,
        items: Vec<FileItem>,
        selected_idx: usize,
        loading: bool,
        error_msg: Option<String>,
        creating_folder: Option<String>,
    },
    GuiSelectLocalPath {
        service_type: ServiceType,
        remote: String,
        remote_path: String,
        current_path: String,
        items: Vec<FileItem>,
        selected_idx: usize,
        loading: bool,
        error_msg: Option<String>,
        creating_folder: Option<String>,
    },
    SelectProtocol {
        remote: String,
        path: String,
        selected_idx: usize,
    },
    AskFlags {
        service_type: ServiceType,
        remote: String,
        path: String,
        protocol: Option<String>,
        flags: Vec<(String, String, String, String)>,
        current_flag_idx: usize,
        input_buffer: String,
        is_simple_terminal: bool,
        is_editing: bool,
    },
    SelectSystemdAction {
        service_name: String,
        file_path: String,
        is_user: bool,
        selected_idx: usize,
    },
    EditSystemdService {
        service_name: String,
        file_path: String,
        is_user: bool,
        fields: Vec<(String, String, String, Vec<String>)>,
        selected_idx: usize,
        scroll_offset: usize,
        is_editing: bool,
        input_buffer: String,
        active_tab: usize,
        adding_new_key: bool,
        new_key_buffer: String,
    },
    CreateSystemdService {
        fields: Vec<(String, String, String, Vec<String>)>,
        selected_idx: usize,
        scroll_offset: usize,
        is_editing: bool,
        input_buffer: String,
        active_tab: usize,
        adding_new_key: bool,
        new_key_buffer: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveService {
    pub service_type_str: String,
    pub remote: String,
    pub path: String,
    pub pid: u32,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemdServiceInfo {
    pub name: String,
    pub file_path: String,
    pub is_user: bool,
    pub active_status: String,
    pub sub_state: String,
    pub description: String,
}

pub struct ServicesState {
    pub menu_options: Vec<&'static str>,
    pub selected_menu_idx: usize,
    pub wizard: ServicesWizardState,
    pub active_services: Vec<ActiveService>,
    pub selected_active_idx: usize,
    pub active_focus: usize, // 0: menu, 1: TUI active, 2: Systemd
    pub systemd_services: Vec<SystemdServiceInfo>,
    pub selected_systemd_idx: usize,
    pub error_message: Option<String>,
    pub info_message: Option<String>,
    pub all_remotes: Vec<String>,
    pub selecting_remote: Option<usize>,
    pub edit_cursor_idx: usize,
}

impl ServicesState {
    pub fn new() -> Self {
        ServicesState {
            menu_options: vec![
                "srv_opt_mount",
                "srv_opt_nfsmount",
                "srv_opt_gui",
                "srv_opt_serve",
            ],
            selected_menu_idx: 0,
            wizard: ServicesWizardState::None,
            active_services: Vec::new(),
            selected_active_idx: 0,
            active_focus: 0,
            systemd_services: Vec::new(),
            selected_systemd_idx: 0,
            error_message: None,
            info_message: None,
            all_remotes: Vec::new(),
            selecting_remote: None,
            edit_cursor_idx: 0,
        }
    }

    pub fn next_menu(&mut self) {
        self.selected_menu_idx = (self.selected_menu_idx + 1) % self.menu_options.len();
    }

    pub fn prev_menu(&mut self) {
        if self.selected_menu_idx == 0 {
            self.selected_menu_idx = self.menu_options.len() - 1;
        } else {
            self.selected_menu_idx -= 1;
        }
    }

    pub fn next_active(&mut self) {
        if !self.active_services.is_empty() {
            self.selected_active_idx = (self.selected_active_idx + 1) % self.active_services.len();
        }
    }

    pub fn prev_active(&mut self) {
        if !self.active_services.is_empty() {
            if self.selected_active_idx == 0 {
                self.selected_active_idx = self.active_services.len() - 1;
            } else {
                self.selected_active_idx -= 1;
            }
        }
    }

    pub fn next_systemd(&mut self) {
        if !self.systemd_services.is_empty() {
            self.selected_systemd_idx =
                (self.selected_systemd_idx + 1) % self.systemd_services.len();
        }
    }

    pub fn prev_systemd(&mut self) {
        if !self.systemd_services.is_empty() {
            if self.selected_systemd_idx == 0 {
                self.selected_systemd_idx = self.systemd_services.len() - 1;
            } else {
                self.selected_systemd_idx -= 1;
            }
        }
    }
}

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

#[derive(Debug, Clone, PartialEq)]
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
}

pub struct MonitorState {
    pub speed: f64,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub active_jobs: Vec<TransferJob>,
    pub selected_job_idx: usize,
    pub history: Vec<String>,
    pub confirm_stop_job: Option<TransferJob>,
    pub failed_files: Vec<FailedCopyItem>,
    pub pending_jobs: Vec<PendingCopyJob>,
    pub selected_pending_idx: usize,
    pub active_pane: MonitorPane,
}

impl MonitorState {
    pub fn new() -> Self {
        MonitorState {
            speed: 0.0,
            bytes_transferred: 0,
            total_bytes: 0,
            active_jobs: Vec::new(),
            selected_job_idx: 0,
            history: Vec::new(),
            confirm_stop_job: None,
            failed_files: Vec::new(),
            pending_jobs: Vec::new(),
            selected_pending_idx: 0,
            active_pane: MonitorPane::ActiveJobs,
        }
    }

    pub fn next(&mut self) {
        match self.active_pane {
            MonitorPane::ActiveJobs => {
                if !self.active_jobs.is_empty() {
                    self.selected_job_idx = (self.selected_job_idx + 1) % self.active_jobs.len();
                }
            }
            MonitorPane::PendingJobs => {
                if !self.pending_jobs.is_empty() {
                    self.selected_pending_idx = (self.selected_pending_idx + 1) % self.pending_jobs.len();
                }
            }
        }
    }

    pub fn prev(&mut self) {
        match self.active_pane {
            MonitorPane::ActiveJobs => {
                if !self.active_jobs.is_empty() {
                    if self.selected_job_idx == 0 {
                        self.selected_job_idx = self.active_jobs.len() - 1;
                    } else {
                        self.selected_job_idx -= 1;
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

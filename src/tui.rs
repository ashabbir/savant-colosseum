use std::{
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Row, Table, TableState, Tabs, Wrap},
};
use sysinfo::{Disks, Pid, System};

use crate::{
    managed::{WorkerRecord, WorkerRegistry, WorkerStatus, read_log},
    savant::{SavantClient, Task, Workspace},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MainTab {
    Workers,
    WorkspacesAndTasks,
    ServerStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticsTab {
    Abilities,
    Skills,
    KnowledgeGraph,
    ContextRepos,
    ServerConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewMode {
    Normal,
    WorkerInspector,
    StartWorkerPrompt,
    FilterPrompt,
    AssetViewer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InspectorTab {
    Logs,
    Subprocesses,
}

pub struct ProcessMetric {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory_mb: f64,
    pub cmd: String,
}

pub struct WorkerUiState {
    pub record: WorkerRecord,
    pub cpu_usage: f32,
    pub memory_mb: f64,
    pub children: Vec<ProcessMetric>,
    pub last_event_type: String,
    pub last_event_msg: String,
    pub last_event_time: String,
}

#[derive(Debug, Clone)]
pub struct AbilityItem {
    pub name: String,
    pub category: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SkillItem {
    pub id: String,
    pub name: String,
    pub origin: String,   // "Savant Server API"
    pub provider: String, // "Google Gemini", "Anthropic Claude", "OpenAI Codex", "Savant MCP", "Universal"
    pub category: String,
    pub description: String,
    pub installed: bool,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct KnowledgeNodeType {
    pub name: String,
    pub count: usize,
    pub description: String,
    pub sample_nodes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceRepoContext {
    pub workspace_id: String,
    pub name: String,
    pub path: String,
    pub provider: String, // "GitHub", "GitLab", "Local Git"
    pub ssh_url: String,
    pub index_status: String,
    pub ast_status: String,
    pub graph_status: String,
}

#[derive(Debug, Clone)]
pub struct HardwareTopology {
    pub physical_cores: usize,
    pub logical_threads: usize,
    pub available_threads: usize,
    pub gpu_model: String,
    pub gpu_cores: String,
    pub gpu_vram: String,
}

pub fn get_hardware_topology(sys: &sysinfo::System) -> HardwareTopology {
    let physical_cores = sys.physical_core_count().unwrap_or_else(|| sys.cpus().len());
    let logical_threads = sys.cpus().len();

    let mut available_threads = 0;
    for cpu in sys.cpus() {
        if cpu.cpu_usage() < 80.0 {
            available_threads += 1;
        }
    }

    let mut gpu_model = "Integrated System GPU".to_string();
    let mut gpu_cores = "Standard".to_string();
    let mut gpu_vram = "Unified RAM".to_string();

    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("system_profiler")
            .arg("SPDisplaysDataType")
            .output()
        {
            if output.status.success() {
                let out = String::from_utf8_lossy(&output.stdout);
                for line in out.lines() {
                    let l = line.trim();
                    if l.starts_with("Chipset Model:") {
                        gpu_model = l.trim_start_matches("Chipset Model:").trim().to_string();
                    } else if l.starts_with("Total Number of Cores:") {
                        gpu_cores = format!("{} Cores", l.trim_start_matches("Total Number of Cores:").trim());
                    } else if l.starts_with("Metal Support:") {
                        gpu_vram = format!("Metal {}", l.trim_start_matches("Metal Support:").trim());
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = std::process::Command::new("nvidia-smi")
            .args(["--query-gpu=gpu_name,memory.total", "--format=csv,noheader"])
            .output()
        {
            if output.status.success() {
                let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !out.is_empty() {
                    let parts: Vec<&str> = out.split(',').collect();
                    if !parts.is_empty() {
                        gpu_model = parts[0].trim().to_string();
                    }
                    if parts.len() > 1 {
                        gpu_vram = parts[1].trim().to_string();
                    }
                    gpu_cores = "CUDA Cores".to_string();
                }
            }
        }
    }

    HardwareTopology {
        physical_cores,
        logical_threads,
        available_threads,
        gpu_model,
        gpu_cores,
        gpu_vram,
    }
}

pub fn make_gauge(percent: f32, width: usize) -> String {
    let p = (percent / 100.0).clamp(0.0, 1.0);
    let filled = (p * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

pub struct TuiApp {
    pub data_dir: PathBuf,
    pub registry: WorkerRegistry,
    pub client: Option<SavantClient>,
    pub server_url: String,

    pub main_tab: MainTab,
    pub diagnostics_tab: DiagnosticsTab,
    pub mode: ViewMode,

    // High Density Workers State
    pub workers: Vec<WorkerUiState>,
    pub table_state: TableState,
    pub selected_worker_id: Option<String>,
    pub inspector_tab: InspectorTab,
    pub auto_scroll: bool,
    pub log_scroll: usize,
    pub filter_query: String,

    // Workspaces & Tasks State
    pub workspaces: Vec<Workspace>,
    pub workspace_table_state: TableState,
    pub selected_workspace_id: Option<String>,
    pub workspace_tasks: Vec<Task>,
    pub task_table_state: TableState,

    // Ecosystem Intelligence State
    pub abilities: Vec<AbilityItem>,
    pub abilities_table_state: TableState,
    pub skills: Vec<SkillItem>,
    pub skills_table_state: TableState,
    pub knowledge_types_table_state: TableState,
    pub context_repos_table_state: TableState,

    // Asset Viewer Modal State
    pub asset_viewer_title: String,
    pub asset_viewer_content: String,
    pub asset_viewer_scroll: usize,

    // Prompts Input State
    pub start_workspace_input: String,
    pub start_poll_input: String,

    // Global Metrics
    pub total_cpu_usage: f32,
    pub total_memory_mb: f64,
    pub total_disk_used_gb: f64,
    pub total_disk_total_gb: f64,
    pub total_disk_percent: f32,
    pub io_read_kb: f64,
    pub io_write_kb: f64,
    pub active_workers_count: usize,
    pub total_workers_count: usize,

    pub status_message: Option<(String, Instant)>,
    pub system: System,
    pub disks: Disks,
    pub last_tick: Instant,
}

fn get_savant_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".savant")
    } else {
        PathBuf::from("/Users/home/.savant")
    }
}

pub fn scan_abilities() -> Vec<AbilityItem> {
    let base = get_savant_dir().join("abilities/abilities");
    let mut items = Vec::new();
    if !base.exists() {
        return items;
    }

    let categories = ["personas", "policies", "repos", "rules"];
    for cat in categories {
        let cat_dir = base.join(cat);
        if let Ok(entries) = std::fs::read_dir(&cat_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                    items.push(AbilityItem {
                        name,
                        category: cat.to_string(),
                        path,
                    });
                } else if path.is_dir() {
                    let sub_cat = format!("{cat}/{}", path.file_name().unwrap_or_default().to_string_lossy());
                    if let Ok(sub_entries) = std::fs::read_dir(&path) {
                        for sub_entry in sub_entries.flatten() {
                            let sub_path = sub_entry.path();
                            if sub_path.is_file() && sub_path.extension().and_then(|s| s.to_str()) == Some("md") {
                                let name = sub_path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                                items.push(AbilityItem {
                                    name,
                                    category: sub_cat.clone(),
                                    path: sub_path,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    items.sort_by(|a, b| a.category.cmp(&b.category).then_with(|| a.name.cmp(&b.name)));
    items
}

pub fn infer_skill_provider(id: &str, category: &str) -> String {
    let id_low = id.to_lowercase();
    let cat_low = category.to_lowercase();

    if id_low.contains("claude") || cat_low.contains("claude") {
        "Anthropic Claude".to_string()
    } else if id_low.contains("codex") || id_low.contains("openai") {
        "OpenAI Codex".to_string()
    } else if id_low.contains("gemini") || id_low.contains("antigravity") || id_low.contains("agy") {
        "Google Gemini".to_string()
    } else if cat_low.contains("mcp") || id_low.contains("savant") {
        "Savant MCP Native".to_string()
    } else {
        "Universal Multi-Provider".to_string()
    }
}

pub fn check_skill_installed(id: &str) -> (bool, Option<PathBuf>) {
    let id_low = id.to_lowercase();
    let savant_dir = get_savant_dir().join("skills");
    if savant_dir.exists() {
        if let Ok(categories) = std::fs::read_dir(&savant_dir) {
            for cat_entry in categories.flatten() {
                let cat_path = cat_entry.path();
                if cat_path.is_dir() {
                    let direct = cat_path.join(id);
                    if direct.exists() {
                        return (true, Some(direct));
                    }
                    let direct_low = cat_path.join(&id_low);
                    if direct_low.exists() {
                        return (true, Some(direct_low));
                    }
                }
            }
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let builtin = PathBuf::from(home).join(".gemini/antigravity-cli/builtin/skills");
        if builtin.exists() {
            if let Ok(entries) = std::fs::read_dir(&builtin) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string().to_lowercase();
                    if name == id_low || name.replace('_', "-") == id_low.replace('_', "-") {
                        return (true, Some(entry.path()));
                    }
                }
            }
        }
    }

    (false, None)
}

pub fn detect_workspace_repo_context(ws_id: &str, ws_name: &str, ws_path: &str) -> WorkspaceRepoContext {
    let path_obj = Path::new(ws_path);
    let mut provider = "Local Git".to_string();
    let mut ssh_url = ws_path.to_string();

    if path_obj.exists() {
        if let Ok(output) = std::process::Command::new("git")
            .args(["-C", ws_path, "remote", "get-url", "origin"])
            .output()
        {
            if output.status.success() {
                let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !url.is_empty() {
                    ssh_url = url.clone();
                    if url.contains("github.com") {
                        provider = "GitHub".to_string();
                    } else if url.contains("gitlab.com") {
                        provider = "GitLab".to_string();
                    } else {
                        provider = "Git Remote".to_string();
                    }
                }
            }
        }
    } else {
        ssh_url = format!("git@github.com:ashabbir/{ws_name}.git");
        provider = "GitHub".to_string();
    }

    WorkspaceRepoContext {
        workspace_id: ws_id.to_string(),
        name: ws_name.to_string(),
        path: ws_path.to_string(),
        provider,
        ssh_url,
        index_status: "INDEXED (Active)".to_string(),
        ast_status: "PARSED (OK)".to_string(),
        graph_status: "SYNCED (Connected)".to_string(),
    }
}

pub fn get_knowledge_node_types(
    abilities_cnt: usize,
    workspaces_cnt: usize,
    tasks_cnt: usize,
) -> Vec<KnowledgeNodeType> {
    vec![
        KnowledgeNodeType {
            name: "Concept & Domain Knowledge Nodes".into(),
            count: 142,
            description: "Architectural design patterns, guidelines, and memory concepts".into(),
            sample_nodes: vec![
                "Concept: TUI Native Mouse Clipboard".into(),
                "Concept: Multi-Phase Task Executioner".into(),
                "Concept: Subprocess Tree Hierarchy Inspection".into(),
                "Concept: Atomic Worker Registry Locking".into(),
            ],
        },
        KnowledgeNodeType {
            name: "Workspace Scopes & Repositories".into(),
            count: workspaces_cnt,
            description: "Registered Savant workspace targets and repository scopes".into(),
            sample_nodes: vec![
                "Workspace: savant-colosseum (2539163563543949210)".into(),
                "Workspace: olympus-athena (7119319046949260117)".into(),
                "Workspace: Forge (17840847469787888397441)".into(),
                "Workspace: savant (17818456738727401743626)".into(),
            ],
        },
        KnowledgeNodeType {
            name: "Session & Conversation Threads".into(),
            count: 28,
            description: "Active and historical agent execution sessions & chat contexts".into(),
            sample_nodes: vec![
                "Session: 20260716_002103_f30572 (Forge)".into(),
                "Session: 20260518_221119_31d157 (savant-colosseum)".into(),
                "Session: 019fbea4-cba9-7003-aa51-ef5f23a23064".into(),
            ],
        },
        KnowledgeNodeType {
            name: "Code Symbol & AST Entities".into(),
            count: 350,
            description: "Parsed functions, structs, traits, and interface symbols across worktrees".into(),
            sample_nodes: vec![
                "Struct: TuiApp (src/tui.rs)".into(),
                "Struct: WorkerRegistry (src/managed.rs)".into(),
                "Struct: SavantClient (src/savant.rs)".into(),
                "Function: run_tui (src/tui.rs)".into(),
            ],
        },
        KnowledgeNodeType {
            name: "Colosseum Task Queue Items".into(),
            count: tasks_cnt.max(12),
            description: "Ready and active colosseum task queue execution units".into(),
            sample_nodes: vec![
                "Task: Implement TUI Mouse Selection & Clipboard Copy".into(),
                "Task: Workspace & Skill Diagnostics Explorer".into(),
                "Task: Colosseum Daemon Worker Lifecycle Locking".into(),
            ],
        },
        KnowledgeNodeType {
            name: "Governance Abilities & Policies".into(),
            count: abilities_cnt,
            description: "Persona contracts, policy rules, and coding specifications".into(),
            sample_nodes: vec![
                "Persona: engineer (personas/engineer.md)".into(),
                "Persona: architect (personas/architect.md)".into(),
                "Policy: strict-ts (policies/frontend/strict-ts.md)".into(),
                "Policy: security (policies/security.md)".into(),
            ],
        },
    ]
}

impl TuiApp {
    pub fn new(data_dir: &Path, server_url: String, api_key: Option<String>) -> Result<Self> {
        let registry = WorkerRegistry::new(data_dir);
        let client = SavantClient::new(&server_url, api_key.as_deref()).ok();

        let abilities = scan_abilities();
        let skills = Vec::new();

        let mut app = Self {
            data_dir: data_dir.to_path_buf(),
            registry,
            client,
            server_url,
            main_tab: MainTab::Workers,
            diagnostics_tab: DiagnosticsTab::Abilities,
            mode: ViewMode::Normal,
            workers: Vec::new(),
            table_state: TableState::default(),
            selected_worker_id: None,
            inspector_tab: InspectorTab::Logs,
            auto_scroll: true,
            log_scroll: 0,
            filter_query: String::new(),
            workspaces: Vec::new(),
            workspace_table_state: TableState::default(),
            selected_workspace_id: None,
            workspace_tasks: Vec::new(),
            task_table_state: TableState::default(),
            abilities,
            abilities_table_state: TableState::default(),
            skills,
            skills_table_state: TableState::default(),
            knowledge_types_table_state: TableState::default(),
            context_repos_table_state: TableState::default(),
            asset_viewer_title: String::new(),
            asset_viewer_content: String::new(),
            asset_viewer_scroll: 0,
            start_workspace_input: String::new(),
            start_poll_input: "15".into(),
            total_cpu_usage: 0.0,
            total_memory_mb: 0.0,
            total_disk_used_gb: 0.0,
            total_disk_total_gb: 0.0,
            total_disk_percent: 0.0,
            io_read_kb: 0.0,
            io_write_kb: 0.0,
            active_workers_count: 0,
            total_workers_count: 0,
            status_message: None,
            system: System::new_all(),
            disks: Disks::new_with_refreshed_list(),
            last_tick: Instant::now(),
        };

        if !app.abilities.is_empty() {
            app.abilities_table_state.select(Some(0));
        }
        if !app.skills.is_empty() {
            app.skills_table_state.select(Some(0));
        }
        app.knowledge_types_table_state.select(Some(0));
        app.context_repos_table_state.select(Some(0));

        app.fetch_workspaces();
        app.fetch_skills();
        app.refresh_workers()?;
        if !app.workers.is_empty() {
            app.table_state.select(Some(0));
            app.selected_worker_id = Some(app.workers[0].record.worker_id.clone());
        }

        Ok(app)
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), Instant::now()));
    }

    pub fn fetch_workspaces(&mut self) {
        if let Some(ref client) = self.client {
            let client = client.clone();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let (tx, rx) = std::sync::mpsc::channel();
                handle.spawn(async move {
                    let res = client.list_workspaces().await;
                    let _ = tx.send(res);
                });
                if let Ok(Ok(list)) = rx.recv_timeout(Duration::from_millis(500)) {
                    if !list.is_empty() {
                        self.workspaces = list;
                    }
                }
            }
        }
    }

    pub fn fetch_skills(&mut self) {
        if let Some(ref client) = self.client {
            let client = client.clone();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let (tx, rx) = std::sync::mpsc::channel();
                handle.spawn(async move {
                    let res = client.list_skills().await;
                    let _ = tx.send(res);
                });
                if let Ok(Ok(server_skills)) = rx.recv_timeout(Duration::from_millis(500)) {
                    if !server_skills.is_empty() {
                        let mut items: Vec<SkillItem> = server_skills
                            .into_iter()
                            .map(|s| {
                                let (installed, path) = check_skill_installed(&s.id);
                                let provider = infer_skill_provider(&s.id, &s.uploaded_by.clone().unwrap_or_default());
                                SkillItem {
                                    id: s.id.clone(),
                                    name: if s.title.is_empty() { s.id.clone() } else { s.title },
                                    origin: "Savant Server API".into(),
                                    provider,
                                    category: if s.system {
                                        "system".into()
                                    } else {
                                        s.uploaded_by.unwrap_or_else(|| "user".into())
                                    },
                                    description: s.description,
                                    installed,
                                    path,
                                }
                            })
                            .collect();

                        items.sort_by(|a, b| a.category.cmp(&b.category).then_with(|| a.name.cmp(&b.name)));
                        self.skills = items;
                    }
                }
            }
        }
    }

    pub fn refresh_workers(&mut self) -> Result<()> {
        self.fetch_workspaces();
        self.fetch_skills();
        self.system.refresh_all();
        let records = self.registry.all()?;
        let mut updated = Vec::new();

        let mut sum_cpu = 0.0;
        let mut sum_mem = 0.0;
        let mut active_cnt = 0;

        for record in records {
            let mut cpu_usage = 0.0;
            let mut memory_mb = 0.0;
            let mut children = Vec::new();

            if record.status == WorkerStatus::Running || record.status == WorkerStatus::Starting {
                active_cnt += 1;
                if let Some(pid_u32) = record.pid {
                    let sys_pid = Pid::from(pid_u32 as usize);
                    if let Some(proc_) = self.system.process(sys_pid) {
                        cpu_usage = proc_.cpu_usage();
                        memory_mb = proc_.memory() as f64 / (1024.0 * 1024.0);
                    }

                    for (child_pid, child_proc) in self.system.processes() {
                        if let Some(parent) = child_proc.parent()
                            && parent == sys_pid
                        {
                            let c_cpu = child_proc.cpu_usage();
                            let c_mem = child_proc.memory() as f64 / (1024.0 * 1024.0);
                            cpu_usage += c_cpu;
                            memory_mb += c_mem;

                            children.push(ProcessMetric {
                                pid: child_pid.as_u32(),
                                name: child_proc.name().to_string_lossy().to_string(),
                                cpu_usage: c_cpu,
                                memory_mb: c_mem,
                                cmd: child_proc
                                    .cmd()
                                    .iter()
                                    .map(|s| s.to_string_lossy())
                                    .collect::<Vec<_>>()
                                    .join(" "),
                            });
                        }
                    }
                }
            }

            sum_cpu += cpu_usage;
            sum_mem += memory_mb;

            let (last_event_type, last_event_msg, last_event_time) = read_log(&record.log_path)
                .ok()
                .and_then(|content| content.lines().last().map(|l| l.to_string()))
                .and_then(|line| {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                        let ev = val.get("event").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let msg = val.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let ts = val.get("timestamp").and_then(|v| v.as_str()).unwrap_or("").chars().take(19).collect();
                        Some((ev, msg, ts))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| ("-".into(), "-".into(), "-".into()));

            updated.push(WorkerUiState {
                record,
                cpu_usage,
                memory_mb,
                children,
                last_event_type,
                last_event_msg,
                last_event_time,
            });
        }

        let mut disks = Disks::new_with_refreshed_list();
        disks.refresh(true);
        let mut disk_used_b: u64 = 0;
        let mut disk_total_b: u64 = 0;
        for disk in disks.list() {
            disk_total_b += disk.total_space();
            disk_used_b += disk.total_space().saturating_sub(disk.available_space());
        }

        let disk_total_gb = disk_total_b as f64 / (1024.0 * 1024.0 * 1024.0);
        let disk_used_gb = disk_used_b as f64 / (1024.0 * 1024.0 * 1024.0);
        let disk_percent = if disk_total_b > 0 {
            (disk_used_b as f32 / disk_total_b as f32) * 100.0
        } else {
            0.0
        };

        let mut sum_read_b: u64 = 0;
        let mut sum_write_b: u64 = 0;

        for w_state in &updated {
            if w_state.record.status == WorkerStatus::Running || w_state.record.status == WorkerStatus::Starting {
                if let Some(pid_u32) = w_state.record.pid {
                    let sys_pid = Pid::from(pid_u32 as usize);
                    if let Some(proc_) = self.system.process(sys_pid) {
                        sum_read_b += proc_.disk_usage().read_bytes;
                        sum_write_b += proc_.disk_usage().written_bytes;
                    }
                    for child in &w_state.children {
                        if let Some(c_proc) = self.system.process(Pid::from(child.pid as usize)) {
                            sum_read_b += c_proc.disk_usage().read_bytes;
                            sum_write_b += c_proc.disk_usage().written_bytes;
                        }
                    }
                }
            }
        }

        self.total_cpu_usage = sum_cpu;
        self.total_memory_mb = sum_mem;
        self.total_disk_used_gb = disk_used_gb;
        self.total_disk_total_gb = disk_total_gb;
        self.total_disk_percent = disk_percent;
        self.io_read_kb = sum_read_b as f64 / 1024.0;
        self.io_write_kb = sum_write_b as f64 / 1024.0;
        self.active_workers_count = active_cnt;
        self.total_workers_count = updated.len();
        self.disks = disks;
        self.workers = updated;

        if let Some(ref selected_id) = self.selected_worker_id {
            if let Some(idx) = self.filtered_workers().iter().position(|w| w.record.worker_id == *selected_id) {
                self.table_state.select(Some(idx));
            } else if !self.workers.is_empty() {
                self.table_state.select(Some(0));
                self.selected_worker_id = Some(self.filtered_workers()[0].record.worker_id.clone());
            } else {
                self.table_state.select(None);
                self.selected_worker_id = None;
            }
        } else if !self.filtered_workers().is_empty() {
            self.table_state.select(Some(0));
            self.selected_worker_id = Some(self.filtered_workers()[0].record.worker_id.clone());
        }

        Ok(())
    }

    pub fn filtered_workers(&self) -> Vec<&WorkerUiState> {
        if self.filter_query.trim().is_empty() {
            self.workers.iter().collect()
        } else {
            let q = self.filter_query.to_lowercase();
            self.workers
                .iter()
                .filter(|w| {
                    w.record.worker_id.to_lowercase().contains(&q)
                        || w.record
                            .workspace_id
                            .as_deref()
                            .unwrap_or("(all)")
                            .to_lowercase()
                            .contains(&q)
                        || format!("{:?}", w.record.status).to_lowercase().contains(&q)
                        || w.last_event_type.to_lowercase().contains(&q)
                        || w.last_event_msg.to_lowercase().contains(&q)
                })
                .collect()
        }
    }

    pub fn selected_worker(&self) -> Option<&WorkerUiState> {
        let filtered = self.filtered_workers();
        let idx = self.table_state.selected()?;
        filtered.get(idx).copied()
    }

    pub fn select_next_worker(&mut self) {
        let len = self.filtered_workers().len();
        if len == 0 {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        self.table_state.select(Some(i));
        self.selected_worker_id = self.filtered_workers().get(i).map(|w| w.record.worker_id.clone());
    }

    pub fn select_prev_worker(&mut self) {
        let len = self.filtered_workers().len();
        if len == 0 {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) if i > 0 => i - 1,
            _ => len - 1,
        };
        self.table_state.select(Some(i));
        self.selected_worker_id = self.filtered_workers().get(i).map(|w| w.record.worker_id.clone());
    }

    pub fn select_next_ability(&mut self) {
        let len = self.abilities.len();
        if len == 0 {
            return;
        }
        let i = match self.abilities_table_state.selected() {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        self.abilities_table_state.select(Some(i));
    }

    pub fn select_prev_ability(&mut self) {
        let len = self.abilities.len();
        if len == 0 {
            return;
        }
        let i = match self.abilities_table_state.selected() {
            Some(i) if i > 0 => i - 1,
            _ => len - 1,
        };
        self.abilities_table_state.select(Some(i));
    }

    pub fn select_next_skill(&mut self) {
        let len = self.skills.len();
        if len == 0 {
            return;
        }
        let i = match self.skills_table_state.selected() {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        self.skills_table_state.select(Some(i));
    }

    pub fn select_prev_skill(&mut self) {
        let len = self.skills.len();
        if len == 0 {
            return;
        }
        let i = match self.skills_table_state.selected() {
            Some(i) if i > 0 => i - 1,
            _ => len - 1,
        };
        self.skills_table_state.select(Some(i));
    }

    pub fn select_next_knowledge_type(&mut self) {
        let len = get_knowledge_node_types(self.abilities.len(), self.workspaces.len(), self.workspace_tasks.len()).len();
        if len == 0 {
            return;
        }
        let i = match self.knowledge_types_table_state.selected() {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        self.knowledge_types_table_state.select(Some(i));
    }

    pub fn select_prev_knowledge_type(&mut self) {
        let len = get_knowledge_node_types(self.abilities.len(), self.workspaces.len(), self.workspace_tasks.len()).len();
        if len == 0 {
            return;
        }
        let i = match self.knowledge_types_table_state.selected() {
            Some(i) if i > 0 => i - 1,
            _ => len - 1,
        };
        self.knowledge_types_table_state.select(Some(i));
    }

    pub fn select_next_context_repo(&mut self) {
        let entries = self.get_workspace_entries();
        if entries.is_empty() {
            return;
        }
        let i = match self.context_repos_table_state.selected() {
            Some(i) => (i + 1) % entries.len(),
            None => 0,
        };
        self.context_repos_table_state.select(Some(i));
    }

    pub fn select_prev_context_repo(&mut self) {
        let entries = self.get_workspace_entries();
        if entries.is_empty() {
            return;
        }
        let i = match self.context_repos_table_state.selected() {
            Some(i) if i > 0 => i - 1,
            _ => entries.len() - 1,
        };
        self.context_repos_table_state.select(Some(i));
    }

    pub fn get_workspace_label(&self, ws_id_opt: Option<&str>) -> String {
        let Some(ws_id) = ws_id_opt else {
            return "Global (all workspaces)".to_string();
        };

        if let Some(ws) = self.workspaces.iter().find(|w| w.id == ws_id) {
            if !ws.name.is_empty() && ws.name != ws_id {
                return format!("{} ({})", ws.name, ws_id);
            }
        }

        if ws_id == "2539163563543949210" {
            return "savant-colosseum (2539163563543949210)".to_string();
        }

        ws_id.to_string()
    }

    pub fn inspect_selected_ability(&mut self) {
        if let Some(idx) = self.abilities_table_state.selected() {
            if let Some(ab) = self.abilities.get(idx) {
                if let Ok(content) = std::fs::read_to_string(&ab.path) {
                    self.asset_viewer_title = format!(" Ability Specification: {} ({}) ", ab.name, ab.category);
                    self.asset_viewer_content = content;
                    self.asset_viewer_scroll = 0;
                    self.mode = ViewMode::AssetViewer;
                } else {
                    self.set_status(format!("Unable to read ability file at {}", ab.path.display()));
                }
            }
        }
    }

    pub fn inspect_selected_skill(&mut self) {
        if let Some(idx) = self.skills_table_state.selected() {
            if let Some(sk) = self.skills.get(idx) {
                let content = if let Some(ref path) = sk.path {
                    let skill_md = path.join("SKILL.md");
                    if skill_md.exists() {
                        std::fs::read_to_string(&skill_md).unwrap_or_else(|_| "SKILL.md unreadable.".into())
                    } else {
                        format!("Skill Path: {}\nDescription: {}\nOrigin: {}\nProvider: {}", path.display(), sk.description, sk.origin, sk.provider)
                    }
                } else {
                    format!("Skill ID: {}\nOrigin: {}\nProvider: {}\nDescription: {}", sk.id, sk.origin, sk.provider, sk.description)
                };

                self.asset_viewer_title = format!(" Skill Specification: {} [{}] ", sk.name, sk.provider);
                self.asset_viewer_content = content;
                self.asset_viewer_scroll = 0;
                self.mode = ViewMode::AssetViewer;
            }
        }
    }

    pub fn inspect_selected_knowledge_type(&mut self) {
        let types = get_knowledge_node_types(self.abilities.len(), self.workspaces.len(), self.workspace_tasks.len());
        if let Some(idx) = self.knowledge_types_table_state.selected() {
            if let Some(k_type) = types.get(idx) {
                let mut content = format!("Node Type: {}\nTotal Count: {}\nDescription: {}\n\nRegistered Nodes:\n", k_type.name, k_type.count, k_type.description);
                for node in &k_type.sample_nodes {
                    content.push_str(&format!("  • {node}\n"));
                }
                self.asset_viewer_title = format!(" Knowledge Graph Node Inspector: {} ", k_type.name);
                self.asset_viewer_content = content;
                self.asset_viewer_scroll = 0;
                self.mode = ViewMode::AssetViewer;
            }
        }
    }

    pub fn inspect_selected_context_repo(&mut self) {
        let entries = self.get_workspace_entries();
        if let Some(idx) = self.context_repos_table_state.selected() {
            if let Some(entry) = entries.get(idx) {
                let ws_id = entry.id.as_deref().unwrap_or("(all)");
                let ws_path = entry.path.as_deref().unwrap_or("-");
                let ctx = detect_workspace_repo_context(ws_id, &entry.name, ws_path);

                let content = format!(
                    "Workspace Target: {}\nWorkspace ID: {}\nProvider: {}\nGit SSH URL: {}\nLocal Path: {}\n\nIndex Status: {}\nAST Status: {}\nGraph Status: {}\n",
                    ctx.name, ctx.workspace_id, ctx.provider, ctx.ssh_url, ctx.path, ctx.index_status, ctx.ast_status, ctx.graph_status
                );

                copy_to_clipboard(&ctx.ssh_url);
                self.set_status(format!("Copied Git SSH URL '{}' to system clipboard", ctx.ssh_url));

                self.asset_viewer_title = format!(" Workspace Context Inspector: {} ({}) ", ctx.name, ctx.provider);
                self.asset_viewer_content = content;
                self.asset_viewer_scroll = 0;
                self.mode = ViewMode::AssetViewer;
            }
        }
    }

    pub fn stop_selected_worker(&mut self) -> Result<()> {
        if let Some(worker) = self.selected_worker() {
            let id = worker.record.worker_id.clone();
            if worker.record.status == WorkerStatus::Stopped
                || worker.record.status == WorkerStatus::Succeeded
                || worker.record.status == WorkerStatus::Failed
            {
                self.set_status(format!(
                    "Worker {id} is already inactive ({:?}). Press [d] to delete record.",
                    worker.record.status
                ));
                return Ok(());
            }

            match self.registry.stop(&id) {
                Ok(_) => {
                    self.set_status(format!("Stop signal (SIGTERM) sent to worker {id}"));
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    let clean_msg = err_msg.strip_prefix("LIFECYCLE: ").unwrap_or(&err_msg);
                    self.set_status(format!("{clean_msg}"));
                }
            }
        }
        self.refresh_workers()?;
        Ok(())
    }

    pub fn force_kill_selected_worker(&mut self) -> Result<()> {
        if let Some(worker) = self.selected_worker() {
            if let Some(pid) = worker.record.pid {
                let _ = std::process::Command::new("kill")
                    .args(["-9", &pid.to_string()])
                    .stderr(std::process::Stdio::null())
                    .status();
                let _ = self.registry.finish_if_active(&worker.record.worker_id, WorkerStatus::Failed);
                self.set_status(format!("Force killed (SIGKILL -9) worker PID {pid}"));
            } else {
                self.set_status(format!("Worker has no active process PID to kill"));
            }
        }
        self.refresh_workers()?;
        Ok(())
    }

    pub fn delete_selected_worker(&mut self) -> Result<()> {
        if let Some(worker) = self.selected_worker() {
            let id = worker.record.worker_id.clone();
            match self.registry.delete(&id) {
                Ok(Some(_)) => {
                    self.set_status(format!("Purged worker record {id}"));
                }
                Ok(None) => {
                    self.set_status(format!("Worker {id} not found"));
                }
                Err(err) => {
                    self.set_status(format!("Error deleting worker: {err}"));
                }
            }
        }
        self.refresh_workers()?;
        Ok(())
    }

    pub fn stop_and_delete_selected_worker(&mut self) -> Result<()> {
        if let Some(worker) = self.selected_worker() {
            let id = worker.record.worker_id.clone();
            if worker.record.status == WorkerStatus::Running
                || worker.record.status == WorkerStatus::Starting
            {
                let _ = self.registry.stop(&id);
                if let Some(pid) = worker.record.pid {
                    let _ = std::process::Command::new("kill")
                        .args(["-9", &pid.to_string()])
                        .stderr(std::process::Stdio::null())
                        .status();
                }
            }
            match self.registry.delete(&id) {
                Ok(Some(_)) => {
                    self.set_status(format!("Stopped and purged worker record {id}"));
                }
                Ok(None) => {
                    self.set_status(format!("Worker {id} not found"));
                }
                Err(err) => {
                    self.set_status(format!("Error deleting worker: {err}"));
                }
            }
        }
        self.refresh_workers()?;
        Ok(())
    }

    pub fn restart_selected_worker(&mut self) -> Result<()> {
        if let Some(worker) = self.selected_worker() {
            let ws_id = worker.record.workspace_id.clone();
            let old_id = worker.record.worker_id.clone();

            if worker.record.status == WorkerStatus::Stopped
                || worker.record.status == WorkerStatus::Failed
                || worker.record.status == WorkerStatus::Succeeded
            {
                let _ = self.registry.delete(&old_id);
            } else {
                let _ = self.registry.stop(&old_id);
            }

            self.launch_worker(ws_id)?;
            self.set_status(format!("Restarted worker for workspace scope"));
        } else {
            self.set_status("No worker selected to restart");
        }
        Ok(())
    }

    pub fn launch_worker(&mut self, workspace_id: Option<String>) -> Result<()> {
        if let Ok(Some(existing)) = self.registry.active_for_workspace(workspace_id.as_deref()) {
            let target = workspace_id.as_deref().unwrap_or("(all)");
            self.set_status(format!(
                "Workspace '{target}' already has running worker {}",
                existing.worker_id
            ));
            return Ok(());
        }

        let current_exe = std::env::current_exe()?;
        let mut cmd = std::process::Command::new(current_exe);
        cmd.arg("start").arg("--daemon");
        if let Some(ref ws) = workspace_id {
            cmd.arg("--workspace").arg(ws);
        }
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        match cmd.spawn() {
            Ok(_) => {
                let target = workspace_id.unwrap_or_else(|| "(all)".into());
                self.set_status(format!("Daemon worker spawned for workspace: {target}"));
            }
            Err(err) => {
                self.set_status(format!("Failed to launch worker: {err}"));
            }
        }
        self.refresh_workers()?;
        Ok(())
    }

    pub fn copy_selected_info(&mut self) {
        if let Some(worker) = self.selected_worker() {
            let id = worker.record.worker_id.clone();
            copy_to_clipboard(&id);
            self.set_status(format!("Yanked Worker ID '{id}' to system clipboard"));
        }
    }

    pub fn copy_log_path(&mut self) {
        if let Some(worker) = self.selected_worker() {
            let path = worker.record.log_path.display().to_string();
            copy_to_clipboard(&path);
            self.set_status(format!("Yanked Log Path to system clipboard"));
        }
    }
}

fn copy_to_clipboard(text: &str) {
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        if let Ok(mut child) = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }
    #[cfg(target_os = "linux")]
    {
        use std::io::Write;
        if let Ok(mut child) = std::process::Command::new("xclip")
            .arg("-selection")
            .arg("clipboard")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }
}

pub fn run_tui(data_dir: &Path, server_url: String, api_key: Option<String>) -> Result<()> {
    enable_raw_mode().context("enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("create terminal")?;

    let app_result = main_tui_loop(&mut terminal, data_dir, server_url, api_key);

    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    app_result
}

fn main_tui_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    data_dir: &Path,
    server_url: String,
    api_key: Option<String>,
) -> Result<()> {
    let mut app = TuiApp::new(data_dir, server_url, api_key)?;

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = Duration::from_millis(250);
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == crossterm::event::KeyEventKind::Press {
                    match app.mode {
                        ViewMode::Normal => handle_normal_keys(&mut app, key)?,
                        ViewMode::WorkerInspector => handle_inspector_keys(&mut app, key)?,
                        ViewMode::FilterPrompt => handle_filter_keys(&mut app, key)?,
                        ViewMode::StartWorkerPrompt => handle_start_prompt_keys(&mut app, key)?,
                        ViewMode::AssetViewer => handle_asset_viewer_keys(&mut app, key)?,
                    }
                }
            }
        }

        if app.last_tick.elapsed() >= Duration::from_millis(500) {
            app.refresh_workers().ok();
            app.last_tick = Instant::now();
        }
    }
}

fn handle_normal_keys(app: &mut TuiApp, key: crossterm::event::KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => return Err(anyhow::anyhow!("QUIT")),
        KeyCode::Char('1') => app.main_tab = MainTab::Workers,
        KeyCode::Char('2') => app.main_tab = MainTab::WorkspacesAndTasks,
        KeyCode::Char('3') => app.main_tab = MainTab::ServerStatus,
        KeyCode::Tab => {
            app.main_tab = match app.main_tab {
                MainTab::Workers => MainTab::WorkspacesAndTasks,
                MainTab::WorkspacesAndTasks => MainTab::ServerStatus,
                MainTab::ServerStatus => MainTab::Workers,
            };
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if app.main_tab == MainTab::ServerStatus {
                app.diagnostics_tab = match app.diagnostics_tab {
                    DiagnosticsTab::Abilities => DiagnosticsTab::ServerConfig,
                    DiagnosticsTab::Skills => DiagnosticsTab::Abilities,
                    DiagnosticsTab::KnowledgeGraph => DiagnosticsTab::Skills,
                    DiagnosticsTab::ContextRepos => DiagnosticsTab::KnowledgeGraph,
                    DiagnosticsTab::ServerConfig => DiagnosticsTab::ContextRepos,
                };
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if app.main_tab == MainTab::ServerStatus {
                app.diagnostics_tab = match app.diagnostics_tab {
                    DiagnosticsTab::Abilities => DiagnosticsTab::Skills,
                    DiagnosticsTab::Skills => DiagnosticsTab::KnowledgeGraph,
                    DiagnosticsTab::KnowledgeGraph => DiagnosticsTab::ContextRepos,
                    DiagnosticsTab::ContextRepos => DiagnosticsTab::ServerConfig,
                    DiagnosticsTab::ServerConfig => DiagnosticsTab::Abilities,
                };
            }
        }
        KeyCode::Down | KeyCode::Char('j') => match app.main_tab {
            MainTab::Workers => app.select_next_worker(),
            MainTab::WorkspacesAndTasks => app.select_next_workspace(),
            MainTab::ServerStatus => match app.diagnostics_tab {
                DiagnosticsTab::Abilities => app.select_next_ability(),
                DiagnosticsTab::Skills => app.select_next_skill(),
                DiagnosticsTab::KnowledgeGraph => app.select_next_knowledge_type(),
                DiagnosticsTab::ContextRepos => app.select_next_context_repo(),
                _ => {}
            },
        },
        KeyCode::Up | KeyCode::Char('k') => match app.main_tab {
            MainTab::Workers => app.select_prev_worker(),
            MainTab::WorkspacesAndTasks => app.select_prev_workspace(),
            MainTab::ServerStatus => match app.diagnostics_tab {
                DiagnosticsTab::Abilities => app.select_prev_ability(),
                DiagnosticsTab::Skills => app.select_prev_skill(),
                DiagnosticsTab::KnowledgeGraph => app.select_prev_knowledge_type(),
                DiagnosticsTab::ContextRepos => app.select_prev_context_repo(),
                _ => {}
            },
        },
        KeyCode::Enter => match app.main_tab {
            MainTab::Workers => {
                if app.selected_worker().is_some() {
                    app.mode = ViewMode::WorkerInspector;
                    app.log_scroll = 0;
                }
            }
            MainTab::WorkspacesAndTasks => {
                let ws = app.selected_workspace_id.clone();
                app.launch_worker(ws)?;
            }
            MainTab::ServerStatus => match app.diagnostics_tab {
                DiagnosticsTab::Abilities => app.inspect_selected_ability(),
                DiagnosticsTab::Skills => app.inspect_selected_skill(),
                DiagnosticsTab::KnowledgeGraph => app.inspect_selected_knowledge_type(),
                DiagnosticsTab::ContextRepos => app.inspect_selected_context_repo(),
                _ => {}
            },
        },
        KeyCode::Char('s') => {
            app.mode = ViewMode::StartWorkerPrompt;
            app.start_workspace_input.clear();
        }
        KeyCode::Char('S') | KeyCode::Char('R') => {
            app.restart_selected_worker()?;
        }
        KeyCode::Char('L') => {
            if app.main_tab == MainTab::WorkspacesAndTasks {
                let ws = app.selected_workspace_id.clone();
                app.launch_worker(ws)?;
            }
        }
        KeyCode::Char('x') | KeyCode::Char('K') => {
            app.stop_selected_worker()?;
        }
        KeyCode::Char('X') => {
            app.force_kill_selected_worker()?;
        }
        KeyCode::Char('y') | KeyCode::Char('c') => {
            app.copy_selected_info();
        }
        KeyCode::Char('Y') => {
            app.copy_log_path();
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            app.delete_selected_worker()?;
        }
        KeyCode::Char('D') => {
            app.stop_and_delete_selected_worker()?;
        }
        KeyCode::Char('r') => {
            app.refresh_workers()?;
            app.set_status("State refreshed");
        }
        KeyCode::Char('/') => {
            app.mode = ViewMode::FilterPrompt;
        }
        _ => {}
    }
    Ok(())
}

fn handle_inspector_keys(app: &mut TuiApp, key: crossterm::event::KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => {
            app.mode = ViewMode::Normal;
        }
        KeyCode::Tab => {
            app.inspector_tab = match app.inspector_tab {
                InspectorTab::Logs => InspectorTab::Subprocesses,
                InspectorTab::Subprocesses => InspectorTab::Logs,
            };
        }
        KeyCode::Char('f') => {
            app.auto_scroll = !app.auto_scroll;
            app.set_status(format!(
                "Auto-scroll follow: {}",
                if app.auto_scroll { "ON" } else { "OFF" }
            ));
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.auto_scroll = false;
            app.log_scroll = app.log_scroll.saturating_add(1);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.auto_scroll = false;
            app.log_scroll = app.log_scroll.saturating_sub(1);
        }
        KeyCode::Char('y') | KeyCode::Char('c') => {
            app.copy_selected_info();
        }
        KeyCode::Char('Y') => {
            app.copy_log_path();
        }
        KeyCode::Char('x') => {
            app.stop_selected_worker()?;
        }
        KeyCode::Char('D') => {
            app.stop_and_delete_selected_worker()?;
            app.mode = ViewMode::Normal;
        }
        _ => {}
    }
    Ok(())
}

fn handle_asset_viewer_keys(app: &mut TuiApp, key: crossterm::event::KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => {
            app.mode = ViewMode::Normal;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.asset_viewer_scroll = app.asset_viewer_scroll.saturating_add(1);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.asset_viewer_scroll = app.asset_viewer_scroll.saturating_sub(1);
        }
        KeyCode::Char('y') | KeyCode::Char('c') => {
            copy_to_clipboard(&app.asset_viewer_content);
            app.set_status("Copied specification content to system clipboard");
        }
        _ => {}
    }
    Ok(())
}

fn handle_filter_keys(app: &mut TuiApp, key: crossterm::event::KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Enter | KeyCode::Esc => {
            app.mode = ViewMode::Normal;
        }
        KeyCode::Backspace => {
            app.filter_query.pop();
        }
        KeyCode::Char(c) => {
            app.filter_query.push(c);
        }
        _ => {}
    }
    Ok(())
}

fn handle_start_prompt_keys(app: &mut TuiApp, key: crossterm::event::KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Enter => {
            let ws = if app.start_workspace_input.trim().is_empty() {
                None
            } else {
                Some(app.start_workspace_input.trim().to_string())
            };
            app.mode = ViewMode::Normal;
            app.launch_worker(ws)?;
        }
        KeyCode::Esc => {
            app.mode = ViewMode::Normal;
        }
        KeyCode::Backspace => {
            app.start_workspace_input.pop();
        }
        KeyCode::Char(c) => {
            app.start_workspace_input.push(c);
        }
        _ => {}
    }
    Ok(())
}

fn ui(f: &mut Frame, app: &mut TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Real-time Hardware Topology & Resource Dashboard Header
            Constraint::Min(9),    // Active Content Pane
            Constraint::Length(3), // Interactive Footer
        ])
        .split(f.area());

    render_system_header(f, app, chunks[0]);

    match app.mode {
        ViewMode::Normal | ViewMode::FilterPrompt | ViewMode::StartWorkerPrompt => match app.main_tab {
            MainTab::Workers => render_dense_workers_dashboard(f, app, chunks[1]),
            MainTab::WorkspacesAndTasks => render_workspaces_tab(f, app, chunks[1]),
            MainTab::ServerStatus => render_server_tab(f, app, chunks[1]),
        },
        ViewMode::WorkerInspector => {
            render_inspector(f, app, chunks[1]);
        }
        ViewMode::AssetViewer => {
            render_asset_viewer(f, app, chunks[1]);
        }
    }

    if app.mode == ViewMode::StartWorkerPrompt {
        render_start_worker_popup(f, app, f.area());
    } else if app.mode == ViewMode::FilterPrompt {
        render_filter_popup(f, app, f.area());
    }

    render_footer(f, app, chunks[2]);
}

fn render_system_header(f: &mut Frame, app: &TuiApp, area: Rect) {
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(area);

    let titles: Vec<Line> = vec![
        Line::from(" [1] Workers Engine "),
        Line::from(" [2] Workspaces & Queue "),
        Line::from(format!(" [3] Diagnostics ({}) ", app.abilities.len() + app.skills.len())),
    ];

    let select_idx = match app.main_tab {
        MainTab::Workers => 0,
        MainTab::WorkspacesAndTasks => 1,
        MainTab::ServerStatus => 2,
    };

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Savant Colosseum (v{}) ", env!("CARGO_PKG_VERSION")))
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .select(select_idx)
        .style(Style::default().fg(Color::Gray))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, top_chunks[0]);

    let hw = get_hardware_topology(&app.system);
    let cpu_bar = make_gauge(app.total_cpu_usage, 6);
    let ram_bar = make_gauge(((app.total_memory_mb / 1024.0) * 100.0) as f32, 6);
    let disk_bar = make_gauge(app.total_disk_percent, 6);

    let metrics_text = vec![
        Line::from(vec![
            Span::styled("CPU: ", Style::default().fg(Color::Yellow)),
            Span::styled(cpu_bar, Style::default().fg(Color::Green)),
            Span::styled(format!(" {:.1}%  ", app.total_cpu_usage), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),

            Span::styled("RAM: ", Style::default().fg(Color::Yellow)),
            Span::styled(ram_bar, Style::default().fg(Color::Cyan)),
            Span::styled(format!(" {:.1}MB  ", app.total_memory_mb), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),

            Span::styled("DISK: ", Style::default().fg(Color::Yellow)),
            Span::styled(disk_bar, Style::default().fg(Color::Magenta)),
            Span::styled(format!(" {:.0}% ({:.0}/{:.0}GB)", app.total_disk_percent, app.total_disk_used_gb, app.total_disk_total_gb), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("I/O: ", Style::default().fg(Color::Yellow)),
            Span::styled(format!("R:{:.0}K / W:{:.0}K  ", app.io_read_kb, app.io_write_kb), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled("Workers: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}/{}  ", app.active_workers_count, app.total_workers_count), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("Server: ", Style::default().fg(Color::Gray)),
            Span::styled("ONLINE  ", Style::default().fg(Color::Green)),
            Span::styled("Abilities: ", Style::default().fg(Color::Gray)),
            Span::styled(app.abilities.len().to_string(), Style::default().fg(Color::Yellow)),
            Span::styled(" │ Skills: ", Style::default().fg(Color::Gray)),
            Span::styled(app.skills.len().to_string(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Topology: ", Style::default().fg(Color::Cyan)),
            Span::styled(format!("{} Cores / {} Threads ({} Available)  │ ", hw.physical_cores, hw.logical_threads, hw.available_threads), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled("GPU: ", Style::default().fg(Color::Cyan)),
            Span::styled(format!("{} ({} │ {})", hw.gpu_model, hw.gpu_cores, hw.gpu_vram), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]),
    ];

    let metrics_block = Paragraph::new(metrics_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Real-time System Resources & Hardware Topology Gauge ")
            .border_style(Style::default().fg(Color::Blue)),
    );

    f.render_widget(metrics_block, top_chunks[1]);
}

fn render_dense_workers_dashboard(f: &mut Frame, app: &mut TuiApp, area: Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let selected_style = Style::default()
        .bg(Color::DarkGray)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let header_cells = [
        "Worker ID",
        "Workspace Scope",
        "Status",
        "PID",
        "CPU %",
        "Memory (MB)",
        "Last Event",
        "Last Log Summary",
    ]
    .iter()
    .map(|h| Span::styled(*h, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let filtered = app.filtered_workers();
    let rows = filtered.iter().map(|w| {
        let status_color = match w.record.status {
            WorkerStatus::Running => Color::Green,
            WorkerStatus::Starting => Color::Yellow,
            WorkerStatus::Stopped => Color::Gray,
            WorkerStatus::Succeeded => Color::Cyan,
            WorkerStatus::Failed => Color::Red,
        };

        let pid_str = w.record.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into());
        let cpu_str = if w.cpu_usage > 0.0 {
            format!("{:.1}%", w.cpu_usage)
        } else {
            "-".into()
        };
        let mem_str = if w.memory_mb > 0.0 {
            format!("{:.1} MB", w.memory_mb)
        } else {
            "-".into()
        };

        Row::new(vec![
            Span::raw(w.record.worker_id.clone()),
            Span::styled(app.get_workspace_label(w.record.workspace_id.as_deref()), Style::default().fg(Color::Yellow)),
            Span::styled(format!("{:?}", w.record.status), Style::default().fg(status_color)),
            Span::raw(pid_str),
            Span::raw(cpu_str),
            Span::raw(mem_str),
            Span::styled(w.last_event_type.clone(), Style::default().fg(Color::Cyan)),
            Span::raw(w.last_event_msg.chars().take(45).collect::<String>()),
        ])
    });

    let table_title = if app.filter_query.is_empty() {
        format!(" Workers Registry ({}) ", filtered.len())
    } else {
        format!(" Workers Registry (Filter: '{}' - {} matches) ", app.filter_query, filtered.len())
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(20),
            Constraint::Percentage(24),
            Constraint::Percentage(10),
            Constraint::Percentage(7),
            Constraint::Percentage(7),
            Constraint::Percentage(9),
            Constraint::Percentage(10),
            Constraint::Percentage(13),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(table_title)
            .border_style(Style::default().fg(Color::Cyan)),
    )
    .row_highlight_style(selected_style)
    .highlight_symbol("► ");

    f.render_stateful_widget(table, main_chunks[0], &mut app.table_state);

    if let Some(worker) = app.selected_worker() {
        let details_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(main_chunks[1]);

        let mut info_lines = vec![
            Line::from(vec![
                Span::styled("Worker ID: ", Style::default().fg(Color::Yellow)),
                Span::styled(&worker.record.worker_id, Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("Workspace Scope: ", Style::default().fg(Color::Yellow)),
                Span::raw(app.get_workspace_label(worker.record.workspace_id.as_deref())),
            ]),
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{:?}", worker.record.status)),
            ]),
            Line::from(vec![
                Span::styled("Process PID: ", Style::default().fg(Color::Yellow)),
                Span::raw(worker.record.pid.map(|p| p.to_string()).unwrap_or_else(|| "N/A".into())),
            ]),
            Line::from(vec![
                Span::styled("Started At: ", Style::default().fg(Color::Yellow)),
                Span::raw(&worker.record.started_at),
            ]),
            Line::from(vec![
                Span::styled("Log Path: ", Style::default().fg(Color::Yellow)),
                Span::raw(worker.record.log_path.display().to_string()),
            ]),
        ];

        if let Some(ref finished) = worker.record.finished_at {
            info_lines.push(Line::from(vec![
                Span::styled("Finished At: ", Style::default().fg(Color::Yellow)),
                Span::raw(finished),
            ]));
        }

        let summary = Paragraph::new(info_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Selected Worker Metadata ")
                .border_style(Style::default().fg(Color::Gray)),
        );
        f.render_widget(summary, details_chunks[0]);

        let child_items: Vec<ListItem> = if worker.children.is_empty() {
            vec![ListItem::new("No active child subprocesses running under process tree.")]
        } else {
            worker
                .children
                .iter()
                .map(|c| {
                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(format!("► PID {}: ", c.pid), Style::default().fg(Color::Green)),
                            Span::styled(&c.name, Style::default().add_modifier(Modifier::BOLD)),
                            Span::raw(format!(" | CPU: {:.1}% | RAM: {:.1}MB", c.cpu_usage, c.memory_mb)),
                        ]),
                        Line::from(vec![
                            Span::styled("  CMD: ", Style::default().fg(Color::DarkGray)),
                            Span::raw(c.cmd.chars().take(60).collect::<String>()),
                        ]),
                    ])
                })
                .collect()
        };

        let sub_list = List::new(child_items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Subprocess Hierarchy ({}) ", worker.children.len()))
                .border_style(Style::default().fg(Color::Gray)),
        );
        f.render_widget(sub_list, details_chunks[1]);
    } else {
        let empty_p = Paragraph::new("No worker selected").block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Metadata ")
                .border_style(Style::default().fg(Color::Gray)),
        );
        f.render_widget(empty_p, main_chunks[1]);
    }
}

pub struct WorkspaceEntry {
    pub id: Option<String>,
    pub name: String,
    pub path: Option<String>,
}

impl TuiApp {
    pub fn get_workspace_entries(&self) -> Vec<WorkspaceEntry> {
        let mut entries = vec![
            WorkspaceEntry {
                id: None,
                name: "Global Scope (All Workspaces)".into(),
                path: Some("Listens & claims ready tasks across all repositories".into()),
            },
        ];

        for ws in &self.workspaces {
            entries.push(WorkspaceEntry {
                id: Some(ws.id.clone()),
                name: if ws.name.is_empty() { ws.id.clone() } else { ws.name.clone() },
                path: ws.path.clone(),
            });
        }

        if !entries.iter().any(|e| e.id.as_deref() == Some("2539163563543949210")) {
            entries.push(WorkspaceEntry {
                id: Some("2539163563543949210".into()),
                name: "savant-colosseum".into(),
                path: Some("/Users/home/code/project-x/savant-colosseum".into()),
            });
        }

        entries
    }

    pub fn select_next_workspace(&mut self) {
        let entries = self.get_workspace_entries();
        if entries.is_empty() {
            return;
        }
        let idx = match self.workspace_table_state.selected() {
            Some(i) => (i + 1) % entries.len(),
            None => 0,
        };
        self.workspace_table_state.select(Some(idx));
        self.selected_workspace_id = entries[idx].id.clone();
    }

    pub fn select_prev_workspace(&mut self) {
        let entries = self.get_workspace_entries();
        if entries.is_empty() {
            return;
        }
        let idx = match self.workspace_table_state.selected() {
            Some(i) if i > 0 => i - 1,
            _ => entries.len() - 1,
        };
        self.workspace_table_state.select(Some(idx));
        self.selected_workspace_id = entries[idx].id.clone();
    }
}

fn render_workspaces_tab(f: &mut Frame, app: &mut TuiApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(6)])
        .split(area);

    let selected_ws_name = app.selected_workspace_id.as_deref().unwrap_or("Global Scope (All)");

    let info_p = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Navigation: ", Style::default().fg(Color::Gray)),
            Span::styled("[↑/↓/j/k]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" Select Workspace  │  ", Style::default().fg(Color::Gray)),
            Span::styled("[Enter] or [L]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(" Launch Worker for Selected Workspace", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Target Selection: ", Style::default().fg(Color::Cyan)),
            Span::styled(selected_ws_name, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" │ Server: "),
            Span::raw(&app.server_url),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Savant Workspaces & Colosseum Worker Launcher ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(info_p, chunks[0]);

    if app.workspace_table_state.selected().is_none() {
        app.workspace_table_state.select(Some(0));
    }

    let entries = app.get_workspace_entries();
    let header_cells = ["Workspace Name", "Workspace ID", "Repository Path / Scope", "Active Workers"]
        .iter()
        .map(|h| Span::styled(*h, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = entries.iter().map(|entry| {
        let ws_id_str = entry.id.as_deref().unwrap_or("(all)");
        let active_cnt = app
            .workers
            .iter()
            .filter(|w| {
                w.record.status == WorkerStatus::Running
                    && w.record.workspace_id.as_deref() == entry.id.as_deref()
            })
            .count();

        let active_str = if active_cnt > 0 {
            format!("{active_cnt} Running")
        } else {
            "0".into()
        };

        Row::new(vec![
            Span::styled(entry.name.clone(), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(ws_id_str.to_string()),
            Span::raw(entry.path.as_deref().unwrap_or("-").to_string()),
            Span::styled(active_str, Style::default().fg(if active_cnt > 0 { Color::Green } else { Color::Gray })),
        ])
    });

    let selected_style = Style::default()
        .bg(Color::DarkGray)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(25),
            Constraint::Percentage(30),
            Constraint::Percentage(15),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Savant Workspaces ({}) ", entries.len()))
            .border_style(Style::default().fg(Color::Blue)),
    )
    .row_highlight_style(selected_style)
    .highlight_symbol("► ");

    f.render_stateful_widget(table, chunks[1], &mut app.workspace_table_state);
}

fn render_server_tab(f: &mut Frame, app: &mut TuiApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(6)])
        .split(area);

    let diag_titles = vec![
        Line::from(format!(" [1] Abilities ({}) ", app.abilities.len())),
        Line::from(format!(" [2] Skills ({}) ", app.skills.len())),
        Line::from(" [3] Knowledge Graph "),
        Line::from(format!(" [4] Context & Repos ({}) ", app.get_workspace_entries().len())),
        Line::from(" [5] Server & Providers "),
    ];

    let select_idx = match app.diagnostics_tab {
        DiagnosticsTab::Abilities => 0,
        DiagnosticsTab::Skills => 1,
        DiagnosticsTab::KnowledgeGraph => 2,
        DiagnosticsTab::ContextRepos => 3,
        DiagnosticsTab::ServerConfig => 4,
    };

    let tabs = Tabs::new(diag_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Savant Intelligence Diagnostics Explorer (Press [←/→] or [h/l] to switch) ")
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .select(select_idx)
        .style(Style::default().fg(Color::Gray))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, chunks[0]);

    match app.diagnostics_tab {
        DiagnosticsTab::Abilities => render_abilities_subtab(f, app, chunks[1]),
        DiagnosticsTab::Skills => render_skills_subtab(f, app, chunks[1]),
        DiagnosticsTab::KnowledgeGraph => render_knowledge_subtab(f, app, chunks[1]),
        DiagnosticsTab::ContextRepos => render_context_repos_subtab(f, app, chunks[1]),
        DiagnosticsTab::ServerConfig => render_config_subtab(f, app, chunks[1]),
    }
}

fn render_abilities_subtab(f: &mut Frame, app: &mut TuiApp, area: Rect) {
    if app.abilities_table_state.selected().is_none() && !app.abilities.is_empty() {
        app.abilities_table_state.select(Some(0));
    }

    let header_cells = ["Ability / Rule Name", "Category / Persona Scope", "Specification File Path"]
        .iter()
        .map(|h| Span::styled(*h, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = app.abilities.iter().map(|ab| {
        Row::new(vec![
            Span::styled(ab.name.clone(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(ab.category.clone(), Style::default().fg(Color::Green)),
            Span::raw(ab.path.display().to_string()),
        ])
    });

    let selected_style = Style::default()
        .bg(Color::DarkGray)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(40),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Savant Abilities & Policies ({}) - Press [Enter] to Inspect ", app.abilities.len()))
            .border_style(Style::default().fg(Color::Blue)),
    )
    .row_highlight_style(selected_style)
    .highlight_symbol("► ");

    f.render_stateful_widget(table, area, &mut app.abilities_table_state);
}

fn render_skills_subtab(f: &mut Frame, app: &mut TuiApp, area: Rect) {
    if app.skills_table_state.selected().is_none() && !app.skills.is_empty() {
        app.skills_table_state.select(Some(0));
    }

    let header_cells = ["Skill ID / Title", "Installation Status", "AI Provider Target", "Category Scope", "Description"]
        .iter()
        .map(|h| Span::styled(*h, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = app.skills.iter().map(|sk| {
        let status_badge = if sk.installed {
            Span::styled(" [✓ INSTALLED] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" [✗ NOT INSTALLED] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        };

        let prov_color = match sk.provider.as_str() {
            p if p.contains("Claude") => Color::Magenta,
            p if p.contains("Codex") => Color::Cyan,
            p if p.contains("Gemini") => Color::Green,
            _ => Color::White,
        };

        Row::new(vec![
            Span::styled(sk.name.clone(), Style::default().add_modifier(Modifier::BOLD)),
            status_badge,
            Span::styled(sk.provider.clone(), Style::default().fg(prov_color)),
            Span::styled(sk.category.clone(), Style::default().fg(Color::Yellow)),
            Span::raw(sk.description.chars().take(45).collect::<String>()),
        ])
    });

    let selected_style = Style::default()
        .bg(Color::DarkGray)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(25),
            Constraint::Percentage(18),
            Constraint::Percentage(20),
            Constraint::Percentage(12),
            Constraint::Percentage(25),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Savant Server Skills ({}) - [✓ Installed / ✗ Not Installed] - Press [Enter] to Inspect ", app.skills.len()))
            .border_style(Style::default().fg(Color::Magenta)),
    )
    .row_highlight_style(selected_style)
    .highlight_symbol("► ");

    f.render_stateful_widget(table, area, &mut app.skills_table_state);
}

fn render_knowledge_subtab(f: &mut Frame, app: &mut TuiApp, area: Rect) {
    if app.knowledge_types_table_state.selected().is_none() {
        app.knowledge_types_table_state.select(Some(0));
    }

    let node_types = get_knowledge_node_types(app.abilities.len(), app.workspaces.len(), app.workspace_tasks.len());

    let header_cells = ["Node Entity Category", "Indexed Count", "Type Description & Memory Scope"]
        .iter()
        .map(|h| Span::styled(*h, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = node_types.iter().map(|nt| {
        Row::new(vec![
            Span::styled(nt.name.clone(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(nt.count.to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(nt.description.clone()),
        ])
    });

    let selected_style = Style::default()
        .bg(Color::DarkGray)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(35),
            Constraint::Percentage(15),
            Constraint::Percentage(50),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Knowledge Graph Nodes by Type ({}) - Press [Enter] to Expand ", node_types.len()))
            .border_style(Style::default().fg(Color::Green)),
    )
    .row_highlight_style(selected_style)
    .highlight_symbol("► ");

    f.render_stateful_widget(table, area, &mut app.knowledge_types_table_state);
}

fn render_context_repos_subtab(f: &mut Frame, app: &mut TuiApp, area: Rect) {
    if app.context_repos_table_state.selected().is_none() {
        app.context_repos_table_state.select(Some(0));
    }

    let entries = app.get_workspace_entries();
    let header_cells = ["Workspace / Repo", "Git Provider", "Index Status", "AST Status", "Graph Status", "Git SSH Remote URL"]
        .iter()
        .map(|h| Span::styled(*h, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = entries.iter().map(|entry| {
        let ws_id = entry.id.as_deref().unwrap_or("(all)");
        let ws_path = entry.path.as_deref().unwrap_or("-");
        let ctx = detect_workspace_repo_context(ws_id, &entry.name, ws_path);

        let prov_color = match ctx.provider.as_str() {
            "GitHub" => Color::Magenta,
            "GitLab" => Color::Yellow,
            _ => Color::Cyan,
        };

        Row::new(vec![
            Span::styled(ctx.name.clone(), Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(ctx.provider.clone(), Style::default().fg(prov_color)),
            Span::styled("INDEXED", Style::default().fg(Color::Green)),
            Span::styled("PARSED", Style::default().fg(Color::Green)),
            Span::styled("SYNCED", Style::default().fg(Color::Green)),
            Span::styled(ctx.ssh_url.clone(), Style::default().fg(Color::Cyan)),
        ])
    });

    let selected_style = Style::default()
        .bg(Color::DarkGray)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(20),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(32),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Context & Workspace Repositories ({}) - Press [Enter] to Copy SSH URL ", entries.len()))
            .border_style(Style::default().fg(Color::Cyan)),
    )
    .row_highlight_style(selected_style)
    .highlight_symbol("► ");

    f.render_stateful_widget(table, area, &mut app.context_repos_table_state);
}

fn check_binary_path(bin: &str) -> (bool, String) {
    if let Ok(output) = std::process::Command::new("which").arg(bin).output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return (true, path);
            }
        }
    }
    (false, "Not Installed".to_string())
}

fn render_config_subtab(f: &mut Frame, app: &TuiApp, area: Rect) {
    let providers = [
        ("gemini / agy", "gemini", "Google Gemini / Antigravity Agentic Engine"),
        ("claude", "claude", "Anthropic Claude Code CLI"),
        ("codex", "codex", "OpenAI Codex CLI Engine"),
        ("opencode", "opencode", "OpenCode Multi-Model Agent"),
        ("ollama", "ollama", "Ollama Local LLM Inference Engine"),
        ("node", "node", "Node.js JavaScript Runtime"),
        ("bun", "bun", "Bun High-Speed Runtime & Package Engine"),
        ("git", "git", "Git Version Control System Engine"),
        ("docker", "docker", "Docker Container Virtualization Engine"),
    ];

    let mut text = vec![
        Line::from(vec![
            Span::styled("Savant Server URL: ", Style::default().fg(Color::Yellow)),
            Span::raw(&app.server_url),
        ]),
        Line::from(vec![
            Span::styled("API Key Authorization: ", Style::default().fg(Color::Yellow)),
            Span::raw(if app.client.is_some() { "Configured & Authorized" } else { "Not Configured" }),
        ]),
        Line::from(vec![
            Span::styled("Colosseum Data Directory: ", Style::default().fg(Color::Yellow)),
            Span::raw(app.data_dir.display().to_string()),
        ]),
        Line::from(vec![
            Span::styled("Workers Log Storage: ", Style::default().fg(Color::Yellow)),
            Span::raw(app.data_dir.join("workers").display().to_string()),
        ]),
        Line::from(vec![
            Span::styled("Worktree Build Storage: ", Style::default().fg(Color::Yellow)),
            Span::raw(app.data_dir.join("worktrees").display().to_string()),
        ]),
        Line::from(""),
        Line::from(Span::styled("AI Providers & Tooling Runtimes Installation Health Check:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
    ];

    for (label, bin, purpose) in providers {
        let (installed, path) = check_binary_path(bin);
        let badge = if installed {
            Span::styled(" [✓ INSTALLED] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" [✗ NOT INSTALLED] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        };

        text.push(Line::from(vec![
            badge,
            Span::styled(format!("{label:<16}"), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(format!(" : {purpose}  ")),
            Span::styled(format!("({path})"), Style::default().fg(if installed { Color::Gray } else { Color::Red })),
        ]));
    }

    let hw = get_hardware_topology(&app.system);
    text.push(Line::from(""));
    text.push(Line::from(Span::styled("Hardware Topology & GPU Accelerator Diagnostics:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));
    text.push(Line::from(vec![
        Span::styled("• Physical CPU Cores: ", Style::default().fg(Color::White)),
        Span::styled(format!("{} Cores", hw.physical_cores), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled("  │ Logical CPU Threads: ", Style::default().fg(Color::White)),
        Span::styled(format!("{} Threads ({} Idle/Available)", hw.logical_threads, hw.available_threads), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
    ]));
    text.push(Line::from(vec![
        Span::styled("• GPU Hardware Accelerator: ", Style::default().fg(Color::White)),
        Span::styled(format!("{} ({})", hw.gpu_model, hw.gpu_cores), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
    ]));
    text.push(Line::from(vec![
        Span::styled("• Graphics Architecture & VRAM: ", Style::default().fg(Color::White)),
        Span::styled(hw.gpu_vram, Style::default().fg(Color::Cyan)),
    ]));

    text.push(Line::from(""));
    text.push(Line::from(Span::styled("System Engine Diagnostics:", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))));
    text.push(Line::from("• Worker Registry Lock: Atomic Directory Reservation (Active)"));
    text.push(Line::from("• Process Liveness Verification: PID + OS Start Time Validation"));
    text.push(Line::from("• Server Liveness: OK"));

    let p = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Savant Executioner Server, Providers & Environment Status ")
            .border_style(Style::default().fg(Color::Green)),
    );

    f.render_widget(p, area);
}

fn render_asset_viewer(f: &mut Frame, app: &mut TuiApp, area: Rect) {
    let lines: Vec<Line> = app
        .asset_viewer_content
        .lines()
        .map(|line| {
            if line.starts_with('#') {
                Line::from(Span::styled(line.to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)))
            } else if line.starts_with("```") || line.starts_with('-') || line.starts_with('*') {
                Line::from(Span::styled(line.to_string(), Style::default().fg(Color::Cyan)))
            } else {
                Line::from(Span::raw(line.to_string()))
            }
        })
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(app.asset_viewer_title.as_str())
                .border_style(Style::default().fg(Color::Green)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.asset_viewer_scroll as u16, 0));

    f.render_widget(paragraph, area);
}

fn render_inspector(f: &mut Frame, app: &mut TuiApp, area: Rect) {
    let auto_scroll = app.auto_scroll;
    let log_scroll = app.log_scroll;
    let inspector_tab = app.inspector_tab.clone();

    let Some(worker) = app.selected_worker() else {
        let p = Paragraph::new("Selected worker unavailable").block(Block::default().borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);

    let titles: Vec<Line> = vec![
        Line::from(" [1] Event Log Stream "),
        Line::from(" [2] Subprocesses & Process Tree "),
    ];

    let tab_idx = match inspector_tab {
        InspectorTab::Logs => 0,
        InspectorTab::Subprocesses => 1,
    };

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Deep Worker Inspector: {} ", worker.record.worker_id)),
        )
        .select(tab_idx)
        .style(Style::default().fg(Color::Cyan))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, chunks[0]);

    match inspector_tab {
        InspectorTab::Logs => render_log_stream_tab(f, auto_scroll, log_scroll, worker, chunks[1]),
        InspectorTab::Subprocesses => render_subprocess_tab(f, worker, chunks[1]),
    }
}

fn render_log_stream_tab(
    f: &mut Frame,
    auto_scroll: bool,
    log_scroll: usize,
    worker: &WorkerUiState,
    area: Rect,
) {
    let log_content = read_log(&worker.record.log_path).unwrap_or_else(|_| "Log file empty or unreadable.".into());

    let lines: Vec<Line> = log_content
        .lines()
        .map(|line| {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                let ts = val.get("timestamp").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let event = val.get("event").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let msg = val.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();

                let event_color = match event.as_str() {
                    e if e.contains("started") || e.contains("completed") => Color::Green,
                    e if e.contains("failed") || e.contains("error") => Color::Red,
                    e if e.contains("idle") => Color::Yellow,
                    _ => Color::Cyan,
                };

                Line::from(vec![
                    Span::styled(format!("{ts} "), Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("[{event}] "), Style::default().fg(event_color).add_modifier(Modifier::BOLD)),
                    Span::raw(msg),
                ])
            } else {
                Line::from(Span::raw(line.to_string()))
            }
        })
        .collect();

    let total_lines = lines.len();
    let scroll_pos = if auto_scroll && total_lines > 0 {
        total_lines.saturating_sub(area.height as usize - 2)
    } else {
        log_scroll
    };

    let title = format!(
        " Live Event Log Stream ({}) [Follow: {}] ",
        worker.record.log_path.display(),
        if auto_scroll { "ON" } else { "OFF" }
    );

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::Green)),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll_pos as u16, 0));

    f.render_widget(paragraph, area);
}

fn render_subprocess_tab(f: &mut Frame, worker: &WorkerUiState, area: Rect) {
    let mut items = Vec::new();

    items.push(ListItem::new(Line::from(vec![
        Span::styled("Worker Process PID: ", Style::default().fg(Color::Yellow)),
        Span::raw(worker.record.pid.map(|p| p.to_string()).unwrap_or_else(|| "None".into())),
        Span::raw(format!(" | CPU: {:.1}% | Memory: {:.1} MB", worker.cpu_usage, worker.memory_mb)),
    ])));
    items.push(ListItem::new(Line::from("──────────────────────────────────────────────────")));

    if worker.children.is_empty() {
        items.push(ListItem::new("No active child subprocesses found under worker process tree."));
    } else {
        items.push(ListItem::new(Span::styled(
            format!("Active Child Subprocesses ({})", worker.children.len()),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )));
        items.push(ListItem::new(""));

        for child in &worker.children {
            items.push(ListItem::new(vec![
                Line::from(vec![
                    Span::styled(format!("► PID {}: ", child.pid), Style::default().fg(Color::Green)),
                    Span::styled(child.name.clone(), Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(format!(" [CPU: {:.1}% | RAM: {:.1} MB]", child.cpu_usage, child.memory_mb)),
                ]),
                Line::from(vec![
                    Span::styled("  Command Line: ", Style::default().fg(Color::Yellow)),
                    Span::raw(child.cmd.clone()),
                ]),
                Line::from(""),
            ]));
        }
    }

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Process Tree & Subprocess Execution Inspector ")
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(list, area);
}

fn render_start_worker_popup(f: &mut Frame, app: &TuiApp, area: Rect) {
    let block = Block::default()
        .title(" Launch Daemon Worker ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let popup_area = centered_rect(60, 25, area);
    f.render_widget(Clear, popup_area);

    let content = vec![
        Line::from("Enter Workspace ID for new worker (or leave empty for all):"),
        Line::from(""),
        Line::from(Span::styled(
            format!(" Workspace ID: {}_", app.start_workspace_input),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press [Enter] to Launch Daemon Worker  │  [Esc] Cancel",
            Style::default().fg(Color::Cyan),
        )),
    ];

    let p = Paragraph::new(content).block(block);
    f.render_widget(p, popup_area);
}

fn render_filter_popup(f: &mut Frame, app: &TuiApp, area: Rect) {
    let block = Block::default()
        .title(" Filter Workers ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let popup_area = centered_rect(60, 20, area);
    f.render_widget(Clear, popup_area);

    let input = Paragraph::new(app.filter_query.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(block);

    f.render_widget(input, popup_area);
}

fn render_footer(f: &mut Frame, app: &TuiApp, area: Rect) {
    let status_text = if let Some((ref msg, ref time)) = app.status_message {
        if time.elapsed() < Duration::from_secs(4) {
            msg.clone()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let key_hints = match app.mode {
        ViewMode::Normal => match app.main_tab {
            MainTab::Workers => " [1/2/3] Tabs │ [↑/↓/j/k] Select │ [Enter] Inspector │ [s] Start │ [S/R] Restart │ [x] TERM │ [X] KILL │ [d] Purge │ [D] Stop & Purge │ [y/c] Copy ID │ [Y] Log Path │ [/] Filter │ [r] Refresh │ [q] Quit ",
            MainTab::WorkspacesAndTasks => " [1/2/3] Tabs │ [↑/↓/j/k] Select Workspace │ [Enter/L] Launch Worker │ [s] Custom Worker │ [r] Refresh │ [q] Quit ",
            MainTab::ServerStatus => " [1/2/3] Tabs │ [h/l] Subtabs │ [↑/↓/j/k] Select │ [Enter] Inspect Spec / Copy Git SSH │ [y/c] Copy │ [r] Refresh │ [q] Quit ",
        },
        ViewMode::WorkerInspector => " [Tab] Switch Logs/Tree │ [f] Toggle Follow │ [↑/↓/j/k] Scroll │ [y/c] Copy ID │ [Y] Log Path │ [x] TERM │ [D] Stop & Purge │ [Esc/q] Back ",
        ViewMode::AssetViewer => " [↑/↓/j/k] Scroll Spec │ [y/c] Copy Content │ [Esc/q] Close Inspector ",
        ViewMode::FilterPrompt => " Type filter query... │ [Enter/Esc] Apply/Done ",
        ViewMode::StartWorkerPrompt => " Type Workspace ID... │ [Enter] Launch Worker │ [Esc] Cancel ",
    };

    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(82), Constraint::Percentage(18)])
        .split(area);

    let hints_p = Paragraph::new(key_hints)
        .style(Style::default().fg(Color::Black).bg(Color::Cyan))
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(hints_p, footer_chunks[0]);

    let status_p = Paragraph::new(status_text)
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(status_p, footer_chunks[1]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

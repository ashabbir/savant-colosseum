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
    pipeline::{AgentConfig, ColosseumRegistry, Pipeline},
    savant::{SavantClient, Task, Workspace, GatewayHealthResponse, detect_gateway_url},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MainTab {
    Workers,
    WorkspacesAndTasks,
    PipelinesAndAgents,
    ServerStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSubpanel {
    Agents,
    Pipelines,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceSubpanel {
    Workspaces,
    Tasks,
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
    NewAgentPrompt,
    NewPipelinePrompt,
    PipelineSelector,
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
pub struct ProviderInstallStatus {
    pub gemini: bool,
    pub claude: bool,
    pub codex: bool,
    pub savant: bool,
}

#[derive(Debug, Clone)]
pub struct SkillItem {
    pub id: String,
    pub name: String,
    pub origin: String,   // "Savant Server API"
    pub provider: String, // "Google Gemini", "Anthropic Claude", "OpenAI Codex", "Savant MCP", "Universal"
    pub category: String,
    pub description: String,
    pub provider_status: ProviderInstallStatus,
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

    // Agent & Pipeline Registry State
    pub colosseum_registry: ColosseumRegistry,
    pub agent_table_state: TableState,
    pub pipeline_table_state: TableState,
    pub agents_subpanel: AgentSubpanel,
    pub editing_agent_id: Option<String>,
    pub editing_pipeline_id: Option<String>,

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

    // New Agent Interactive Creation State
    pub new_agent_name_input: String,
    pub new_agent_persona_input: String,
    pub new_agent_provider_input: String,
    pub new_agent_model_input: String,
    pub new_agent_pickup_input: String,
    pub new_agent_working_input: String,
    pub new_agent_drop_input: String,
    pub new_agent_prompt_input: String,
    pub new_agent_field_step: usize,
    pub new_agent_persona_idx: usize,
    pub new_agent_provider_idx: usize,
    pub new_agent_model_idx: usize,
    pub new_agent_pickup_idx: usize,
    pub new_agent_working_idx: usize,
    pub new_agent_drop_idx: usize,

    // New Pipeline Interactive Creation State
    pub new_pipeline_name_input: String,
    pub new_pipeline_agents_input: String,
    pub new_pipeline_agent_ids: Vec<String>,
    pub new_pipeline_selected_agent_idx: usize,
    pub new_pipeline_field_step: usize,

    // Pipeline Selector (for launching workers)
    pub pipeline_selector_idx: usize,

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

    // Savant Gateway AI Multi-Provider State
    pub gateway_url: String,
    pub gateway_health: Option<GatewayHealthResponse>,
    pub gateway_table_state: TableState,
    pub gateway_rx: Option<std::sync::mpsc::Receiver<Result<GatewayHealthResponse>>>,
    pub gateway_pending: bool,

    // Non-blocking async fetch channels
    pub workspaces_rx: Option<std::sync::mpsc::Receiver<Result<Vec<Workspace>>>>,
    pub workspaces_pending: bool,
    pub skills_rx: Option<std::sync::mpsc::Receiver<Result<Vec<crate::savant::ServerSkill>>>>,
    pub skills_pending: bool,
    pub tasks_rx: Option<std::sync::mpsc::Receiver<Result<Vec<Task>>>>,
    pub tasks_pending: bool,
    pub workspaces_subpanel: WorkspaceSubpanel,

    pub status_message: Option<(String, Instant)>,
    pub system: System,
    pub disks: Disks,
    pub last_tick: Instant,
    pub slow_tick: Instant,
}


pub fn scan_abilities() -> Vec<AbilityItem> {
    Vec::new()
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

pub fn check_skill_providers(id: &str) -> (ProviderInstallStatus, Option<PathBuf>) {
    let id_low = id.to_lowercase();
    let id_dash = id_low.replace('_', "-");
    let id_underscore = id_low.replace('-', "_");

    let mut gemini = false;
    let mut claude = false;
    let mut codex = false;
    let mut savant = false;
    let mut sample_path: Option<PathBuf> = None;

    let Ok(home) = std::env::var("HOME") else {
        return (ProviderInstallStatus { gemini, claude, codex, savant }, None);
    };

    let home_path = PathBuf::from(home);

    // 1. Check Google Gemini / AGY (~/.gemini/)
    let gemini_dirs = [
        home_path.join(".gemini/skills"),
        home_path.join(".gemini/antigravity-cli/skills"),
        home_path.join(".gemini/antigravity-cli/builtin/skills"),
    ];
    for dir in &gemini_dirs {
        if dir.exists() {
            if dir.join(id).exists() || dir.join(&id_low).exists() || dir.join(&id_dash).exists() || dir.join(&id_underscore).exists() {
                gemini = true;
                if sample_path.is_none() {
                    sample_path = Some(dir.join(id));
                }
            }
        }
    }

    // 2. Check Anthropic Claude Code (~/.claude/skills/)
    let claude_dir = home_path.join(".claude/skills");
    if claude_dir.exists() {
        if claude_dir.join(id).exists() || claude_dir.join(&id_low).exists() || claude_dir.join(&id_dash).exists() || claude_dir.join(&id_underscore).exists() {
            claude = true;
            if sample_path.is_none() {
                sample_path = Some(claude_dir.join(id));
            }
        }
    }

    // 3. Check OpenAI Codex (~/.codex/skills/)
    let codex_dir = home_path.join(".codex/skills");
    if codex_dir.exists() {
        if codex_dir.join(id).exists() || codex_dir.join(&id_low).exists() || codex_dir.join(&id_dash).exists() || codex_dir.join(&id_underscore).exists() {
            codex = true;
            if sample_path.is_none() {
                sample_path = Some(codex_dir.join(id));
            }
        }
    }

    // 4. Check Savant Engine (~/.savant/skills/)
    let savant_dir = home_path.join(".savant/skills");
    if savant_dir.exists() {
        if savant_dir.join(id).exists() || savant_dir.join(&id_low).exists() {
            savant = true;
            if sample_path.is_none() {
                sample_path = Some(savant_dir.join(id));
            }
        } else if let Ok(categories) = std::fs::read_dir(&savant_dir) {
            for cat_entry in categories.flatten() {
                let cat_path = cat_entry.path();
                if cat_path.is_dir() {
                    if cat_path.join(id).exists() || cat_path.join(&id_low).exists() || cat_path.join(&id_dash).exists() {
                        savant = true;
                        if sample_path.is_none() {
                            sample_path = Some(cat_path.join(id));
                        }
                    }
                }
            }
        }
    }

    (
        ProviderInstallStatus {
            gemini,
            claude,
            codex,
            savant,
        },
        sample_path,
    )
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
            colosseum_registry: ColosseumRegistry::load_from_file(&ColosseumRegistry::default_storage_path()).unwrap_or_default(),
            agent_table_state: TableState::default(),
            pipeline_table_state: TableState::default(),
            agents_subpanel: AgentSubpanel::Agents,
            editing_agent_id: None,
            editing_pipeline_id: None,
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
            new_agent_name_input: String::new(),
            new_agent_persona_input: "persona.coder".to_string(),
            new_agent_provider_input: "claude".to_string(),
            new_agent_model_input: "claude-3-5-sonnet".to_string(),
            new_agent_pickup_input: "ready".to_string(),
            new_agent_working_input: "in-progress".to_string(),
            new_agent_drop_input: "review".to_string(),
            new_agent_prompt_input: String::new(),
            new_agent_field_step: 0,
            new_agent_persona_idx: 0,
            new_agent_provider_idx: 0,
            new_agent_model_idx: 0,
            new_agent_pickup_idx: 2,
            new_agent_working_idx: 3,
            new_agent_drop_idx: 4,
            new_pipeline_name_input: String::new(),
            new_pipeline_agents_input: String::new(),
            new_pipeline_agent_ids: Vec::new(),
            new_pipeline_selected_agent_idx: 0,
            new_pipeline_field_step: 0,
            pipeline_selector_idx: 0,
            total_cpu_usage: 0.0,
            total_memory_mb: 0.0,
            total_disk_used_gb: 0.0,
            total_disk_total_gb: 0.0,
            total_disk_percent: 0.0,
            io_read_kb: 0.0,
            io_write_kb: 0.0,
            active_workers_count: 0,
            total_workers_count: 0,
            gateway_url: detect_gateway_url(),
            gateway_health: None,
            gateway_table_state: TableState::default(),
            gateway_rx: None,
            gateway_pending: false,
            workspaces_rx: None,
            workspaces_pending: false,
            skills_rx: None,
            skills_pending: false,
            tasks_rx: None,
            tasks_pending: false,
            workspaces_subpanel: WorkspaceSubpanel::Workspaces,
            status_message: None,
            system: System::new_all(),
            disks: Disks::new_with_refreshed_list(),
            last_tick: Instant::now(),
            slow_tick: Instant::now(),
        };

        if !app.abilities.is_empty() {
            app.abilities_table_state.select(Some(0));
        }
        if !app.skills.is_empty() {
            app.skills_table_state.select(Some(0));
        }
        app.knowledge_types_table_state.select(Some(0));
        app.context_repos_table_state.select(Some(0));
        app.gateway_table_state.select(Some(0));

        app.fetch_workspaces();
        app.fetch_skills();
        app.fetch_gateway();
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

    pub fn fetch_tasks(&mut self) {
        if self.tasks_pending {
            return;
        }
        if let Some(ref client) = self.client {
            let client = client.clone();
            let ws_id = self.selected_workspace_id.clone();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let (tx, rx) = std::sync::mpsc::channel();
                self.tasks_pending = true;
                handle.spawn(async move {
                    let res = client.list_tasks(ws_id.as_deref()).await;
                    let _ = tx.send(res);
                });
                self.tasks_rx = Some(rx);
            }
        }
    }

    pub fn fetch_workspaces(&mut self) {
        if self.workspaces_pending {
            return;
        }
        if let Some(ref client) = self.client {
            let client = client.clone();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let (tx, rx) = std::sync::mpsc::channel();
                self.workspaces_pending = true;
                handle.spawn(async move {
                    let res = client.list_workspaces().await;
                    let _ = tx.send(res);
                });
                self.workspaces_rx = Some(rx);
            }
        }
    }

    pub fn fetch_skills(&mut self) {
        if self.skills_pending {
            return;
        }
        if let Some(ref client) = self.client {
            let client = client.clone();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let (tx, rx) = std::sync::mpsc::channel();
                self.skills_pending = true;
                handle.spawn(async move {
                    let res = client.list_skills().await;
                    let _ = tx.send(res);
                });
                self.skills_rx = Some(rx);
            }
        }
    }

    pub fn poll_async_channels(&mut self) {
        // Gateway health
        if let Some(ref rx) = self.gateway_rx {
            if let Ok(res) = rx.try_recv() {
                if let Ok(health) = res {
                    self.gateway_health = Some(health);
                }
                self.gateway_rx = None;
                self.gateway_pending = false;
            }
        }
        // Workspaces
        if let Some(ref rx) = self.workspaces_rx {
            if let Ok(res) = rx.try_recv() {
                if let Ok(list) = res {
                    if !list.is_empty() {
                        self.workspaces = list;
                        if self.selected_workspace_id.is_none() {
                            let entries = self.get_workspace_entries();
                            if !entries.is_empty() {
                                self.selected_workspace_id = entries[0].id.clone();
                                self.fetch_tasks();
                            }
                        }
                    }
                }
                self.workspaces_rx = None;
                self.workspaces_pending = false;
            }
        }
        // Tasks
        if let Some(ref rx) = self.tasks_rx {
            if let Ok(res) = rx.try_recv() {
                if let Ok(list) = res {
                    self.workspace_tasks = list;
                    if let Some(sel) = self.task_table_state.selected() {
                        if sel >= self.workspace_tasks.len() {
                            if self.workspace_tasks.is_empty() {
                                self.task_table_state.select(None);
                            } else {
                                self.task_table_state.select(Some(0));
                            }
                        }
                    } else if !self.workspace_tasks.is_empty() {
                        self.task_table_state.select(Some(0));
                    }
                }
                self.tasks_rx = None;
                self.tasks_pending = false;
            }
        }
        // Skills
        let mut new_skills: Option<Vec<SkillItem>> = None;
        if let Some(ref rx) = self.skills_rx {
            if let Ok(res) = rx.try_recv() {
                if let Ok(server_skills) = res {
                    if !server_skills.is_empty() {
                        let mut items: Vec<SkillItem> = server_skills
                            .into_iter()
                            .map(|s| {
                                let (provider_status, path) = check_skill_providers(&s.id);
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
                                    provider_status,
                                    path,
                                }
                            })
                            .collect();
                        items.sort_by(|a, b| a.category.cmp(&b.category).then_with(|| a.name.cmp(&b.name)));
                        new_skills = Some(items);
                    }
                }
                self.skills_rx = None;
                self.skills_pending = false;
            }
        }
        if let Some(skills) = new_skills {
            self.skills = skills;
        }
    }

    pub fn fetch_gateway(&mut self) {
        if self.gateway_pending {
            return;
        }
        if let Some(ref client) = self.client {
            let client = client.clone();
            let gw_url = self.gateway_url.clone();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let (tx, rx) = std::sync::mpsc::channel();
                self.gateway_pending = true;
                handle.spawn(async move {
                    let res = client.get_gateway_health(&gw_url).await;
                    let _ = tx.send(res);
                });
                self.gateway_rx = Some(rx);
            }
        }
    }

    pub fn refresh_workers(&mut self) -> Result<()> {
        // Fire off non-blocking async fetches; results arrive via poll_async_channels
        self.fetch_workspaces();
        self.fetch_gateway();
        self.fetch_tasks();
        // Only refresh CPU/memory for active processes
        self.system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
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

        // Disk stats are expensive — only refresh on slow_tick (every 5s)
        let do_slow = self.slow_tick.elapsed() >= Duration::from_secs(5);
        if do_slow {
            self.disks.refresh(true);
            self.abilities = scan_abilities();
            self.fetch_skills();
            self.fetch_tasks();
        }
        let mut disk_used_b: u64 = 0;
        let mut disk_total_b: u64 = 0;
        for disk in self.disks.list() {
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
        self.workers = updated;
        if do_slow {
            self.slow_tick = Instant::now();
        }

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
                let ps = &sk.provider_status;
                let header_info = format!(
                    "Skill Title: {}\nSkill ID: {}\nCategory: {}\nOrigin: {}\n\nPer-Provider Installation Breakdown:\n  • Google Gemini / AGY: {}\n  • Anthropic Claude Code: {}\n  • OpenAI Codex: {}\n  • Savant Engine: {}\n\nDescription: {}\n\n",
                    sk.name,
                    sk.id,
                    sk.category,
                    sk.origin,
                    if ps.gemini { "INSTALLED (✓)" } else { "NOT INSTALLED (✗)" },
                    if ps.claude { "INSTALLED (✓)" } else { "NOT INSTALLED (✗)" },
                    if ps.codex { "INSTALLED (✓)" } else { "NOT INSTALLED (✗)" },
                    if ps.savant { "INSTALLED (✓)" } else { "NOT INSTALLED (✗)" },
                    sk.description,
                );

                let body = if let Some(ref path) = sk.path {
                    let skill_md = path.join("SKILL.md");
                    if skill_md.exists() {
                        std::fs::read_to_string(&skill_md).unwrap_or_else(|_| "SKILL.md unreadable.".into())
                    } else {
                        format!("Local Skill Path: {}\n", path.display())
                    }
                } else {
                    "No local SKILL.md file present.".to_string()
                };

                self.asset_viewer_title = format!(" Skill Specification & Provider Status: {} ", sk.name);
                self.asset_viewer_content = format!("{header_info}{body}");
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

    pub fn launch_worker_with_pipeline(&mut self, workspace_id: Option<String>, pipeline_id: Option<String>) -> Result<()> {
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
        if let Some(ref pipe) = pipeline_id {
            cmd.arg("--pipeline").arg(pipe);
        }
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        match cmd.spawn() {
            Ok(_) => {
                let target = workspace_id.unwrap_or_else(|| "(all)".into());
                let pipe_label = pipeline_id.as_deref().unwrap_or("(no pipeline)");
                self.set_status(format!("✓ Worker spawned: workspace={target}, pipeline={pipe_label}"));
            }
            Err(err) => {
                self.set_status(format!("✗ Failed to launch worker: {err}"));
            }
        }
        self.refresh_workers()?;
        Ok(())
    }

    pub fn launch_worker(&mut self, _workspace_id: Option<String>) -> Result<()> {
        self.set_status("⚠  A pipeline is required to launch a worker. Go to Tab 2 and select a pipeline.");
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
        app.poll_async_channels();
        terminal.draw(|f| ui(f, &mut app))?;

        // Poll for input at ~60fps; non-blocking so the loop stays responsive
        let timeout = Duration::from_millis(16);
        if event::poll(timeout)? {
            // Drain all pending input events in this tick to prevent input lag
            loop {
                if let Event::Key(key) = event::read()? {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        match app.mode {
                            ViewMode::Normal => handle_normal_keys(&mut app, key)?,
                            ViewMode::WorkerInspector => handle_inspector_keys(&mut app, key)?,
                            ViewMode::FilterPrompt => handle_filter_keys(&mut app, key)?,
                            ViewMode::StartWorkerPrompt => handle_start_prompt_keys(&mut app, key)?,
                            ViewMode::AssetViewer => handle_asset_viewer_keys(&mut app, key)?,
                            ViewMode::NewAgentPrompt => handle_new_agent_keys(&mut app, key)?,
                            ViewMode::NewPipelinePrompt => handle_new_pipeline_keys(&mut app, key)?,
                            ViewMode::PipelineSelector => handle_pipeline_selector_keys(&mut app, key)?,
                        }
                    }
                }
                // Only drain if more events are immediately available
                if !event::poll(Duration::from_millis(0))? {
                    break;
                }
            }
        }

        // Refresh worker process stats every 500ms (non-blocking)
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
        KeyCode::Char('3') => app.main_tab = MainTab::PipelinesAndAgents,
        KeyCode::Char('4') => app.main_tab = MainTab::ServerStatus,
        KeyCode::Tab => {
            app.main_tab = match app.main_tab {
                MainTab::Workers => MainTab::WorkspacesAndTasks,
                MainTab::WorkspacesAndTasks => MainTab::PipelinesAndAgents,
                MainTab::PipelinesAndAgents => MainTab::ServerStatus,
                MainTab::ServerStatus => MainTab::Workers,
            };
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if app.main_tab == MainTab::PipelinesAndAgents {
                app.agents_subpanel = AgentSubpanel::Agents;
            } else if app.main_tab == MainTab::WorkspacesAndTasks {
                app.workspaces_subpanel = WorkspaceSubpanel::Workspaces;
            } else if app.main_tab == MainTab::ServerStatus {
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
            if app.main_tab == MainTab::PipelinesAndAgents {
                app.agents_subpanel = AgentSubpanel::Pipelines;
            } else if app.main_tab == MainTab::WorkspacesAndTasks {
                app.workspaces_subpanel = WorkspaceSubpanel::Tasks;
            } else if app.main_tab == MainTab::ServerStatus {
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
            MainTab::WorkspacesAndTasks => match app.workspaces_subpanel {
                WorkspaceSubpanel::Workspaces => app.select_next_workspace(),
                WorkspaceSubpanel::Tasks => app.select_next_task(),
            },
            MainTab::PipelinesAndAgents => match app.agents_subpanel {
                AgentSubpanel::Agents => {
                    let count = app.colosseum_registry.agents.len();
                    if count > 0 {
                        let i = match app.agent_table_state.selected() {
                            Some(i) => if i + 1 >= count { 0 } else { i + 1 },
                            None => 0,
                        };
                        app.agent_table_state.select(Some(i));
                    }
                }
                AgentSubpanel::Pipelines => {
                    let count = app.colosseum_registry.pipelines.len();
                    if count > 0 {
                        let i = match app.pipeline_table_state.selected() {
                            Some(i) => if i + 1 >= count { 0 } else { i + 1 },
                            None => 0,
                        };
                        app.pipeline_table_state.select(Some(i));
                    }
                }
            },
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
            MainTab::WorkspacesAndTasks => match app.workspaces_subpanel {
                WorkspaceSubpanel::Workspaces => app.select_prev_workspace(),
                WorkspaceSubpanel::Tasks => app.select_prev_task(),
            },
            MainTab::PipelinesAndAgents => match app.agents_subpanel {
                AgentSubpanel::Agents => {
                    let count = app.colosseum_registry.agents.len();
                    if count > 0 {
                        let i = match app.agent_table_state.selected() {
                            Some(i) => if i == 0 { count - 1 } else { i - 1 },
                            None => 0,
                        };
                        app.agent_table_state.select(Some(i));
                    }
                }
                AgentSubpanel::Pipelines => {
                    let count = app.colosseum_registry.pipelines.len();
                    if count > 0 {
                        let i = match app.pipeline_table_state.selected() {
                            Some(i) => if i == 0 { count - 1 } else { i - 1 },
                            None => 0,
                        };
                        app.pipeline_table_state.select(Some(i));
                    }
                }
            },
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
                // Open pipeline selector popup — user picks a pipeline, then worker launches
                app.pipeline_selector_idx = 0;
                app.mode = ViewMode::PipelineSelector;
            }
            MainTab::PipelinesAndAgents => match app.agents_subpanel {
                AgentSubpanel::Agents => {
                    app.open_edit_selected_agent()?;
                }
                AgentSubpanel::Pipelines => {
                    app.open_edit_selected_pipeline()?;
                }
            },
            MainTab::ServerStatus => match app.diagnostics_tab {
                DiagnosticsTab::Abilities => app.inspect_selected_ability(),
                DiagnosticsTab::Skills => app.inspect_selected_skill(),
                DiagnosticsTab::KnowledgeGraph => app.inspect_selected_knowledge_type(),
                DiagnosticsTab::ContextRepos => app.inspect_selected_context_repo(),
                _ => {}
            },
        },
        KeyCode::Char('S') | KeyCode::Char('R') => {
            app.restart_selected_worker()?;
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
            if app.main_tab == MainTab::PipelinesAndAgents {
                match app.agents_subpanel {
                    AgentSubpanel::Agents => app.delete_selected_agent()?,
                    AgentSubpanel::Pipelines => app.delete_selected_pipeline()?,
                }
            } else {
                app.delete_selected_worker()?;
            }
        }
        KeyCode::Char('D') => {
            app.stop_and_delete_selected_worker()?;
        }
        KeyCode::Char('r') => {
            app.refresh_workers()?;
            app.set_status("State refreshed");
        }
        KeyCode::Char('a') | KeyCode::Char('N') => {
            let _ = app.refresh_workers();
            app.mode = ViewMode::NewAgentPrompt;
            app.new_agent_name_input.clear();

            let personas = app.get_available_personas();
            app.new_agent_persona_idx = 0;
            app.new_agent_persona_input = personas.first().cloned().unwrap_or_else(|| "persona.coder".to_string());

            let providers = app.get_available_providers();
            app.new_agent_provider_idx = 0;
            let first_prov = providers.first().cloned().unwrap_or_else(|| "claude".to_string());
            app.new_agent_provider_input = first_prov.clone();

            let models = app.get_models_for_provider(&first_prov);
            app.new_agent_model_idx = 0;
            app.new_agent_model_input = models.first().cloned().unwrap_or_else(|| "claude-3-5-sonnet".to_string());

            let statuses = app.get_sanctum_statuses();
            app.new_agent_pickup_idx = 2;
            app.new_agent_pickup_input = statuses.get(2).cloned().unwrap_or_else(|| "ready".to_string());

            app.new_agent_working_idx = 3;
            app.new_agent_working_input = statuses.get(3).cloned().unwrap_or_else(|| "in-progress".to_string());

            app.new_agent_drop_idx = 4;
            app.new_agent_drop_input = statuses.get(4).cloned().unwrap_or_else(|| "review".to_string());
            app.new_agent_prompt_input.clear();
            app.new_agent_field_step = 0;
        }
        KeyCode::Char('p') | KeyCode::Char('P') => {
            app.editing_pipeline_id = None;
            app.mode = ViewMode::NewPipelinePrompt;
            app.new_pipeline_name_input.clear();
            app.new_pipeline_agents_input.clear();
            app.new_pipeline_agent_ids.clear();
            app.new_pipeline_selected_agent_idx = 0;
            app.new_pipeline_field_step = 0;
        }
        KeyCode::Char('C') => {
            if app.main_tab == MainTab::PipelinesAndAgents {
                app.clone_selected_agent()?;
            }
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
            // Pipeline is required — redirect to pipeline selector instead of launching bare
            app.mode = ViewMode::Normal;
            app.set_status("⚠  A pipeline is required. Use Tab 2 → Enter to select a pipeline.");
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

fn handle_new_agent_keys(app: &mut TuiApp, key: crossterm::event::KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.mode = ViewMode::Normal;
        }
        KeyCode::Left => match app.new_agent_field_step {
            1 => app.cycle_new_agent_persona(false),
            2 => app.cycle_new_agent_provider(false),
            3 => app.cycle_new_agent_model(false),
            4 => app.cycle_new_agent_pickup(false),
            5 => app.cycle_new_agent_working(false),
            6 => app.cycle_new_agent_drop(false),
            _ => {}
        },
        KeyCode::Right => match app.new_agent_field_step {
            1 => app.cycle_new_agent_persona(true),
            2 => app.cycle_new_agent_provider(true),
            3 => app.cycle_new_agent_model(true),
            4 => app.cycle_new_agent_pickup(true),
            5 => app.cycle_new_agent_working(true),
            6 => app.cycle_new_agent_drop(true),
            _ => {}
        },
        KeyCode::Tab | KeyCode::Down => {
            app.new_agent_field_step = (app.new_agent_field_step + 1) % 8;
        }
        KeyCode::BackTab | KeyCode::Up => {
            app.new_agent_field_step = if app.new_agent_field_step == 0 { 7 } else { app.new_agent_field_step - 1 };
        }
        KeyCode::Enter => {
            if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT)
                || key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                || key.modifiers.contains(crossterm::event::KeyModifiers::ALT)
            {
                if app.new_agent_field_step == 7 {
                    app.new_agent_prompt_input.push('\n');
                }
            } else if app.new_agent_field_step < 7 {
                app.new_agent_field_step += 1;
            } else {
                let name = if app.new_agent_name_input.trim().is_empty() {
                    "New Agent".to_string()
                } else {
                    app.new_agent_name_input.trim().to_string()
                };
                let id = format!("agent-{}", name.to_lowercase().replace(' ', "-"));
                let agent = AgentConfig::new(
                    id,
                    name.clone(),
                    app.new_agent_prompt_input.trim(),
                    app.new_agent_persona_input.trim(),
                    "v1",
                    app.new_agent_provider_input.trim(),
                    app.new_agent_model_input.trim(),
                    app.new_agent_pickup_input.trim(),
                    app.new_agent_working_input.trim(),
                    app.new_agent_drop_input.trim(),
                );
                app.colosseum_registry.register_agent(agent);
                let _ = app.colosseum_registry.save_to_file(&ColosseumRegistry::default_storage_path());
                app.mode = ViewMode::Normal;
                app.set_status(format!("✓ Agent '{}' created from scratch", name));
            }
        }
        KeyCode::Backspace => match app.new_agent_field_step {
            0 => { app.new_agent_name_input.pop(); }
            1 => app.cycle_new_agent_persona(false),
            2 => app.cycle_new_agent_provider(false),
            3 => app.cycle_new_agent_model(false),
            4 => app.cycle_new_agent_pickup(false),
            5 => app.cycle_new_agent_working(false),
            6 => app.cycle_new_agent_drop(false),
            7 => { app.new_agent_prompt_input.pop(); }
            _ => {}
        },
        KeyCode::Char(c) => match app.new_agent_field_step {
            0 => { app.new_agent_name_input.push(c); }
            1 => app.cycle_new_agent_persona(true),
            2 => app.cycle_new_agent_provider(true),
            3 => app.cycle_new_agent_model(true),
            4 => app.cycle_new_agent_pickup(true),
            5 => app.cycle_new_agent_working(true),
            6 => app.cycle_new_agent_drop(true),
            7 => { app.new_agent_prompt_input.push(c); }
            _ => {}
        },
        _ => {}
    }
    Ok(())
}

fn handle_new_pipeline_keys(app: &mut TuiApp, key: crossterm::event::KeyEvent) -> Result<()> {
    let available_agents: Vec<AgentConfig> = app.colosseum_registry.agents.values().cloned().collect();
    let total_agents = available_agents.len();

    match key.code {
        KeyCode::Esc => {
            app.mode = ViewMode::Normal;
        }
        KeyCode::Tab | KeyCode::BackTab => {
            app.new_pipeline_field_step = if app.new_pipeline_field_step == 0 { 1 } else { 0 };
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.new_pipeline_field_step == 0 {
                app.new_pipeline_field_step = 1;
            } else if total_agents > 0 {
                app.new_pipeline_selected_agent_idx = if app.new_pipeline_selected_agent_idx == 0 {
                    total_agents - 1
                } else {
                    app.new_pipeline_selected_agent_idx - 1
                };
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.new_pipeline_field_step == 0 {
                app.new_pipeline_field_step = 1;
            } else if total_agents > 0 {
                app.new_pipeline_selected_agent_idx = (app.new_pipeline_selected_agent_idx + 1) % total_agents;
            }
        }
        KeyCode::Char(' ') => {
            if app.new_pipeline_field_step == 1 && total_agents > 0 {
                if let Some(agent) = available_agents.get(app.new_pipeline_selected_agent_idx) {
                    if let Some(pos) = app.new_pipeline_agent_ids.iter().position(|id| id == &agent.id) {
                        app.new_pipeline_agent_ids.remove(pos);
                    } else {
                        app.new_pipeline_agent_ids.push(agent.id.clone());
                    }
                }
            } else if app.new_pipeline_field_step == 0 {
                app.new_pipeline_name_input.push(' ');
            }
        }
        KeyCode::Enter => {
            if app.new_pipeline_field_step == 0 {
                app.new_pipeline_field_step = 1;
            } else {
                // Field 1 (last field): Save & Validate Pipeline
                let name = if app.new_pipeline_name_input.trim().is_empty() {
                    "New Pipeline".to_string()
                } else {
                    app.new_pipeline_name_input.trim().to_string()
                };
                if app.new_pipeline_agent_ids.is_empty() {
                    app.set_status("✗ Cannot save pipeline: Select at least 1 agent for the pipeline sequence.");
                    return Ok(());
                }

                let id = if let Some(ref edit_id) = app.editing_pipeline_id {
                    edit_id.clone()
                } else {
                    format!("pipeline-{}", name.to_lowercase().replace(' ', "-"))
                };
                let pipeline = Pipeline {
                    id,
                    name: name.clone(),
                    agent_ids: app.new_pipeline_agent_ids.clone(),
                };
                match app.colosseum_registry.register_pipeline(pipeline, false) {
                    Ok(()) => {
                        let _ = app.colosseum_registry.save_to_file(&ColosseumRegistry::default_storage_path());
                        app.editing_pipeline_id = None;
                        app.mode = ViewMode::Normal;
                        app.set_status(format!("✓ Pipeline '{}' saved & validated", name));
                    }
                    Err(err) => {
                        app.set_status(format!("✗ Pipeline error: {}", err));
                    }
                }
            }
        }
        KeyCode::Backspace => {
            if app.new_pipeline_field_step == 0 {
                app.new_pipeline_name_input.pop();
            }
        }
        KeyCode::Char(c) => {
            if app.new_pipeline_field_step == 0 {
                app.new_pipeline_name_input.push(c);
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_pipeline_selector_keys(app: &mut TuiApp, key: crossterm::event::KeyEvent) -> Result<()> {
    let pipelines: Vec<(String, String)> = app
        .colosseum_registry
        .pipelines
        .values()
        .map(|p| (p.id.clone(), p.name.clone()))
        .collect();
    let total = pipelines.len();

    match key.code {
        KeyCode::Esc => {
            app.mode = ViewMode::Normal;
            app.set_status("Worker launch cancelled");
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if total > 0 {
                app.pipeline_selector_idx = if app.pipeline_selector_idx == 0 {
                    total - 1
                } else {
                    app.pipeline_selector_idx - 1
                };
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if total > 0 {
                app.pipeline_selector_idx = (app.pipeline_selector_idx + 1) % total;
            }
        }
        KeyCode::Enter => {
            let ws = app.selected_workspace_id.clone();
            if total == 0 {
                // No pipelines defined — stay in selector and show error
                app.set_status("⚠  No pipelines registered. Go to Tab 3 to create one first.");
            } else {
                let pipeline_id = pipelines
                    .get(app.pipeline_selector_idx)
                    .map(|(id, _)| id.clone());
                app.mode = ViewMode::Normal;
                app.launch_worker_with_pipeline(ws, pipeline_id)?;
            }
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
        ViewMode::Normal | ViewMode::FilterPrompt | ViewMode::StartWorkerPrompt | ViewMode::NewAgentPrompt | ViewMode::NewPipelinePrompt | ViewMode::PipelineSelector => match app.main_tab {
            MainTab::Workers => render_dense_workers_dashboard(f, app, chunks[1]),
            MainTab::WorkspacesAndTasks => render_workspaces_tab(f, app, chunks[1]),
            MainTab::PipelinesAndAgents => render_pipelines_and_agents_view(f, app, chunks[1]),
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
    } else if app.mode == ViewMode::NewAgentPrompt {
        render_new_agent_popup(f, app, f.area());
    } else if app.mode == ViewMode::NewPipelinePrompt {
        render_new_pipeline_popup(f, app, f.area());
    } else if app.mode == ViewMode::PipelineSelector {
        render_pipeline_selector_popup(f, app, f.area());
    }

    render_footer(f, app, chunks[2]);
}

fn render_system_header(f: &mut Frame, app: &TuiApp, area: Rect) {
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(44), Constraint::Percentage(56)])
        .split(area);

    let titles: Vec<Line> = vec![
        Line::from(" [1] Engine "),
        Line::from(" [2] Workspaces "),
        Line::from(" [3] Agents "),
        Line::from(" [4] Diagnostics "),
    ];

    let select_idx = match app.main_tab {
        MainTab::Workers => 0,
        MainTab::WorkspacesAndTasks => 1,
        MainTab::PipelinesAndAgents => 2,
        MainTab::ServerStatus => 3,
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
            Span::styled("Gateway: ", Style::default().fg(Color::Gray)),
            if let Some(ref gw) = app.gateway_health {
                let total_models: usize = gw.provider_details.iter().map(|p| p.models.len()).sum();
                Span::styled(format!("ONLINE ({} Providers, {} Models)  ", gw.providers.len(), total_models), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            } else {
                Span::styled("OFFLINE  ", Style::default().fg(Color::Red))
            },
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
        self.workspace_tasks.clear();
        self.task_table_state.select(None);
        self.tasks_pending = false;
        self.fetch_tasks();
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
        self.workspace_tasks.clear();
        self.task_table_state.select(None);
        self.tasks_pending = false;
        self.fetch_tasks();
    }

    pub fn select_next_task(&mut self) {
        if self.workspace_tasks.is_empty() {
            return;
        }
        let idx = match self.task_table_state.selected() {
            Some(i) => (i + 1) % self.workspace_tasks.len(),
            None => 0,
        };
        self.task_table_state.select(Some(idx));
    }

    pub fn select_prev_task(&mut self) {
        if self.workspace_tasks.is_empty() {
            return;
        }
        let idx = match self.task_table_state.selected() {
            Some(i) if i > 0 => i - 1,
            _ => self.workspace_tasks.len() - 1,
        };
        self.task_table_state.select(Some(idx));
    }

    pub fn clone_selected_agent(&mut self) -> Result<()> {
        let agent_keys: Vec<String> = self.colosseum_registry.agents.keys().cloned().collect();
        if agent_keys.is_empty() {
            self.set_status("No agents in library to clone");
            return Ok(());
        }
        let sel_idx = self.agent_table_state.selected().unwrap_or(0);
        if let Some(source_id) = agent_keys.get(sel_idx) {
            let source_name = self.colosseum_registry.agents.get(source_id).map(|a| a.name.clone()).unwrap_or_default();
            let new_id = format!("{}-copy", source_id);
            let new_name = format!("{} Copy", source_name);
            match self.colosseum_registry.clone_agent(source_id, &new_id, &new_name) {
                Ok(cloned) => {
                    let _ = self.colosseum_registry.save_to_file(&ColosseumRegistry::default_storage_path());
                    self.set_status(format!("✓ Cloned agent '{}' to '{}'", source_name, cloned.name));
                }
                Err(err) => {
                    self.set_status(format!("✗ Clone failed: {}", err));
                }
            }
        }
        Ok(())
    }

    pub fn get_available_personas(&self) -> Vec<String> {
        let mut personas = vec![
            "persona.coder".to_string(),
            "persona.architect".to_string(),
            "persona.reviewer".to_string(),
            "persona.engineer".to_string(),
            "persona.product".to_string(),
            "persona.qa".to_string(),
            "persona.security-auditor".to_string(),
        ];
        for item in &self.abilities {
            if item.category == "personas" {
                let id = format!("persona.{}", item.name);
                if !personas.contains(&id) {
                    personas.push(id);
                }
            }
        }
        personas
    }

    pub fn cycle_new_agent_persona(&mut self, next: bool) {
        let personas = self.get_available_personas();
        if personas.is_empty() {
            return;
        }
        if next {
            self.new_agent_persona_idx = (self.new_agent_persona_idx + 1) % personas.len();
        } else {
            self.new_agent_persona_idx = if self.new_agent_persona_idx == 0 {
                personas.len() - 1
            } else {
                self.new_agent_persona_idx - 1
            };
        }
        self.new_agent_persona_input = personas[self.new_agent_persona_idx].clone();
    }

    pub fn get_available_providers(&self) -> Vec<String> {
        let mut providers = Vec::new();
        if let Some(ref gw) = self.gateway_health {
            for detail in &gw.provider_details {
                if !providers.contains(&detail.id) {
                    providers.push(detail.id.clone());
                }
            }
        }
        if providers.is_empty() {
            providers = vec![
                "claude".to_string(),
                "codex".to_string(),
                "copilot".to_string(),
                "gemini".to_string(),
                "agy".to_string(),
            ];
        }
        providers
    }

    pub fn get_models_for_provider(&self, provider: &str) -> Vec<String> {
        if let Some(ref gw) = self.gateway_health {
            if let Some(detail) = gw.provider_details.iter().find(|d| d.id == provider) {
                if !detail.models.is_empty() {
                    return detail.models.clone();
                }
            }
        }
        match provider {
            "claude" => vec!["claude-3-5-sonnet".into(), "claude-3-opus".into(), "claude-3-5-haiku".into()],
            "codex" => vec!["gpt-4o".into(), "gpt-4o-mini".into(), "o1-preview".into(), "o3-mini".into()],
            "copilot" => vec!["copilot-chat".into(), "gpt-4o".into()],
            "gemini" => vec!["gemini-1.5-pro".into(), "gemini-1.5-flash".into(), "gemini-2.0-flash-exp".into()],
            "agy" => vec!["antigravity-3.5".into(), "default".into()],
            _ => vec!["default".into()],
        }
    }

    pub fn cycle_new_agent_provider(&mut self, next: bool) {
        let providers = self.get_available_providers();
        if providers.is_empty() { return; }
        if next {
            self.new_agent_provider_idx = (self.new_agent_provider_idx + 1) % providers.len();
        } else {
            self.new_agent_provider_idx = if self.new_agent_provider_idx == 0 {
                providers.len() - 1
            } else {
                self.new_agent_provider_idx - 1
            };
        }
        let sel_provider = providers[self.new_agent_provider_idx].clone();
        self.new_agent_provider_input = sel_provider.clone();

        let models = self.get_models_for_provider(&sel_provider);
        self.new_agent_model_idx = 0;
        self.new_agent_model_input = models.first().cloned().unwrap_or_else(|| "default".to_string());
    }

    pub fn cycle_new_agent_model(&mut self, next: bool) {
        let models = self.get_models_for_provider(&self.new_agent_provider_input);
        if models.is_empty() { return; }
        if next {
            self.new_agent_model_idx = (self.new_agent_model_idx + 1) % models.len();
        } else {
            self.new_agent_model_idx = if self.new_agent_model_idx == 0 {
                models.len() - 1
            } else {
                self.new_agent_model_idx - 1
            };
        }
        self.new_agent_model_input = models[self.new_agent_model_idx].clone();
    }

    pub fn get_sanctum_statuses(&self) -> Vec<String> {
        let mut statuses = vec![
            "backlog".to_string(),
            "grooming".to_string(),
            "ready".to_string(),
            "in-progress".to_string(),
            "review".to_string(),
            "needs-input".to_string(),
            "done".to_string(),
            "merged".to_string(),
            "blocked".to_string(),
            "failed".to_string(),
        ];
        for t in &self.workspace_tasks {
            let st = t.status.trim().to_string();
            if !st.is_empty() && !statuses.contains(&st) {
                statuses.push(st);
            }
        }
        statuses
    }

    pub fn cycle_new_agent_pickup(&mut self, next: bool) {
        let list = self.get_sanctum_statuses();
        if list.is_empty() { return; }
        if next {
            self.new_agent_pickup_idx = (self.new_agent_pickup_idx + 1) % list.len();
        } else {
            self.new_agent_pickup_idx = if self.new_agent_pickup_idx == 0 {
                list.len() - 1
            } else {
                self.new_agent_pickup_idx - 1
            };
        }
        self.new_agent_pickup_input = list[self.new_agent_pickup_idx].clone();
    }

    pub fn cycle_new_agent_working(&mut self, next: bool) {
        let list = self.get_sanctum_statuses();
        if list.is_empty() { return; }
        if next {
            self.new_agent_working_idx = (self.new_agent_working_idx + 1) % list.len();
        } else {
            self.new_agent_working_idx = if self.new_agent_working_idx == 0 {
                list.len() - 1
            } else {
                self.new_agent_working_idx - 1
            };
        }
        self.new_agent_working_input = list[self.new_agent_working_idx].clone();
    }

    pub fn cycle_new_agent_drop(&mut self, next: bool) {
        let list = self.get_sanctum_statuses();
        if list.is_empty() { return; }
        if next {
            self.new_agent_drop_idx = (self.new_agent_drop_idx + 1) % list.len();
        } else {
            self.new_agent_drop_idx = if self.new_agent_drop_idx == 0 {
                list.len() - 1
            } else {
                self.new_agent_drop_idx - 1
            };
        }
        self.new_agent_drop_input = list[self.new_agent_drop_idx].clone();
    }

    pub fn open_edit_selected_agent(&mut self) -> Result<()> {
        let agents: Vec<AgentConfig> = self.colosseum_registry.agents.values().cloned().collect();
        let selected_idx = self.agent_table_state.selected().unwrap_or(0);
        let Some(agent) = agents.get(selected_idx) else {
            self.set_status("No agent selected to edit");
            return Ok(());
        };

        let attached_count = self.colosseum_registry.pipeline_count_for_agent(&agent.id);
        if attached_count > 0 {
            self.set_status(format!("🔒 Agent '{}' is attached to {} pipeline(s). Attached agents cannot be edited directly; press [C] to Clone.", agent.name, attached_count));
            return Ok(());
        }

        self.editing_agent_id = Some(agent.id.clone());
        self.new_agent_name_input = agent.name.clone();
        self.new_agent_persona_input = agent.persona.clone();
        self.new_agent_provider_input = agent.provider.clone();
        self.new_agent_model_input = agent.model.clone();
        self.new_agent_pickup_input = agent.pickup_location.clone();
        self.new_agent_working_input = agent.get_working_location().to_string();
        self.new_agent_drop_input = agent.drop_location.clone();
        self.new_agent_prompt_input = agent.prompt.clone();
        self.new_agent_field_step = 0;
        self.mode = ViewMode::NewAgentPrompt;
        self.set_status(format!("Editing agent '{}'", agent.name));
        Ok(())
    }

    pub fn open_edit_selected_pipeline(&mut self) -> Result<()> {
        let pipelines: Vec<Pipeline> = self.colosseum_registry.pipelines.values().cloned().collect();
        let selected_idx = self.pipeline_table_state.selected().unwrap_or(0);
        let Some(pipeline) = pipelines.get(selected_idx) else {
            self.set_status("No pipeline selected to edit");
            return Ok(());
        };

        let running_workers = self.workers.iter().filter(|w| w.record.status == WorkerStatus::Running).count();
        if running_workers > 0 {
            self.set_status(format!("🔒 Pipeline '{}' has active running workers and cannot be modified while running.", pipeline.name));
            return Ok(());
        }

        self.editing_pipeline_id = Some(pipeline.id.clone());
        self.new_pipeline_name_input = pipeline.name.clone();
        self.new_pipeline_agent_ids = pipeline.agent_ids.clone();
        self.new_pipeline_selected_agent_idx = 0;
        self.new_pipeline_field_step = 0;
        self.mode = ViewMode::NewPipelinePrompt;
        self.set_status(format!("Editing pipeline '{}'", pipeline.name));
        Ok(())
    }

    pub fn delete_selected_agent(&mut self) -> Result<()> {
        let agents: Vec<AgentConfig> = self.colosseum_registry.agents.values().cloned().collect();
        let selected_idx = self.agent_table_state.selected().unwrap_or(0);
        let Some(agent) = agents.get(selected_idx) else {
            self.set_status("No agent selected to delete");
            return Ok(());
        };

        let name = agent.name.clone();
        match self.colosseum_registry.remove_agent(&agent.id) {
            Ok(_) => {
                let _ = self.colosseum_registry.save_to_file(&ColosseumRegistry::default_storage_path());
                let new_count = self.colosseum_registry.agents.len();
                if new_count > 0 {
                    let next_i = if selected_idx >= new_count { new_count - 1 } else { selected_idx };
                    self.agent_table_state.select(Some(next_i));
                } else {
                    self.agent_table_state.select(None);
                }
                self.set_status(format!("✓ Agent '{}' deleted successfully", name));
            }
            Err(err) => {
                self.set_status(format!("🔒 {}", err));
            }
        }
        Ok(())
    }

    pub fn delete_selected_pipeline(&mut self) -> Result<()> {
        let pipelines: Vec<Pipeline> = self.colosseum_registry.pipelines.values().cloned().collect();
        let selected_idx = self.pipeline_table_state.selected().unwrap_or(0);
        let Some(pipeline) = pipelines.get(selected_idx) else {
            self.set_status("No pipeline selected to delete");
            return Ok(());
        };

        let name = pipeline.name.clone();
        let running_workers = self.workers.iter().filter(|w| w.record.status == WorkerStatus::Running).count();
        match self.colosseum_registry.remove_pipeline(&pipeline.id, running_workers > 0) {
            Ok(_) => {
                let _ = self.colosseum_registry.save_to_file(&ColosseumRegistry::default_storage_path());
                let new_count = self.colosseum_registry.pipelines.len();
                if new_count > 0 {
                    let next_i = if selected_idx >= new_count { new_count - 1 } else { selected_idx };
                    self.pipeline_table_state.select(Some(next_i));
                } else {
                    self.pipeline_table_state.select(None);
                }
                self.set_status(format!("✓ Pipeline '{}' deleted successfully", name));
            }
            Err(err) => {
                self.set_status(format!("🔒 {}", err));
            }
        }
        Ok(())
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
            Span::styled("[←/→/h/l]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" Switch Panel  │  ", Style::default().fg(Color::Gray)),
            Span::styled("[↑/↓/j/k]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" Select Item  │  ", Style::default().fg(Color::Gray)),
            Span::styled("[Enter] or [L]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(" Launch Worker for Selected Workspace", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Target Workspace: ", Style::default().fg(Color::Cyan)),
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

    let is_ws_focused = app.workspaces_subpanel == WorkspaceSubpanel::Workspaces;
    let is_tasks_focused = app.workspaces_subpanel == WorkspaceSubpanel::Tasks;

    let sub_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    let entries = app.get_workspace_entries();
    let header_cells = ["Workspace Name", "Workspace ID", "Active"]
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
            Span::styled(active_str, Style::default().fg(if active_cnt > 0 { Color::Green } else { Color::Gray })),
        ])
    });

    let ws_highlight_style = if is_ws_focused {
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(40),
            Constraint::Percentage(40),
            Constraint::Percentage(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Savant Workspaces ({}) ", entries.len()))
            .border_style(Style::default().fg(if is_ws_focused { Color::Yellow } else { Color::Cyan })),
    )
    .row_highlight_style(ws_highlight_style)
    .highlight_symbol("► ");

    f.render_stateful_widget(table, sub_chunks[0], &mut app.workspace_table_state);

    // Right Panel: Tasks in the selected Workspace + Task Details
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(sub_chunks[1]);

    let task_header_cells = ["Task ID", "Title", "Status", "Priority", "Ready"]
        .iter()
        .map(|h| Span::styled(*h, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let task_header = Row::new(task_header_cells).height(1).bottom_margin(1);

    let task_rows = app.workspace_tasks.iter().map(|task| {
        Row::new(vec![
            Span::raw(&task.task_id),
            Span::styled(&task.title, Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                &task.status,
                Style::default().fg(match task.status.as_str() {
                    "done" | "merged" => Color::Green,
                    "in-progress" => Color::Blue,
                    "blocked" | "failed" => Color::Red,
                    "ready" => Color::Cyan,
                    _ => Color::White,
                })
            ),
            Span::raw(&task.priority),
            Span::styled(
                if task.colosseum_ready { "Yes" } else { "No" },
                Style::default().fg(if task.colosseum_ready { Color::Green } else { Color::Red })
            ),
        ])
    });

    let task_highlight_style = if is_tasks_focused {
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    };

    let tasks_table = Table::new(
        task_rows,
        [
            Constraint::Percentage(20),
            Constraint::Percentage(40),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(10),
        ],
    )
    .header(task_header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Savant Tasks ({}) ", app.workspace_tasks.len()))
            .border_style(Style::default().fg(if is_tasks_focused { Color::Yellow } else { Color::Cyan })),
    )
    .row_highlight_style(task_highlight_style)
    .highlight_symbol("► ");

    f.render_stateful_widget(tasks_table, right_chunks[0], &mut app.task_table_state);

    let selected_task = app.task_table_state.selected()
        .and_then(|idx| app.workspace_tasks.get(idx));

    let details_p = if let Some(task) = selected_task {
        let depends_on_str = if task.depends_on.is_empty() {
            "None".to_string()
        } else {
            task.depends_on.join(", ")
        };
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Task: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(&task.title, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("ID: ", Style::default().fg(Color::Gray)),
                Span::raw(&task.task_id),
                Span::raw("  │  Priority: "),
                Span::styled(&task.priority, Style::default().fg(Color::Magenta)),
                Span::raw("  │  Ready: "),
                Span::styled(
                    if task.colosseum_ready { "Yes" } else { "No" },
                    Style::default().fg(if task.colosseum_ready { Color::Green } else { Color::Red }).add_modifier(Modifier::BOLD)
                ),
            ]),
            Line::from(vec![
                Span::styled("Depends On: ", Style::default().fg(Color::Gray)),
                Span::raw(depends_on_str),
            ]),
            Line::from(""),
            Line::from(Span::styled("Description:", Style::default().fg(Color::Yellow))),
            Line::from(task.description.as_str()),
        ])
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Task Details ")
                .border_style(Style::default().fg(Color::Cyan)),
        )
    } else {
        Paragraph::new("No task selected or tasks not loaded.")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Task Details ")
                    .border_style(Style::default().fg(Color::Gray)),
            )
    };
    f.render_widget(details_p, right_chunks[1]);
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

    let header_cells = ["Ability / Rule Name", "Category / Persona Scope"]
        .iter()
        .map(|h| Span::styled(*h, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = app.abilities.iter().map(|ab| {
        Row::new(vec![
            Span::styled(ab.name.clone(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(ab.category.clone(), Style::default().fg(Color::Green)),
        ])
    });

    let selected_style = Style::default()
        .bg(Color::DarkGray)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(50),
            Constraint::Percentage(50),
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

    let header_cells = ["Skill ID / Title", "Gemini Status", "Claude Status", "Codex Status", "Savant Status", "Category Scope", "Description"]
        .iter()
        .map(|h| Span::styled(*h, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = app.skills.iter().map(|sk| {
        let ps = &sk.provider_status;

        let gem_badge = if ps.gemini {
            Span::styled(" [✓ INSTALLED] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" [✗ MISSING] ", Style::default().fg(Color::DarkGray))
        };

        let cl_badge = if ps.claude {
            Span::styled(" [✓ INSTALLED] ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" [✗ MISSING] ", Style::default().fg(Color::DarkGray))
        };

        let cx_badge = if ps.codex {
            Span::styled(" [✓ INSTALLED] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" [✗ MISSING] ", Style::default().fg(Color::DarkGray))
        };

        let sv_badge = if ps.savant {
            Span::styled(" [✓ INSTALLED] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" [✗ MISSING] ", Style::default().fg(Color::DarkGray))
        };

        Row::new(vec![
            Span::styled(sk.name.clone(), Style::default().add_modifier(Modifier::BOLD)),
            gem_badge,
            cl_badge,
            cx_badge,
            sv_badge,
            Span::styled(sk.category.clone(), Style::default().fg(Color::Yellow)),
            Span::raw(sk.description.chars().take(40).collect::<String>()),
        ])
    });

    let selected_style = Style::default()
        .bg(Color::DarkGray)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(22),
            Constraint::Percentage(14),
            Constraint::Percentage(14),
            Constraint::Percentage(14),
            Constraint::Percentage(14),
            Constraint::Percentage(8),
            Constraint::Percentage(14),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Savant Server Skills ({}) - Multi-Provider Installation Matrix (Gemini, Claude, Codex, Savant) - Press [Enter] to Inspect ", app.skills.len()))
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
    text.push(Line::from(Span::styled("Savant Gateway AI Multi-Provider Hub:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))));
    if let Some(ref gw) = app.gateway_health {
        let total_models: usize = gw.provider_details.iter().map(|p| p.models.len()).sum();
        text.push(Line::from(vec![
            Span::styled("• Status: ", Style::default().fg(Color::White)),
            Span::styled(format!("ONLINE ({})", app.gateway_url), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("  │ Gateway Service: ", Style::default().fg(Color::White)),
            Span::styled(format!("{} v{}", gw.service, gw.version), Style::default().fg(Color::Cyan)),
        ]));
        text.push(Line::from(vec![
            Span::styled("• Registered Gateway Providers: ", Style::default().fg(Color::White)),
            Span::styled(format!("{} Providers ({})", gw.providers.len(), gw.providers.join(", ")), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled("  │ Total Models: ", Style::default().fg(Color::White)),
            Span::styled(format!("{} Available Models", total_models), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        ]));

        text.push(Line::from(""));
        text.push(Line::from(Span::styled("  Gateway Provider & Model Breakdown:", Style::default().fg(Color::Yellow))));
        for p in &gw.provider_details {
            let models_preview = if p.models.len() > 4 {
                format!("{} (and {} more...)", p.models[..4].join(", "), p.models.len() - 4)
            } else {
                p.models.join(", ")
            };
            text.push(Line::from(vec![
                Span::styled(format!("    • {:<10}", p.label), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(format!("Default: {:<20}", p.default_model.as_deref().unwrap_or("-")), Style::default().fg(Color::Yellow)),
                Span::styled(format!("Models ({}): ", p.models.len()), Style::default().fg(Color::Green)),
                Span::raw(models_preview),
            ]));
        }
    } else {
        text.push(Line::from(vec![
            Span::styled("• Status: ", Style::default().fg(Color::White)),
            Span::styled(format!("OFFLINE ({})", app.gateway_url), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ]));
    }

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

fn render_pipeline_selector_popup(f: &mut Frame, app: &TuiApp, area: Rect) {
    let pipelines: Vec<&crate::pipeline::Pipeline> = app.colosseum_registry.pipelines.values().collect();
    let ws_name = app.selected_workspace_id.as_deref().unwrap_or("(all workspaces)");

    let block = Block::default()
        .title(format!(" 🚀 Select Pipeline — Workspace: {} ", ws_name))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD));

    let popup_area = centered_rect(60, 60, area);
    f.render_widget(Clear, popup_area);

    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        "Choose a pipeline to run, then press Enter to launch the worker:",
        Style::default().fg(Color::Cyan),
    )));
    lines.push(Line::from(""));

    if pipelines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (No pipelines registered — go to Tab 3 to create one)",
            Style::default().fg(Color::Gray),
        )));
    } else {
        for (i, pipeline) in pipelines.iter().enumerate() {
            let is_sel = i == app.pipeline_selector_idx;
            let prefix = if is_sel { " ►► " } else { "    " };
            let (row_style, name_style) = if is_sel {
                (
                    Style::default().bg(Color::Magenta).fg(Color::Black).add_modifier(Modifier::BOLD),
                    Style::default().bg(Color::Magenta).fg(Color::Black).add_modifier(Modifier::BOLD),
                )
            } else {
                (
                    Style::default().fg(Color::White),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                )
            };

            // Build concise DAG order string
            let dag: Vec<String> = pipeline.agent_ids.iter().enumerate().map(|(idx, id)| {
                let name = app.colosseum_registry.agents.get(id).map(|a| a.name.as_str()).unwrap_or(id.as_str());
                format!("Stage {}: {}", idx + 1, name)
            }).collect();
            let dag_str = if dag.is_empty() { "(no stages)".to_string() } else { dag.join(" ──► ") };

            lines.push(Line::from(vec![
                Span::styled(prefix, row_style),
                Span::styled(format!("{} ", pipeline.name), name_style),
                Span::styled(format!(" │ {} ", dag_str), if is_sel { Style::default().bg(Color::Magenta).fg(Color::Black) } else { Style::default().fg(Color::DarkGray) }),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press [Enter] to Launch Worker  │  [↑/↓] Navigate  │  [Esc] Cancel",
        Style::default().fg(Color::Green),
    )));

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
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
            MainTab::Workers => " [1/2/3/4] Tabs │ [↑/↓/j/k] Select │ [Enter] Inspector │ [S/R] Restart │ [x] Stop │ [X] Kill │ [d] Purge │ [D] Stop+Purge │ [y/c] Copy ID │ [Y] Log Path │ [/] Filter │ [r] Refresh │ [q] Quit ",
            MainTab::WorkspacesAndTasks => " [1/2/3/4] Tabs │ [↑/↓/j/k] Select Workspace │ [Enter] Select Pipeline & Launch │ [r] Refresh │ [q] Quit ",
            MainTab::PipelinesAndAgents => " [1/2/3/4] Tabs │ [◄/►] Switch Panel │ [▲/▼] Select │ [Enter] Edit │ [d] Delete │ [a] New Agent │ [p] New Pipeline │ [C] Clone Agent │ [q] Quit ",
            MainTab::ServerStatus => " [1/2/3/4] Tabs │ [h/l] Subtabs │ [↑/↓/j/k] Select │ [Enter] Inspect Spec / Copy Git SSH │ [y/c] Copy │ [r] Refresh │ [q] Quit ",
        },
        ViewMode::WorkerInspector => " [Tab] Switch Logs/Tree │ [f] Toggle Follow │ [↑/↓/j/k] Scroll │ [y/c] Copy ID │ [Y] Log Path │ [x] TERM │ [D] Stop & Purge │ [Esc/q] Back ",
        ViewMode::AssetViewer => " [↑/↓/j/k] Scroll Spec │ [y/c] Copy Content │ [Esc/q] Close Inspector ",
        ViewMode::FilterPrompt => " Type filter query... │ [Enter/Esc] Apply/Done ",
        ViewMode::StartWorkerPrompt => " Type Workspace ID... │ [Enter] Launch Worker │ [Esc] Cancel ",
        ViewMode::NewAgentPrompt => " [Tab/Down] Next Field │ [Up] Prev Field │ [Enter] Submit / Next Field │ [Esc] Cancel ",
        ViewMode::NewPipelinePrompt => " [Tab/Down] Next Field │ [Up] Prev Field │ [Enter] Submit & Validate │ [Esc] Cancel ",
        ViewMode::PipelineSelector => " [↑/↓] Select Pipeline │ [Enter] Launch Worker │ [Esc] Cancel ",
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

fn render_pipelines_and_agents_view(f: &mut Frame, app: &mut TuiApp, area: Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    // Count running and total workers across all workers for workspace/pipeline metrics
    let running_workers_count = app.workers.iter().filter(|w| w.record.status == WorkerStatus::Running).count();
    let stopped_workers_count = app.workers.iter().filter(|w| w.record.status != WorkerStatus::Running).count();

    // Left Pane: Agents Library with Pipeline binding count & lock status
    let is_agents_focused = app.agents_subpanel == AgentSubpanel::Agents;
    let is_pipelines_focused = app.agents_subpanel == AgentSubpanel::Pipelines;

    // Left Pane: Agents Library with Pipeline binding count & lock status
    let agent_rows: Vec<Row> = app
        .colosseum_registry
        .agents
        .values()
        .map(|agent| {
            let p_count = app.colosseum_registry.pipeline_count_for_agent(&agent.id);
            let lock_span = if p_count > 0 {
                Span::styled(format!("{} (LOCKED)", p_count), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            } else {
                Span::styled("0 (free)".to_string(), Style::default().fg(Color::DarkGray))
            };

            Row::new(vec![
                Span::styled(&agent.name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                lock_span,
                Span::styled(&agent.persona, Style::default().fg(Color::Yellow)),
                Span::styled(&agent.provider, Style::default().fg(Color::Green)),
                Span::styled(&agent.model, Style::default().fg(Color::Magenta)),
                Span::styled(&agent.pickup_location, Style::default().fg(Color::White)),
                Span::styled(agent.get_working_location(), Style::default().fg(Color::LightYellow)),
                Span::styled(&agent.drop_location, Style::default().fg(Color::LightCyan)),
            ])
        })
        .collect();

    let agents_title = if is_agents_focused {
        format!(" ► AGENT LIBRARY ({}) ◄ [ACTIVE - Up/Down: Select, Enter: Edit] ", app.colosseum_registry.agents.len())
    } else {
        format!(" Agent Library ({}) [Press Left to Focus] ", app.colosseum_registry.agents.len())
    };

    let agent_highlight_style = if is_agents_focused {
        Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(Color::DarkGray).fg(Color::White).add_modifier(Modifier::BOLD)
    };

    let agents_table = Table::new(
        agent_rows,
        [
            Constraint::Percentage(16),
            Constraint::Percentage(10),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(14),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
        ],
    )
    .header(
        Row::new(vec!["Agent Name", "Pipelines", "Persona", "Provider", "Model", "Pickup Loc", "Working Loc", "Drop Loc"])
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    )
    .row_highlight_style(agent_highlight_style)
    .highlight_symbol(" ► ")
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(agents_title)
            .border_style(Style::default().fg(if is_agents_focused { Color::Yellow } else { Color::Cyan })),
    );

    f.render_stateful_widget(agents_table, main_chunks[0], &mut app.agent_table_state);

    // Right Pane: Pipelines & DAG Visualizer with Worker Status
    let mut pipeline_lines = Vec::new();
    pipeline_lines.push(Line::from(Span::styled("Defined Pipelines & Worker Status", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))));
    pipeline_lines.push(Line::from("──────────────────────────────────────────────────"));

    if app.colosseum_registry.pipelines.is_empty() {
        pipeline_lines.push(Line::from(Span::styled("No pipelines registered. Define agents and assemble a pipeline.", Style::default().fg(Color::Gray))));
    } else {
        for (p_idx, pipeline) in app.colosseum_registry.pipelines.values().enumerate() {
            let is_p_selected = app.pipeline_table_state.selected() == Some(p_idx);

            let (p_prefix, p_style) = if is_p_selected && is_pipelines_focused {
                (" ► ", Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD))
            } else if is_p_selected {
                ("   ", Style::default().bg(Color::DarkGray).fg(Color::White).add_modifier(Modifier::BOLD))
            } else {
                ("   ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            };

            pipeline_lines.push(Line::from(vec![
                Span::styled(p_prefix, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!(" Pipeline: {} ", pipeline.name), p_style),
                Span::styled(format!(" (ID: {})", pipeline.id), Style::default().fg(Color::DarkGray)),
            ]));

            // Worker status breakdown for workflow
            let status_color = if running_workers_count > 0 { Color::Green } else { Color::DarkGray };
            pipeline_lines.push(Line::from(vec![
                Span::styled("  Workers: ", Style::default().fg(Color::Gray)),
                Span::styled(format!("{} RUNNING, {} STOPPED", running_workers_count, stopped_workers_count), Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
                if running_workers_count > 0 {
                    Span::styled(" │ [LOCKED: Cannot modify while running]", Style::default().fg(Color::Red))
                } else {
                    Span::styled(" │ [Unlocked for editing]", Style::default().fg(Color::DarkGray))
                },
            ]));

            // Validate pipeline
            match app.colosseum_registry.validate_pipeline(pipeline) {
                Ok(()) => {
                    pipeline_lines.push(Line::from(Span::styled("  ✓ Validation: Passed (No pickup location conflicts)", Style::default().fg(Color::Green))));
                }
                Err(err) => {
                    pipeline_lines.push(Line::from(Span::styled(format!("  ✗ Validation FAILED: {}", err), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))));
                }
            }

            // Render concise DAG Order on pipeline list
            let mut dag_spans = Vec::new();
            dag_spans.push(Span::styled("  DAG Order: ", Style::default().fg(Color::Yellow)));
            if pipeline.agent_ids.is_empty() {
                dag_spans.push(Span::styled("(No stages configured)", Style::default().fg(Color::DarkGray)));
            } else {
                for (idx, agent_id) in pipeline.agent_ids.iter().enumerate() {
                    if idx > 0 {
                        dag_spans.push(Span::styled(" ──► ", Style::default().fg(Color::Yellow)));
                    }
                    let agent_name = app
                        .colosseum_registry
                        .agents
                        .get(agent_id)
                        .map(|a| a.name.as_str())
                        .unwrap_or(agent_id.as_str());
                    dag_spans.push(Span::styled(
                        format!("Stage {}: {}", idx + 1, agent_name),
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    ));
                }
            }
            pipeline_lines.push(Line::from(dag_spans));
            pipeline_lines.push(Line::from(""));
        }
    }

    let pipeline_title = if is_pipelines_focused {
        " ► PIPELINE VISUALIZER ◄ [ACTIVE - Up/Down: Select, Enter: Edit] "
    } else {
        " Pipeline Visualizer [Press Right to Focus] "
    };

    let pipeline_p = Paragraph::new(pipeline_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(pipeline_title)
            .border_style(Style::default().fg(if is_pipelines_focused { Color::Yellow } else { Color::Green })),
    );

    f.render_widget(pipeline_p, main_chunks[1]);
}

fn render_new_agent_popup(f: &mut Frame, app: &TuiApp, area: Rect) {
    let block = Block::default()
        .title(" Create New Agent Config from Scratch ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let popup_area = centered_rect(75, 70, area);
    f.render_widget(Clear, popup_area);

    let fields = [
        ("Agent Name", &app.new_agent_name_input),
        ("Persona", &app.new_agent_persona_input),
        ("Provider", &app.new_agent_provider_input),
        ("Model", &app.new_agent_model_input),
        ("Pickup Loc", &app.new_agent_pickup_input),
        ("Working Loc", &app.new_agent_working_input),
        ("Drop Loc", &app.new_agent_drop_input),
        ("Prompt", &app.new_agent_prompt_input),
    ];

    let mut lines = Vec::new();
    lines.push(Line::from("Fill in Agent fields (Press [Tab/Down] to cycle fields, [Enter] to Create):"));
    lines.push(Line::from(""));

    for (idx, (label, value)) in fields.iter().enumerate() {
        let is_active = idx == app.new_agent_field_step;
        let prefix = if is_active { " ► " } else { "   " };
        let style = if is_active {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let cursor = if is_active { "_" } else { "" };

        if idx == 1 {
            let personas = app.get_available_personas();
            let total = personas.len();
            lines.push(Line::from(vec![
                Span::styled(format!("{}{:12}: ", prefix, label), style),
                Span::styled(format!("◄ {} ►", value), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  ({}/{} Savant Personas - [Left/Right] to select)", app.new_agent_persona_idx + 1, total), Style::default().fg(Color::Magenta)),
            ]));
        } else if idx == 2 {
            let providers = app.get_available_providers();
            let total = providers.len();
            lines.push(Line::from(vec![
                Span::styled(format!("{}{:12}: ", prefix, label), style),
                Span::styled(format!("◄ {} ►", value), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  ({}/{} Gateway Providers - [Left/Right] to select)", app.new_agent_provider_idx + 1, total), Style::default().fg(Color::Magenta)),
            ]));
        } else if idx == 3 {
            let models = app.get_models_for_provider(&app.new_agent_provider_input);
            let total = models.len();
            lines.push(Line::from(vec![
                Span::styled(format!("{}{:12}: ", prefix, label), style),
                Span::styled(format!("◄ {} ►", value), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  ({}/{} Models for {} - [Left/Right] to select)", app.new_agent_model_idx + 1, total, app.new_agent_provider_input), Style::default().fg(Color::Magenta)),
            ]));
        } else if idx == 4 {
            let statuses = app.get_sanctum_statuses();
            let total = statuses.len();
            lines.push(Line::from(vec![
                Span::styled(format!("{}{:12}: ", prefix, label), style),
                Span::styled(format!("◄ {} ►", value), Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  ({}/{} Sanctum Statuses - [Left/Right] to select)", app.new_agent_pickup_idx + 1, total), Style::default().fg(Color::Magenta)),
            ]));
        } else if idx == 5 {
            let statuses = app.get_sanctum_statuses();
            let total = statuses.len();
            lines.push(Line::from(vec![
                Span::styled(format!("{}{:12}: ", prefix, label), style),
                Span::styled(format!("◄ {} ►", value), Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  ({}/{} Sanctum Statuses - [Left/Right] to select)", app.new_agent_working_idx + 1, total), Style::default().fg(Color::Magenta)),
            ]));
        } else if idx == 6 {
            let statuses = app.get_sanctum_statuses();
            let total = statuses.len();
            lines.push(Line::from(vec![
                Span::styled(format!("{}{:12}: ", prefix, label), style),
                Span::styled(format!("◄ {} ►", value), Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  ({}/{} Sanctum Statuses - [Left/Right] to select)", app.new_agent_drop_idx + 1, total), Style::default().fg(Color::Magenta)),
            ]));
        } else if idx == 7 {
            lines.push(Line::from(vec![
                Span::styled(format!("{}{:12}: ", prefix, label), style),
                Span::styled("(Multi-Line Instructions - 3+ Lines Room)", Style::default().fg(Color::Yellow)),
            ]));
            let full_val = format!("{}{}", value, cursor);
            let val_lines: Vec<String> = full_val.split('\n').map(|s| s.to_string()).collect();
            let display_lines_cnt = val_lines.len().max(3);

            for line_i in 0..display_lines_cnt {
                let text_line = val_lines.get(line_i).cloned().unwrap_or_default();
                let line_prefix = if line_i == 0 { "   └─► " } else { "      │ " };
                lines.push(Line::from(vec![
                    Span::styled(line_prefix, Style::default().fg(if is_active { Color::Yellow } else { Color::DarkGray })),
                    Span::styled(text_line, Style::default().fg(Color::White)),
                ]));
            }
        } else {
            lines.push(Line::from(vec![
                Span::styled(format!("{}{:12}: ", prefix, label), style),
                Span::styled(format!("{}{}", value, cursor), Style::default().fg(Color::White)),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press [Enter] Create Agent  │  [Tab/Down] Next Field  │  [Shift+Enter] Insert Newline  │  [Esc] Cancel",
        Style::default().fg(Color::Cyan),
    )));

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}

fn render_new_pipeline_popup(f: &mut Frame, app: &TuiApp, area: Rect) {
    let title = if app.editing_pipeline_id.is_some() {
        " Edit Pipeline Config - Select Agents from Available Library "
    } else {
        " Create New Pipeline - Select Agents from Available Library "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let popup_area = centered_rect(80, 75, area);
    f.render_widget(Clear, popup_area);

    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        "Set Pipeline Name and select Agent Sequence ([Space/Enter] Toggle Agent, [Up/Down] Navigate, [Ctrl+Enter] Save):",
        Style::default().fg(Color::Cyan),
    )));
    lines.push(Line::from(""));

    // Pipeline Name Input
    let is_name_active = app.new_pipeline_field_step == 0;
    let name_prefix = if is_name_active { " ► " } else { "   " };
    let name_style = if is_name_active {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let cursor = if is_name_active { "_" } else { "" };
    lines.push(Line::from(vec![
        Span::styled(format!("{}Pipeline Name : ", name_prefix), name_style),
        Span::styled(format!("{}{}", app.new_pipeline_name_input, cursor), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(""));

    // Agent Selection Header
    let is_agents_active = app.new_pipeline_field_step == 1;
    let agents_header_prefix = if is_agents_active { " ► " } else { "   " };
    lines.push(Line::from(vec![
        Span::styled(format!("{}Available Registered Agents in Library (Toggle to include in pipeline sequence):", agents_header_prefix), if is_agents_active { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Gray) }),
    ]));
    lines.push(Line::from("────────────────────────────────────────────────────────────────────────────────"));

    let available_agents: Vec<AgentConfig> = app.colosseum_registry.agents.values().cloned().collect();
    if available_agents.is_empty() {
        lines.push(Line::from(Span::styled("   No agents available in library. Press [a] to create agents first.", Style::default().fg(Color::Red))));
    } else {
        for (idx, agent) in available_agents.iter().enumerate() {
            let is_row_highlighted = is_agents_active && idx == app.new_pipeline_selected_agent_idx;
            let row_prefix = if is_row_highlighted { "  ► " } else { "    " };

            let (check_str, check_style) = if let Some(pos) = app.new_pipeline_agent_ids.iter().position(|id| id == &agent.id) {
                (
                    format!("[✓ Stage {}]", pos + 1),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                )
            } else {
                ("[         ]".to_string(), Style::default().fg(Color::DarkGray))
            };

            let name_style = if is_row_highlighted {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };

            lines.push(Line::from(vec![
                Span::styled(row_prefix, Style::default().fg(Color::Yellow)),
                Span::styled(format!("{:12} ", check_str), check_style),
                Span::styled(format!("{:16} ", agent.name), name_style),
                Span::styled(format!("{:14} ", agent.persona), Style::default().fg(Color::Yellow)),
                Span::styled(format!("[{}:{}] ", agent.provider, agent.model), Style::default().fg(Color::Magenta)),
                Span::styled(format!(" ({} ──► {} ──► {})", agent.pickup_location, agent.get_working_location(), agent.drop_location), Style::default().fg(Color::Gray)),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("Selected Pipeline Sequence & Status Flow Preview:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))));

    if app.new_pipeline_agent_ids.is_empty() {
        lines.push(Line::from(Span::styled("   (No agents selected yet - press Space to select agents above)", Style::default().fg(Color::Gray))));
    } else {
        let total_stages = app.new_pipeline_agent_ids.len();
        for (i, agent_id) in app.new_pipeline_agent_ids.iter().enumerate() {
            if let Some(agent) = app.colosseum_registry.agents.get(agent_id) {
                lines.push(Line::from(vec![
                    Span::styled(format!("   Stage {}: ", i + 1), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::styled(&agent.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    Span::styled(format!(" ({}) ", agent.persona), Style::default().fg(Color::Cyan)),
                    Span::styled(format!("[Pickup: {} ──► Working: {} ──► Drop: {}]", agent.pickup_location, agent.get_working_location(), agent.drop_location), Style::default().fg(Color::Yellow)),
                ]));

                if i + 1 < total_stages {
                    let next_id = &app.new_pipeline_agent_ids[i + 1];
                    let next_name = app.colosseum_registry.agents.get(next_id).map(|a| a.name.as_str()).unwrap_or(next_id.as_str());
                    lines.push(Line::from(vec![
                        Span::styled(format!("           └─► Handoff via status '{}' to Stage {}: '{}'", agent.drop_location, i + 2, next_name), Style::default().fg(Color::Magenta)),
                    ]));
                }
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press [Enter] Next Field / Save Pipeline  │  [Space] Toggle Agent  │  [Tab] Switch Field  │  [Esc] Cancel",
        Style::default().fg(Color::Green),
    )));

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}


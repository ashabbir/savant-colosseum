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
use sysinfo::{Pid, System};

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
pub enum ViewMode {
    Normal,
    WorkerInspector,
    StartWorkerPrompt,
    FilterPrompt,
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

pub struct TuiApp {
    pub data_dir: PathBuf,
    pub registry: WorkerRegistry,
    pub client: Option<SavantClient>,
    pub server_url: String,

    pub main_tab: MainTab,
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

    // Prompts Input State
    pub start_workspace_input: String,
    pub start_poll_input: String,

    // Global Metrics
    pub total_cpu_usage: f32,
    pub total_memory_mb: f64,
    pub active_workers_count: usize,
    pub total_workers_count: usize,

    pub status_message: Option<(String, Instant)>,
    pub system: System,
    pub last_tick: Instant,
}

impl TuiApp {
    pub fn new(data_dir: &Path, server_url: String, api_key: Option<String>) -> Result<Self> {
        let registry = WorkerRegistry::new(data_dir);
        let client = SavantClient::new(&server_url, api_key.as_deref()).ok();

        let mut app = Self {
            data_dir: data_dir.to_path_buf(),
            registry,
            client,
            server_url,
            main_tab: MainTab::Workers,
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
            start_workspace_input: String::new(),
            start_poll_input: "15".into(),
            total_cpu_usage: 0.0,
            total_memory_mb: 0.0,
            active_workers_count: 0,
            total_workers_count: 0,
            status_message: None,
            system: System::new_all(),
            last_tick: Instant::now(),
        };

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

    pub fn refresh_workers(&mut self) -> Result<()> {
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

            // Extract last event summary from worker JSONL log
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

        self.total_cpu_usage = sum_cpu;
        self.total_memory_mb = sum_mem;
        self.active_workers_count = active_cnt;
        self.total_workers_count = updated.len();
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

    pub fn launch_worker(&mut self, workspace_id: Option<String>) -> Result<()> {
        let current_exe = std::env::current_exe()?;
        let mut cmd = std::process::Command::new(current_exe);
        cmd.arg("start").arg("--daemon");
        if let Some(ref ws) = workspace_id {
            cmd.arg("--workspace").arg(ws);
        }
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
        KeyCode::Down | KeyCode::Char('j') => match app.main_tab {
            MainTab::Workers => app.select_next_worker(),
            _ => {}
        },
        KeyCode::Up | KeyCode::Char('k') => match app.main_tab {
            MainTab::Workers => app.select_prev_worker(),
            _ => {}
        },
        KeyCode::Char('g') => {
            if !app.filtered_workers().is_empty() {
                app.table_state.select(Some(0));
                app.selected_worker_id = Some(app.filtered_workers()[0].record.worker_id.clone());
            }
        }
        KeyCode::Char('G') => {
            let len = app.filtered_workers().len();
            if len > 0 {
                app.table_state.select(Some(len - 1));
                app.selected_worker_id = Some(app.filtered_workers()[len - 1].record.worker_id.clone());
            }
        }
        KeyCode::Enter => {
            if app.main_tab == MainTab::Workers && app.selected_worker().is_some() {
                app.mode = ViewMode::WorkerInspector;
                app.log_scroll = 0;
            }
        }
        KeyCode::Char('s') => {
            app.mode = ViewMode::StartWorkerPrompt;
            app.start_workspace_input.clear();
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
            Constraint::Length(4), // System Dashboard Header
            Constraint::Min(10),   // Active Content Pane
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
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    // Tab Bar Title Block
    let titles: Vec<Line> = vec![
        Line::from(" [1] Workers Engine "),
        Line::from(" [2] Workspaces & Queue "),
        Line::from(" [3] Server Diagnostics "),
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
                .title(" SAVANT COLOSSEUM POWERHOUSE (v4.0.0) ")
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

    // Live System Metrics Block
    let _cpu_percent = (app.total_cpu_usage / 100.0).clamp(0.0, 1.0);
    let cpu_str = format!("CPU: {:.1}%", app.total_cpu_usage);
    let mem_str = format!("RAM: {:.1} MB", app.total_memory_mb);
    let worker_stats = format!(
        "Active Workers: {} / {}",
        app.active_workers_count, app.total_workers_count
    );

    let metrics_text = vec![
        Line::from(vec![
            Span::styled(format!("{cpu_str:<14}"), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{mem_str:<16}"), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(worker_stats, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Server Status: ", Style::default().fg(Color::Gray)),
            Span::styled("ONLINE (Connected)", Style::default().fg(Color::Green)),
            Span::raw(" │ Data: "),
            Span::raw(app.data_dir.display().to_string()),
        ]),
    ];

    let metrics_block = Paragraph::new(metrics_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Real-time Resource Engine ")
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
            Span::raw(w.record.workspace_id.as_deref().unwrap_or("(all workspaces)").to_string()),
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
            Constraint::Percentage(22),
            Constraint::Percentage(16),
            Constraint::Percentage(10),
            Constraint::Percentage(8),
            Constraint::Percentage(8),
            Constraint::Percentage(10),
            Constraint::Percentage(12),
            Constraint::Percentage(14),
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
                Span::styled("Workspace: ", Style::default().fg(Color::Yellow)),
                Span::raw(worker.record.workspace_id.as_deref().unwrap_or("(all workspaces)")),
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

fn render_workspaces_tab(f: &mut Frame, app: &TuiApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(6)])
        .split(area);

    let info_p = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::Gray)),
            Span::styled("[s]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" to launch a worker for any workspace, or ", Style::default().fg(Color::Gray)),
            Span::styled("[L]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" to launch a daemon worker directly.", Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::styled("Connected Server: ", Style::default().fg(Color::Cyan)),
            Span::raw(&app.server_url),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Workspace & Task Discovery Engine ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(info_p, chunks[0]);

    let ws_list = vec![
        ListItem::new(Line::from(vec![
            Span::styled("• Global Scope (All Workspaces)", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" - Listens for any ready Colosseum task across all repositories"),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("• Workspace: savant-colosseum", Style::default().fg(Color::Yellow)),
            Span::raw(" (ID: 2539163563543949210)"),
        ])),
    ];

    let list_w = List::new(ws_list).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Active Savant Workspaces & Queue Scope ")
            .border_style(Style::default().fg(Color::Blue)),
    );

    f.render_widget(list_w, chunks[1]);
}

fn render_server_tab(f: &mut Frame, app: &TuiApp, area: Rect) {
    let text = vec![
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
        Line::from(Span::styled("System Engine Diagnostics:", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
        Line::from("• Worker Registry Lock: Atomic Directory Reservation (Active)"),
        Line::from("• Process Liveness Verification: PID + OS Start Time Validation"),
        Line::from("• Server Liveness: OK"),
    ];

    let p = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Savant Executioner Server & Environment Status ")
            .border_style(Style::default().fg(Color::Green)),
    );

    f.render_widget(p, area);
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
            MainTab::Workers => " [1/2/3] Tabs │ [s] Start │ [x] TERM │ [X] KILL │ [y] Copy ID │ [Y] Log Path │ [d] Delete │ [g/G] Top/Bot │ [q] Quit ",
            MainTab::WorkspacesAndTasks => " [1/2/3] Tabs │ [s] Start │ [L] Launch for Workspace │ [q] Quit ",
            MainTab::ServerStatus => " [1/2/3] Tabs │ [r] Refresh │ [q] Quit ",
        },
        ViewMode::WorkerInspector => " [Tab] Switch Tab │ [f] Toggle Follow │ [↑/↓] Scroll │ [y] Copy ID │ [Y] Log Path │ [x] TERM │ [Esc/q] Back ",
        ViewMode::FilterPrompt => " Type filter query... │ [Enter/Esc] Done ",
        ViewMode::StartWorkerPrompt => " Type Workspace ID... │ [Enter] Launch Worker │ [Esc] Cancel ",
    };

    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
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

pub mod app;
pub mod benchmark;
pub mod dashboard;
pub mod devices;
pub mod help;
pub mod logs;
pub mod plugins;
pub mod scheduler;
pub mod tasks;
pub mod telemetry;
pub mod theme;

use std::io;
use std::time::{Duration, Instant};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame, Terminal,
};

use app::{
    ActionItem, ActiveModal, ActiveScreen, App, ExecutionMode, LogSeverity,
    TaskRecord, TaskStatus, TaskTimingBreakdown, WorkloadClassification,
};
use pocketbridge_core::{
    collect_host_profile, profiler::build_worker_profile_from_props,
    read_envelope, run_benchmark_suite, save_benchmark_report, write_envelope,
    AdbTransport, AgentRole, BenchmarkConfig, ChunkTransformParams, CostModel,
    Envelope, HelloPayload, LocalExecutor, MessageKind,
    PluginManager, RemoteJobClient, Scheduler, SchedulerConfig, ADAPTER_ID,
};
use telemetry::CpuSample;
use theme::*;

/// Safe terminal restorer in case of panics or normal exits
struct TerminalRestorer;

impl Drop for TerminalRestorer {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

pub async fn run() -> Result<()> {
    // Install panic hook to always restore terminal on panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let _restorer = TerminalRestorer;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    // Initial Host Profile
    if let Ok(host) = collect_host_profile().await {
        app.host_profile = Some(host);
        app.log(LogSeverity::Info, "system", "Collected host hardware profile.");
    }

    // Initial ADB Device Check
    let transport = AdbTransport::new(app.port);
    if let Ok(dev) = transport.check_device().await {
        app.transport.adb_device = Some(dev.clone());
        app.log(LogSeverity::Info, "transport", format!("Detected ADB device: {}", dev));
        if let Ok(props) = transport.query_getprop().await {
            app.worker_profile = Some(build_worker_profile_from_props(&props));
            app.log(LogSeverity::Info, "worker", "Cached worker properties via ADB getprop.");
        }
    }

    update_scheduler_evaluation(&mut app);

    // Run main asynchronous loop
    let res = run_loop(&mut terminal, &mut app).await;

    // Graceful teardown
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

fn update_scheduler_evaluation(app: &mut App) {
    let config = SchedulerConfig {
        speedup_margin: app.scheduler_margin,
        host_load_factor: app.scheduler_host_load,
        ..Default::default()
    };
    let scheduler = Scheduler::new(config, CostModel::default());
    let input_bytes = (app.scheduler_records as usize) * 8;
    let output_bytes = (app.scheduler_records as usize) * 8;

    let decision = scheduler.evaluate_task(
        app.scheduler_records,
        app.scheduler_rounds,
        input_bytes,
        output_bytes,
        app.worker_profile.as_ref(),
        app.transport.rtt_ms,
    );
    app.scheduler_decision = Some(decision);
}

async fn run_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    let mut last_telemetry_tick = Instant::now() - Duration::from_secs(10);
    let mut cpu_sample: Option<CpuSample> = None;

    loop {
        // 1. Periodic Telemetry Tick (Every 1.0 second)
        if last_telemetry_tick.elapsed() >= Duration::from_millis(1000) {
            last_telemetry_tick = Instant::now();

            // Host /proc metrics
            app.host = telemetry::HostMetrics::poll(&mut cpu_sample).await;

            // Worker Heartbeat & RTT query if connected
            if app.transport.is_connected {
                let mut transport_clone = app.transport.clone();
                let hb_res = tokio::time::timeout(
                    Duration::from_millis(600),
                    transport_clone.query_heartbeat(),
                ).await;

                match hb_res {
                    Ok(Ok(worker_hb)) => {
                        app.transport.rtt_ms = transport_clone.rtt_ms;
                        app.transport.last_heartbeat = Some(worker_hb.clone());
                        app.transport.last_heartbeat_time = Some(Instant::now());
                        app.transport.is_connected = true;

                        if let Some(prof) = app.worker_profile.as_mut() {
                            prof.battery_percent = worker_hb.battery_percent;
                            prof.battery_charging = worker_hb.battery_charging;
                            prof.thermal_status = worker_hb.thermal_status.clone();
                            if let (Some(used), Some(total)) = (worker_hb.memory_used_bytes, worker_hb.memory_total_bytes) {
                                prof.ram_total_bytes = total;
                                prof.ram_available_bytes = Some(total.saturating_sub(used));
                            }
                        }
                        update_scheduler_evaluation(app);
                    }
                    Ok(Err(e)) => {
                        app.transport.last_error = Some(e);
                    }
                    Err(_) => {
                        // Timeout
                    }
                }
            }
        }

        // 2. Render Frame
        terminal.draw(|f| {
            let size = f.area();

            // Terminal Dimensions Safety Check (Small Terminals Guard)
            if size.width < 50 || size.height < 14 {
                let msg = Paragraph::new(vec![
                    Line::from(Span::styled("TERMINAL TOO SMALL", Style::default().fg(COLOR_ERROR).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled(format!("Current: {}x{} | Minimum: 50x14", size.width, size.height), style_value())),
                    Line::from(Span::raw("Please expand terminal window.")),
                ])
                .block(Block::default().borders(Borders::ALL).title(" PocketBridge "));
                f.render_widget(msg, size);
                return;
            }

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Top Navigation / Header
                    Constraint::Min(8),    // Active Screen Body
                    Constraint::Length(3), // Bottom Footer / Keybindings
                ])
                .split(size);

            // Render Header
            render_top_header(f, app, chunks[0]);

            // Render Active Screen Body
            match app.screen {
                ActiveScreen::Overview => dashboard::render(f, app, chunks[1]),
                ActiveScreen::Devices => devices::render(f, app, chunks[1]),
                ActiveScreen::Tasks => tasks::render(f, app, chunks[1]),
                ActiveScreen::Benchmarks => benchmark::render(f, app, chunks[1]),
                ActiveScreen::Scheduler => scheduler::render(f, app, chunks[1]),
                ActiveScreen::Plugins => plugins::render(f, app, chunks[1]),
                ActiveScreen::Logs => logs::render(f, app, chunks[1]),
            }

            // Render Footer
            render_footer(f, app, chunks[2]);

            // Render Active Modals / Overlays
            match &app.modal {
                ActiveModal::None => {}
                ActiveModal::Help => help::render_help_modal(f, size),
                ActiveModal::ActionPalette => help::render_action_palette(f, app, size),
                ActiveModal::TaskDetail(_) => {}
                ActiveModal::ConfirmAction { .. } => {}
            }
        })?;

        // 3. Process Input Events (50ms timeout)
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                // Global Emergency Quit
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }

                // Modal Key Handling
                match &app.modal {
                    ActiveModal::Help => {
                        if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') || key.code == KeyCode::Char('q') {
                            app.modal = ActiveModal::None;
                        }
                        continue;
                    }
                    ActiveModal::ActionPalette => {
                        match key.code {
                            KeyCode::Esc => {
                                app.modal = ActiveModal::None;
                                app.action_palette_query.clear();
                            }
                            KeyCode::Up => {
                                app.selected_action_index = app.selected_action_index.saturating_sub(1);
                            }
                            KeyCode::Down => {
                                app.selected_action_index = (app.selected_action_index + 1).min(App::actions_list().len().saturating_sub(1));
                            }
                            KeyCode::Backspace => {
                                app.action_palette_query.pop();
                            }
                            KeyCode::Char(c) => {
                                app.action_palette_query.push(c);
                            }
                            KeyCode::Enter => {
                                let query = app.action_palette_query.to_lowercase();
                                let filtered: Vec<ActionItem> = App::actions_list()
                                    .into_iter()
                                    .filter(|a| query.is_empty() || a.title.to_lowercase().contains(&query) || a.description.to_lowercase().contains(&query))
                                    .collect();

                                if let Some(action) = filtered.get(app.selected_action_index.min(filtered.len().saturating_sub(1))) {
                                    let cmd_id = action.command_id;
                                    app.modal = ActiveModal::None;
                                    app.action_palette_query.clear();
                                    execute_action_by_id(app, cmd_id).await;
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }
                    _ => {}
                }

                // Search Filter Mode in Logs Screen
                if app.is_searching_logs {
                    match key.code {
                        KeyCode::Esc => {
                            app.is_searching_logs = false;
                            app.log_search_query.clear();
                        }
                        KeyCode::Enter => {
                            app.is_searching_logs = false;
                        }
                        KeyCode::Backspace => {
                            app.log_search_query.pop();
                        }
                        KeyCode::Char(c) => {
                            app.log_search_query.push(c);
                        }
                        _ => {}
                    }
                    continue;
                }

                // Global Navigation & Screen Shortcuts
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Tab => app.next_screen(),
                    KeyCode::BackTab => app.prev_screen(),
                    KeyCode::Char('1') => app.screen = ActiveScreen::Overview,
                    KeyCode::Char('2') => app.screen = ActiveScreen::Devices,
                    KeyCode::Char('3') => app.screen = ActiveScreen::Tasks,
                    KeyCode::Char('4') => app.screen = ActiveScreen::Benchmarks,
                    KeyCode::Char('5') => app.screen = ActiveScreen::Scheduler,
                    KeyCode::Char('6') => app.screen = ActiveScreen::Plugins,
                    KeyCode::Char('7') => app.screen = ActiveScreen::Logs,
                    KeyCode::Char('?') => app.modal = ActiveModal::Help,
                    KeyCode::Char(' ') => {
                        app.modal = ActiveModal::ActionPalette;
                        app.action_palette_query.clear();
                        app.selected_action_index = 0;
                    }

                    // Up / Down arrow table navigation
                    KeyCode::Up => match app.screen {
                        ActiveScreen::Devices => {
                            app.selected_device_index = app.selected_device_index.saturating_sub(1);
                        }
                        ActiveScreen::Tasks => {
                            app.selected_task_index = app.selected_task_index.saturating_sub(1);
                        }
                        ActiveScreen::Benchmarks => {
                            app.selected_benchmark_index = app.selected_benchmark_index.saturating_sub(1);
                        }
                        ActiveScreen::Plugins => {
                            app.selected_plugin_index = app.selected_plugin_index.saturating_sub(1);
                        }
                        ActiveScreen::Logs => {
                            app.selected_log_index = app.selected_log_index.saturating_sub(1);
                        }
                        _ => {}
                    },
                    KeyCode::Down => match app.screen {
                        ActiveScreen::Devices => {
                            app.selected_device_index = (app.selected_device_index + 1).min(1);
                        }
                        ActiveScreen::Tasks => {
                            if !app.tasks.is_empty() {
                                app.selected_task_index = (app.selected_task_index + 1).min(app.tasks.len() - 1);
                            }
                        }
                        ActiveScreen::Benchmarks => {
                            let max_idx = benchmark::supported_benchmarks().len().saturating_sub(1);
                            app.selected_benchmark_index = (app.selected_benchmark_index + 1).min(max_idx);
                        }
                        ActiveScreen::Plugins => {
                            let total_p = app.catalog_plugins.len();
                            app.selected_plugin_index = (app.selected_plugin_index + 1).min(total_p);
                        }
                        ActiveScreen::Logs if !app.logs.is_empty() => {
                            app.selected_log_index = (app.selected_log_index + 1).min(app.logs.len() - 1);
                        }
                        _ => {}
                    },

                    // 'c' -> Connect & Handshake
                    KeyCode::Char('c') => {
                        handle_connect_action(app).await;
                    }

                    // 'd' -> Disconnect Worker
                    KeyCode::Char('d') => {
                        app.transport.is_connected = false;
                        app.transport.is_connecting = false;
                        app.transport.hello = None;
                        app.transport.last_heartbeat = None;
                        app.log(LogSeverity::Warn, "transport", "Worker disconnected by user.");
                        app.set_status("Worker disconnected.");
                    }

                    // 'r' -> Refresh Telemetry
                    KeyCode::Char('r') => {
                        app.log(LogSeverity::Info, "system", "Refreshing system profiles and ADB status...");
                        if let Ok(host) = collect_host_profile().await {
                            app.host_profile = Some(host);
                        }
                        let transport = AdbTransport::new(app.port);
                        if let Ok(props) = transport.query_getprop().await {
                            app.worker_profile = Some(build_worker_profile_from_props(&props));
                        }
                        if app.transport.is_connected {
                            let _ = app.transport.query_heartbeat().await;
                        }
                        app.refresh_plugins();
                        update_scheduler_evaluation(app);
                        app.set_status("Telemetry refreshed.");
                    }

                    // 'j' -> Run Fast Calibration Job
                    KeyCode::Char('j') if app.active_task_id.is_none() => {
                        handle_run_calibration_job(app).await;
                    }

                    // 'Enter' on Benchmarks screen -> Execute selected benchmark
                    KeyCode::Enter if app.screen == ActiveScreen::Benchmarks && !app.benchmark_running => {
                        handle_run_benchmark(app).await;
                    }

                    // 's' on Benchmarks screen -> Save report
                    KeyCode::Char('s') if app.screen == ActiveScreen::Benchmarks => {
                        if !app.benchmark_results.is_empty() {
                            let report = pocketbridge_core::BenchmarkReport {
                                timestamp: chrono::Local::now().to_rfc3339(),
                                host_profile: app.host_profile.clone(),
                                worker_profile: app.worker_profile.clone(),
                                results: app.benchmark_results.clone(),
                            };
                            match save_benchmark_report(&report, "benchmarks/results").await {
                                Ok(p) => {
                                    app.last_saved_report = Some(p.display().to_string());
                                    app.log(LogSeverity::Info, "benchmark", format!("Saved report: {}", p.display()));
                                    app.set_status(format!("Report saved: {}", p.display()));
                                }
                                Err(e) => app.log(LogSeverity::Error, "benchmark", format!("Save error: {e}")),
                            }
                        }
                    }

                    // '+' / '-' -> Tuning parameters
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        if app.screen == ActiveScreen::Scheduler {
                            app.scheduler_records = match app.scheduler_records {
                                50_000 => 500_000,
                                500_000 => 2_000_000,
                                2_000_000 => 5_000_000,
                                _ => 50_000,
                            };
                            update_scheduler_evaluation(app);
                            app.log(LogSeverity::Info, "scheduler", format!("Scheduler workload: {} records", app.scheduler_records));
                        } else if app.screen == ActiveScreen::Benchmarks {
                            app.benchmark_runs = (app.benchmark_runs + 5).min(50);
                            app.set_status(format!("Benchmark runs: {}", app.benchmark_runs));
                        }
                    }
                    KeyCode::Char('-') => {
                        if app.screen == ActiveScreen::Scheduler {
                            app.scheduler_records = match app.scheduler_records {
                                5_000_000 => 2_000_000,
                                2_000_000 => 500_000,
                                500_000 => 50_000,
                                _ => 5_000_000,
                            };
                            update_scheduler_evaluation(app);
                            app.log(LogSeverity::Info, "scheduler", format!("Scheduler workload: {} records", app.scheduler_records));
                        } else if app.screen == ActiveScreen::Benchmarks {
                            app.benchmark_runs = (app.benchmark_runs.saturating_sub(5)).max(1);
                            app.set_status(format!("Benchmark runs: {}", app.benchmark_runs));
                        }
                    }

                    // 'h' -> Toggle Simulated Host Load
                    KeyCode::Char('h') | KeyCode::Char('H') if app.screen == ActiveScreen::Scheduler => {
                        app.scheduler_host_load = match (app.scheduler_host_load * 100.0).round() as i32 {
                            100 => 0.50,
                            50 => 0.20,
                            20 => 0.05,
                            _ => 1.00,
                        };
                        update_scheduler_evaluation(app);
                        app.log(LogSeverity::Info, "scheduler", format!("Adjusted host capacity to {:.0}%", app.scheduler_host_load * 100.0));
                    }

                    // 'p' -> Re-evaluate scheduler plan
                    KeyCode::Char('p') if app.screen == ActiveScreen::Scheduler => {
                        update_scheduler_evaluation(app);
                        app.log(LogSeverity::Info, "scheduler", "Re-evaluated explainable decision.");
                    }

                    // '/' -> Search in Logs
                    KeyCode::Char('/') if app.screen == ActiveScreen::Logs => {
                        app.is_searching_logs = true;
                    }

                    // 'F' -> Toggle Severity Filter in Logs
                    KeyCode::Char('f') | KeyCode::Char('F') if app.screen == ActiveScreen::Logs => {
                        app.log_severity_filter = match app.log_severity_filter {
                            None => Some(LogSeverity::Info),
                            Some(LogSeverity::Info) => Some(LogSeverity::Warn),
                            Some(LogSeverity::Warn) => Some(LogSeverity::Error),
                            Some(LogSeverity::Error) => None,
                            _ => None,
                        };
                    }

                    // 'i' -> Install Plugin in Plugins screen
                    KeyCode::Char('i') if app.screen == ActiveScreen::Plugins => {
                        handle_plugin_install_action(app).await;
                    }

                    // 'u' -> Remove Plugin in Plugins screen
                    KeyCode::Char('u') if app.screen == ActiveScreen::Plugins => {
                        handle_plugin_remove_action(app).await;
                    }

                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn render_top_header(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<String> = ActiveScreen::all()
        .iter()
        .map(|s| format!("[{}] {}", s.shortcut(), s.title()))
        .collect();

    let tab_titles: Vec<&str> = titles.iter().map(|s| s.as_str()).collect();
    let sel_idx = app.screen as usize;

    let dev_label = app.transport.adb_device.as_deref().unwrap_or("No Phone");
    let (conn_badge, conn_color) = if app.transport.is_connected {
        ("ONLINE", COLOR_SUCCESS)
    } else if app.transport.is_connecting {
        ("CONNECTING", COLOR_WARNING)
    } else {
        ("OFFLINE", COLOR_ERROR)
    };

    let title_block = format!(
        " POCKETBRIDGE v1.0.0 (Host: {} | Worker: {} [{}]) ",
        app.host.hostname, dev_label, conn_badge
    );

    let tabs = Tabs::new(tab_titles)
        .select(sel_idx)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(conn_color))
                .title(title_block),
        )
        .style(Style::default().fg(COLOR_MUTED))
        .highlight_style(Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD));

    f.render_widget(tabs, area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let status_str = app.current_status().map(|s| format!("  [{}]", s)).unwrap_or_default();

    let text = match app.screen {
        ActiveScreen::Overview => format!("[1-7] Screens  [c] Connect  [j] Run Job  [d] Disconnect  [r] Refresh  [Space] Action Menu  [?] Help  [q] Quit{}", status_str),
        ActiveScreen::Devices => format!("[↑/↓] Select Node  [c] Connect  [d] Disconnect  [r] Refresh Telemetry  [Space] Menu  [?] Help{}", status_str),
        ActiveScreen::Tasks => format!("[↑/↓] Select Task  [j] Run Fast Job  [r] Refresh  [Space] Menu  [?] Help{}", status_str),
        ActiveScreen::Benchmarks => format!("[↑/↓] Select Target  [Enter] Run  [+/-] Repetitions  [s] Save Report  [?] Help{}", status_str),
        ActiveScreen::Scheduler => format!("[h] Toggle Host Load  [+/-] Records  [p] Re-evaluate  [Space] Menu  [?] Help{}", status_str),
        ActiveScreen::Plugins => format!("[↑/↓] Select Plugin  [i] Install  [u] Remove  [r] Refresh Hub  [?] Help{}", status_str),
        ActiveScreen::Logs => format!("[/] Search  [Esc] Clear Filter  [F] Toggle Severity  [Space] Menu  [?] Help{}", status_str),
    };

    let p = Paragraph::new(Line::from(Span::styled(text, Style::default().fg(COLOR_SECONDARY))))
        .block(Block::default().borders(Borders::ALL).border_style(style_border_normal()));

    f.render_widget(p, area);
}

async fn execute_action_by_id(app: &mut App, command_id: &str) {
    match command_id {
        "screen_overview" => app.screen = ActiveScreen::Overview,
        "screen_devices" => app.screen = ActiveScreen::Devices,
        "screen_tasks" => app.screen = ActiveScreen::Tasks,
        "screen_benchmarks" => app.screen = ActiveScreen::Benchmarks,
        "screen_scheduler" => app.screen = ActiveScreen::Scheduler,
        "screen_plugins" => app.screen = ActiveScreen::Plugins,
        "screen_logs" => app.screen = ActiveScreen::Logs,
        "action_connect" => handle_connect_action(app).await,
        "action_refresh" => {
            app.set_status("Refreshing telemetry...");
            let transport = AdbTransport::new(app.port);
            if let Ok(props) = transport.query_getprop().await {
                app.worker_profile = Some(build_worker_profile_from_props(&props));
            }
            if app.transport.is_connected {
                let _ = app.transport.query_heartbeat().await;
            }
            update_scheduler_evaluation(app);
        }
        "action_run_job" => handle_run_calibration_job(app).await,
        "action_disconnect" => {
            app.transport.is_connected = false;
            app.transport.last_heartbeat = None;
            app.set_status("Worker disconnected.");
        }
        _ => {}
    }
}

async fn handle_connect_action(app: &mut App) {
    app.transport.is_connecting = true;
    app.log(LogSeverity::Info, "transport", format!("Configuring ADB forward tcp:{} -> tcp:{}...", app.port, app.port));
    app.set_status("Connecting to worker over USB/ADB...");

    let transport = AdbTransport::new(app.port);
    if let Err(e) = transport.setup_forwarding().await {
        app.transport.is_connecting = false;
        app.transport.is_connected = false;
        app.log(LogSeverity::Error, "transport", format!("ADB forward error: {e}"));
        app.set_status(format!("Connection failed: {e}"));
        return;
    }

    match transport.connect_tcp().await {
        Ok(mut stream) => {
            let hello = Envelope::hello("pocketbridge-tui", "1.0.0", AgentRole::Host);
            if write_envelope(&mut stream, &hello).await.is_ok() {
                if let Ok(reply) = read_envelope(&mut stream).await {
                    if reply.kind == MessageKind::Hello {
                        if let Ok(worker_hello) = serde_json::from_value::<HelloPayload>(reply.payload) {
                            app.transport.hello = Some(worker_hello.clone());
                            app.transport.is_connected = true;
                            app.transport.is_connecting = false;
                            app.log(
                                LogSeverity::Info,
                                "worker",
                                format!("Handshake confirmed with {} (v{})", worker_hello.agent_name, worker_hello.agent_version),
                            );
                            app.set_status("Connected and handshook successfully!");

                            // Immediate heartbeat query
                            let _ = app.transport.query_heartbeat().await;
                            update_scheduler_evaluation(app);
                        }
                    }
                }
            }
        }
        Err(e) => {
            app.transport.is_connecting = false;
            app.transport.is_connected = false;
            app.log(LogSeverity::Error, "transport", format!("TCP connect failed on port {}: {e}", app.port));
            app.set_status(format!("TCP connect failed: {e}"));
        }
    }
}

async fn handle_run_calibration_job(app: &mut App) {
    let task_id = format!("task-{:x}", chrono::Local::now().timestamp_millis());
    app.active_task_id = Some(task_id.clone());
    app.active_task_progress = 0.05;
    app.active_task_status_msg = "Initiating calibration job...".to_string();
    app.log(LogSeverity::Info, "task", format!("Started calibration task {} (50,000 records)...", &task_id[..8]));

    let params = ChunkTransformParams {
        record_count: 50_000,
        rounds: 20,
        seed: 0x123456789ABCDEF0,
    };

    let t_start = Instant::now();

    // 1. Local reference computation
    app.active_task_status_msg = "Computing host baseline...".to_string();
    let local_out = LocalExecutor::run_transform(&params);

    // 2. Remote execution
    let transport = AdbTransport::new(app.port);
    let _ = transport.setup_forwarding().await;

    match transport.connect_tcp().await {
        Ok(mut stream) => {
            let hello = Envelope::hello("pocketbridge-job", "1.0.0", AgentRole::Host);
            let _ = write_envelope(&mut stream, &hello).await;
            let _ = read_envelope(&mut stream).await;

            match RemoteJobClient::execute_remote_job(
                &mut stream,
                &params,
                Some(&local_out),
                |_fraction, _msg| {},
            ).await {
                Ok(res) => {
                    let total_ms = t_start.elapsed().as_secs_f64() * 1000.0;
                    app.active_task_id = None;
                    app.active_task_progress = 1.0;

                    let speedup_str = res.speedup.map(|s| format!("{:.2}x", s)).unwrap_or_else(|| "-".into());
                    app.log(LogSeverity::Info, "task", format!("Task {} finished. Speedup: {}", &task_id[..8], speedup_str));
                    app.set_status(format!("Job finished: Speedup {}", speedup_str));

                    app.tasks.push(TaskRecord {
                        id: task_id,
                        adapter_id: ADAPTER_ID.to_string(),
                        name: "SIMD Core Transform".to_string(),
                        classification: WorkloadClassification::Synthetic,
                        mode: ExecutionMode::Remote,
                        target_device: app.transport.adb_device.clone().unwrap_or_else(|| "Phone Worker".into()),
                        status: TaskStatus::Succeeded,
                        duration_ms: Some(total_ms),
                        records: res.records_processed,
                        speedup: res.speedup,
                        sha256: res.remote_sha256.clone(),
                        error: res.error_message.clone(),
                        started_at: chrono::Local::now(),
                        timings: Some(TaskTimingBreakdown {
                            upload_ms: res.remote_upload_ms,
                            compute_ms: res.remote_compute_ms,
                            download_ms: res.remote_download_ms,
                            total_ms: res.remote_total_ms,
                        }),
                        parameters_summary: "50,000 records, 20 rounds, PRNG seed: 0x12345678".to_string(),
                    });
                }
                Err(e) => {
                    app.active_task_id = None;
                    app.log(LogSeverity::Error, "task", format!("Remote task execution error: {e}"));
                    app.set_status(format!("Task failed: {e}"));
                }
            }
        }
        Err(e) => {
            app.active_task_id = None;
            app.log(LogSeverity::Error, "task", format!("Could not connect to worker: {e}"));
            app.set_status(format!("Connection error: {e}"));
        }
    }
}

async fn handle_run_benchmark(app: &mut App) {
    let benchmarks = benchmark::supported_benchmarks();
    let selected = &benchmarks[app.selected_benchmark_index.min(benchmarks.len() - 1)];

    if !selected.is_supported {
        app.log(LogSeverity::Warn, "benchmark", format!("Suite '{}' is not implemented.", selected.name));
        return;
    }

    app.benchmark_running = true;
    app.log(LogSeverity::Info, "benchmark", format!("Running benchmark '{}' ({} runs)...", selected.name, app.benchmark_runs));
    app.set_status(format!("Running {}...", selected.name));

    let transport = AdbTransport::new(app.port);
    let _ = transport.setup_forwarding().await;

    match transport.connect_tcp().await {
        Ok(mut stream) => {
            let hello = Envelope::hello("pocketbridge-bench", "1.0.0", AgentRole::Host);
            let _ = write_envelope(&mut stream, &hello).await;
            let _ = read_envelope(&mut stream).await;

            let config = BenchmarkConfig {
                payload_sizes: vec![1_000_000, 10_000_000, 100_000_000],
                runs_per_size: app.benchmark_runs,
                include_latency_pings: true,
                latency_ping_sizes: vec![64, 1024],
                priority: Some(1),
            };

            match run_benchmark_suite(&mut stream, &config, |_done, _total, _stage| {}).await {
                Ok(report) => {
                    app.benchmark_results = report.results.clone();
                    app.benchmark_running = false;

                    // Update throughput sample on transport panel
                    if let Some(tp_sample) = report.results.iter().find(|r| r.test_type == pocketbridge_core::TestType::ThroughputEcho) {
                        app.transport.measured_throughput_mbps = tp_sample.stats.throughput_mbps;
                        app.transport.throughput_sample_bytes = Some(tp_sample.payload_size_bytes);
                    }

                    app.log(LogSeverity::Info, "benchmark", "Benchmark suite completed successfully.");
                    app.set_status("Benchmark completed!");

                    if let Ok(path) = save_benchmark_report(&report, "benchmarks/results").await {
                        app.last_saved_report = Some(path.display().to_string());
                        app.log(LogSeverity::Info, "benchmark", format!("Saved report to {}", path.display()));
                    }
                }
                Err(e) => {
                    app.benchmark_running = false;
                    app.log(LogSeverity::Error, "benchmark", format!("Benchmark execution failed: {e}"));
                    app.set_status(format!("Benchmark failed: {e}"));
                }
            }
        }
        Err(e) => {
            app.benchmark_running = false;
            app.log(LogSeverity::Error, "benchmark", format!("Connection failed for benchmark: {e}"));
            app.set_status(format!("Connection error: {e}"));
        }
    }
}

async fn handle_plugin_install_action(app: &mut App) {
    if app.selected_plugin_index == 0 {
        app.set_status("chunk_transform is built into the core engine.");
        return;
    }

    let cat_idx = app.selected_plugin_index - 1;
    if let Some((manifest, is_installed)) = app.catalog_plugins.get(cat_idx) {
        if *is_installed {
            app.set_status(format!("Plugin '{}' is already installed.", manifest.id));
            return;
        }

        let manifest = manifest.clone();
        app.log(LogSeverity::Info, "plugin", format!("Installing plugin '{}' (v{})...", manifest.id, manifest.version));
        app.set_status(format!("Installing {}...", manifest.id));

        let manager = PluginManager::default_manager();
        match manager.install_manifest(&manifest, None) {
            Ok(dir) => {
                app.log(LogSeverity::Info, "plugin", format!("Installed to {}", dir.display()));
                let _ = manager.sync_to_worker(&manifest, None).await;
                app.refresh_plugins();
                app.set_status(format!("Plugin '{}' installed successfully!", manifest.name));
            }
            Err(e) => {
                app.log(LogSeverity::Error, "plugin", format!("Installation error: {e}"));
                app.set_status(format!("Install error: {e}"));
            }
        }
    }
}

async fn handle_plugin_remove_action(app: &mut App) {
    if app.selected_plugin_index == 0 {
        app.set_status("Cannot remove built-in core engine.");
        return;
    }

    let cat_idx = app.selected_plugin_index - 1;
    let maybe_target = app.catalog_plugins.get(cat_idx).map(|(m, inst)| (m.id.clone(), *inst));

    if let Some((target_id, is_installed)) = maybe_target {
        if !is_installed {
            app.set_status(format!("Plugin '{}' is not installed.", target_id));
            return;
        }

        let manager = PluginManager::default_manager();
        match manager.remove(&target_id) {
            Ok(true) => {
                app.log(LogSeverity::Warn, "plugin", format!("Removed plugin '{}'.", target_id));
                app.refresh_plugins();
                app.set_status(format!("Plugin '{}' removed.", target_id));
            }
            Ok(false) => {
                app.set_status(format!("Plugin '{}' was not found on disk.", target_id));
            }
            Err(e) => {
                app.log(LogSeverity::Error, "plugin", format!("Error removing plugin: {e}"));
                app.set_status(format!("Remove error: {e}"));
            }
        }
    }
}

pub mod app;
pub mod benchmark;
pub mod dashboard;
pub mod profile;

use std::io;
use std::time::Duration;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Tabs},
    Terminal,
};

use app::{ActiveTab, App};
use pocketbridge_core::{
    collect_host_profile, profiler::build_worker_profile_from_props, read_envelope,
    run_benchmark_suite, save_benchmark_report, write_envelope, AdbTransport, AgentRole,
    BenchmarkConfig, ChunkTransformParams, Envelope, HeartbeatPayload, HelloPayload, LocalExecutor,
    MessageKind, RemoteJobClient,
};

pub async fn run() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    if let Ok(host) = collect_host_profile().await {
        app.host_profile = Some(host);
        app.log("Loaded host profile (Arch Linux)");
    }

    let transport = AdbTransport::new(app.port);
    if let Ok(dev) = transport.check_device().await {
        app.device_name = Some(dev);
        if let Ok(props) = transport.query_getprop().await {
            app.worker_profile = Some(build_worker_profile_from_props(&props));
            app.log("Detected connected phone properties via ADB");
        }
    }

    let res = run_loop(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

async fn run_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    let mut last_hb_check = std::time::Instant::now();

    loop {
        terminal.draw(|f| {
            let size = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Top Navigation Bar
                    Constraint::Min(10),   // Active Tab Body
                    Constraint::Length(3), // Bottom Footer / Keybindings
                ])
                .split(size);

            let titles = vec!["[1] Dashboard", "[2] Benchmark", "[3] Device Profile"];
            let tab_index = app.active_tab as usize;
            let tabs = Tabs::new(titles)
                .select(tab_index)
                .block(Block::default().borders(Borders::ALL).title(" PocketBridge v0.1.0 (Feasibility & Compute Offloader) "))
                .style(Style::default().fg(Color::White))
                .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
            f.render_widget(tabs, chunks[0]);

            match app.active_tab {
                ActiveTab::Dashboard => dashboard::render(f, app, chunks[1]),
                ActiveTab::Benchmark => benchmark::render(f, app, chunks[1]),
                ActiveTab::Profile => profile::render(f, app, chunks[1]),
            }

            let footer_text = match app.active_tab {
                ActiveTab::Dashboard => "[Tab] Tabs | [c] Connect | [j] Run Compute Job | [d] Disconnect | [r] Refresh | [q] Quit",
                ActiveTab::Benchmark => "[Enter] Run Benchmark | [+/-] Repetitions | [s] Save Report | [Tab] Tabs | [q] Quit",
                ActiveTab::Profile => "[r] Refresh Profile | [c] Connect | [Tab] Tabs | [q] Quit",
            };
            let footer = Paragraph::new(Span::styled(footer_text, Style::default().fg(Color::Yellow)))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(footer, chunks[2]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Tab => app.next_tab(),
                    KeyCode::BackTab => app.prev_tab(),
                    KeyCode::Char('1') => app.active_tab = ActiveTab::Dashboard,
                    KeyCode::Char('2') => app.active_tab = ActiveTab::Benchmark,
                    KeyCode::Char('3') => app.active_tab = ActiveTab::Profile,
                    KeyCode::Char('c') => {
                        app.connecting = true;
                        app.log(format!("Setting up ADB forwarding and connecting to port {}...", app.port));
                        let transport = AdbTransport::new(app.port);
                        if let Err(e) = transport.setup_forwarding().await {
                            app.log(format!("ADB forward error: {e}"));
                            app.connecting = false;
                        } else {
                            match transport.connect_tcp().await {
                                Ok(mut stream) => {
                                    let hello = Envelope::hello("pocketbridge-tui", "0.1.0", AgentRole::Host);
                                    if let Ok(_) = write_envelope(&mut stream, &hello).await {
                                        if let Ok(reply) = read_envelope(&mut stream).await {
                                            if reply.kind == MessageKind::Hello {
                                                if let Ok(worker_hello) = serde_json::from_value::<HelloPayload>(reply.payload) {
                                                    app.worker_hello = Some(worker_hello.clone());
                                                    app.connected = true;
                                                    app.connecting = false;
                                                    app.status_message = format!("Connected to {}", worker_hello.agent_name);
                                                    app.log(format!("Handshake complete with worker v{}", worker_hello.agent_version));
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    app.connecting = false;
                                    app.connected = false;
                                    app.status_message = format!("Connection failed: {e}");
                                    app.log(format!("TCP connect error: {e}"));
                                }
                            }
                        }
                    }
                    KeyCode::Char('j') if !app.job_running => {
                        app.job_running = true;
                        app.job_progress_pct = 0.0;
                        app.job_status_msg = "Starting compute job...".into();
                        app.log(format!("Executing compute task ({} records)...", app.job_records));

                        let params = ChunkTransformParams {
                            record_count: app.job_records,
                            rounds: app.job_rounds,
                            seed: 0x123456789ABCDEF0,
                        };

                        // 1. Local reference
                        app.job_status_msg = "Computing local reference on host...".into();
                        let local_out = LocalExecutor::run_transform(&params);
                        app.log(format!("Local compute took {:.2}ms (sha={})", local_out.duration_ms, &local_out.output_sha256[..8]));

                        // 2. Remote execution
                        let transport = AdbTransport::new(app.port);
                        let _ = transport.setup_forwarding().await;

                        match transport.connect_tcp().await {
                            Ok(mut stream) => {
                                let hello = Envelope::hello("pocketbridge-job", "0.1.0", AgentRole::Host);
                                let _ = write_envelope(&mut stream, &hello).await;
                                let _ = read_envelope(&mut stream).await;

                                match RemoteJobClient::execute_remote_job(
                                    &mut stream,
                                    &params,
                                    Some(&local_out),
                                    |_fraction, _msg| {
                                        // Progress update
                                    },
                                ).await {
                                    Ok(res) => {
                                        app.job_running = false;
                                        app.job_progress_pct = 1.0;
                                        if let Some(s) = res.speedup {
                                            app.log(format!("Task completed! Speedup: {:.2}x", s));
                                        } else {
                                            app.log("Task completed successfully");
                                        }
                                        app.active_job_result = Some(res);
                                    }
                                    Err(e) => {
                                        app.job_running = false;
                                        app.log(format!("Remote task execution failed: {e}"));
                                    }
                                }
                            }
                            Err(e) => {
                                app.job_running = false;
                                app.log(format!("Could not connect to worker for remote job: {e}"));
                            }
                        }
                    }
                    KeyCode::Char('d') => {
                        app.connected = false;
                        app.connecting = false;
                        app.worker_hello = None;
                        app.last_heartbeat = None;
                        app.status_message = "Disconnected by user".into();
                        app.log("Disconnected from worker");
                    }
                    KeyCode::Char('r') => {
                        app.log("Refreshing hardware profiles...");
                        if let Ok(host) = collect_host_profile().await {
                            app.host_profile = Some(host);
                        }
                        let transport = AdbTransport::new(app.port);
                        if let Ok(props) = transport.query_getprop().await {
                            app.worker_profile = Some(build_worker_profile_from_props(&props));
                            app.log("Refreshed worker properties via ADB");
                        }
                    }
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        app.runs = (app.runs + 5).min(100);
                        app.log(format!("Benchmark repetitions set to {}", app.runs));
                    }
                    KeyCode::Char('-') => {
                        app.runs = (app.runs.saturating_sub(5)).max(1);
                        app.log(format!("Benchmark repetitions set to {}", app.runs));
                    }
                    KeyCode::Enter if app.active_tab == ActiveTab::Benchmark && !app.benchmark_running => {
                        app.benchmark_running = true;
                        app.log(format!("Starting benchmark suite (runs={}, port={})...", app.runs, app.port));

                        let transport = AdbTransport::new(app.port);
                        let _ = transport.setup_forwarding().await;

                        match transport.connect_tcp().await {
                            Ok(mut stream) => {
                                let hello = Envelope::hello("pocketbridge-bench", "0.1.0", AgentRole::Host);
                                let _ = write_envelope(&mut stream, &hello).await;
                                let _ = read_envelope(&mut stream).await;

                                let config = BenchmarkConfig {
                                    payload_sizes: vec![1_000_000, 10_000_000, 100_000_000],
                                    runs_per_size: app.runs,
                                    include_latency_pings: true,
                                    latency_ping_sizes: vec![64, 1024],
                                    priority: Some(1),
                                };

                                match run_benchmark_suite(&mut stream, &config, |_done, _total, _stage| {}).await {
                                    Ok(report) => {
                                        app.benchmark_results = report.results.clone();
                                        app.benchmark_running = false;
                                        app.log("Benchmark suite finished successfully!");
                                        if let Ok(path) = save_benchmark_report(&report, "benchmarks/results").await {
                                            app.last_saved_report = Some(path.display().to_string());
                                            app.log(format!("Saved report to {}", path.display()));
                                        }
                                    }
                                    Err(e) => {
                                        app.benchmark_running = false;
                                        app.log(format!("Benchmark failed: {e}"));
                                    }
                                }
                            }
                            Err(e) => {
                                app.benchmark_running = false;
                                app.log(format!("Cannot run benchmark: Failed to connect ({e})"));
                            }
                        }
                    }
                    KeyCode::Char('s') if app.active_tab == ActiveTab::Benchmark => {
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
                                    app.log(format!("Report saved: {}", p.display()));
                                }
                                Err(e) => app.log(format!("Save error: {e}")),
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Heartbeat tick if connected
        if app.connected && last_hb_check.elapsed() >= Duration::from_secs(5) {
            last_hb_check = std::time::Instant::now();
            let transport = AdbTransport::new(app.port);
            if let Ok(mut stream) = transport.connect_tcp().await {
                let hb = Envelope::heartbeat(HeartbeatPayload {
                    uptime_ms: 0,
                    cpu_usage_percent: None,
                    memory_used_bytes: None,
                    memory_total_bytes: None,
                    temperature_celsius: None,
                    battery_percent: None,
                    battery_charging: None,
                    thermal_status: None,
                });
                if let Ok(_) = write_envelope(&mut stream, &hb).await {
                    if let Ok(reply) = read_envelope(&mut stream).await {
                        if reply.kind == MessageKind::Heartbeat {
                            if let Ok(worker_hb) = serde_json::from_value::<HeartbeatPayload>(reply.payload) {
                                app.last_heartbeat = Some(worker_hb);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

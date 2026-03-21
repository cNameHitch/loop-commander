use anyhow::Context;
use chrono::{DateTime, Local, Utc};
use clap::{Parser, Subcommand};
use intern_core::{
    prompt::AgentEntry, CreateTaskInput, DashboardMetrics, DryRunResult, ExecutionLog, InternPaths,
    JsonRpcRequest, JsonRpcResponse, Schedule, Task, TaskExport, TaskStatus,
};
use serde_json::json;
use std::collections::HashMap;
use std::io::Write;

// ── CLI Argument Definitions ─────────────────────────────

#[derive(Parser)]
#[command(name = "intern", about = "Intern CLI", version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all tasks
    List,
    /// Add a new task
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        command: String,
        #[arg(long)]
        schedule: String,
        #[arg(long, default_value = "~/")]
        working_dir: String,
        #[arg(long)]
        budget: Option<f64>,
        #[arg(long)]
        template: Option<String>,
        #[arg(long, num_args = 1..)]
        tags: Option<Vec<String>>,
    },
    /// Edit a task
    Edit {
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        command: Option<String>,
        #[arg(long)]
        schedule: Option<String>,
        #[arg(long)]
        working_dir: Option<String>,
        #[arg(long)]
        budget: Option<f64>,
    },
    /// Remove a task
    Rm {
        id: String,
        #[arg(short)]
        y: bool,
    },
    /// Pause a task
    Pause { id: String },
    /// Resume a task
    Resume { id: String },
    /// Run a task immediately
    Run {
        id: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Stop a running task
    Stop { id: String },
    /// View execution logs
    Logs {
        id: Option<String>,
        #[arg(long, default_value = "20")]
        limit: u32,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        follow: bool,
    },
    /// Show dashboard status
    Status,
    /// Export a task
    Export {
        id: String,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Import a task from YAML
    Import {
        file: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Daemon management
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// First-run setup
    Init,
    /// Generate a Claude prompt from a plain English intent
    Generate {
        /// Plain English description of what the task should do
        #[arg(long)]
        intent: String,
        /// Comma-separated agent slugs to include
        #[arg(long, value_delimiter = ',')]
        agents: Option<Vec<String>>,
        /// Working directory context (defaults to current dir)
        #[arg(long)]
        working_dir: Option<String>,
    },
    /// Agent registry management
    Agents {
        #[command(subcommand)]
        action: AgentsAction,
    },
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Start the daemon
    Start,
    /// Stop the daemon
    Stop,
    /// Show daemon status
    Status,
    /// Install daemon as a launchd service
    Install,
}

#[derive(Subcommand)]
enum AgentsAction {
    /// List available agents from the registry
    List {
        /// Filter by category
        #[arg(long)]
        category: Option<String>,
    },
    /// Force-refresh the agent registry from GitHub
    Refresh,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Get a configuration value (or all values if key is omitted)
    Get { key: Option<String> },
    /// Set a configuration value
    Set { key: String, value: String },
}

// ── IPC Client ───────────────────────────────────────────

async fn send_rpc(method: &str, params: serde_json::Value) -> anyhow::Result<serde_json::Value> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let paths = InternPaths::new();
    let stream = UnixStream::connect(&paths.socket_path).await.map_err(|_| {
        anyhow::anyhow!("Intern daemon is not running. Start it with: lc daemon start")
    })?;

    let (reader, mut writer) = stream.into_split();

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: method.to_string(),
        params,
        id: serde_json::Value::Number(1.into()),
    };

    let mut line = serde_json::to_string(&request)?;
    line.push('\n');
    writer.write_all(line.as_bytes()).await?;

    let mut reader = BufReader::new(reader);
    let mut response_line = String::new();
    reader.read_line(&mut response_line).await?;

    let response: JsonRpcResponse = serde_json::from_str(&response_line)?;

    if let Some(error) = response.error {
        anyhow::bail!("Error ({}): {}", error.code, error.message);
    }

    Ok(response.result.unwrap_or(serde_json::Value::Null))
}

// ── Formatting Helpers ───────────────────────────────────

/// Pad or truncate a string to the given width.
fn pad(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{s:<width$}")
    }
}

/// Format a duration in seconds to a human-readable string like "47s", "5m 12s", "2h 3m".
fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        let m = secs / 60;
        let s = secs % 60;
        if s == 0 {
            format!("{m}m")
        } else {
            format!("{m}m {s}s")
        }
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        if m == 0 {
            format!("{h}h")
        } else {
            format!("{h}h {m}m")
        }
    }
}

/// Format a cost in USD.
fn format_cost(cost: f64) -> String {
    format!("${cost:.2}")
}

/// Format a `DateTime<Utc>` as a local time string like "Mar 15, 02:00:12 PM".
fn format_time(dt: &DateTime<Utc>) -> String {
    let local: DateTime<Local> = dt.with_timezone(&Local);
    local.format("%b %d, %I:%M:%S %p").to_string()
}

/// Generate a sparkline string from a slice of f64 values.
///
/// Uses Unicode block characters scaled to the maximum value in the series.
/// Empty or all-zero input produces a flat line of `\u{2581}` characters.
fn sparkline(values: &[f64]) -> String {
    const BLOCKS: [char; 8] = [
        '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}',
        '\u{2588}',
    ];

    if values.is_empty() {
        return String::new();
    }

    let max = values.iter().cloned().fold(0.0_f64, f64::max);
    if max <= 0.0 {
        return BLOCKS[0].to_string().repeat(values.len());
    }

    values
        .iter()
        .map(|&v| {
            let normalized = (v / max).clamp(0.0, 1.0);
            let idx = (normalized * 7.0).round() as usize;
            BLOCKS[idx.min(7)]
        })
        .collect()
}

/// Parse a cron expression string into a `Schedule`.
///
/// Recognizes common cron patterns and converts them to the appropriate
/// `Schedule` variant that the daemon expects.
fn parse_schedule(expr: &str) -> Schedule {
    Schedule::Cron {
        expression: expr.to_string(),
    }
}

// ── Command Handlers ─────────────────────────────────────

async fn cmd_list() -> anyhow::Result<()> {
    let result = send_rpc("task.list", json!({})).await?;
    let tasks: Vec<Task> = serde_json::from_value(result)?;

    if tasks.is_empty() {
        println!("No tasks configured. Create one with: lc add --name \"...\" --command \"...\" --schedule \"...\"");
        return Ok(());
    }

    // Fetch metrics for run counts and health
    let metrics_result = send_rpc("metrics.dashboard", json!({})).await;
    let metrics_map: HashMap<String, (u64, f64)> = match metrics_result {
        Ok(val) => {
            let dm: DashboardMetrics = serde_json::from_value(val).unwrap_or(DashboardMetrics {
                total_tasks: 0,
                active_tasks: 0,
                total_runs: 0,
                overall_success_rate: 0.0,
                total_spend: 0.0,
                tasks: vec![],
                cost_trend: vec![],
            });
            dm.tasks
                .into_iter()
                .map(|tm| {
                    let health = if tm.total_runs == 0 {
                        100.0
                    } else {
                        (tm.success_count as f64 / tm.total_runs as f64) * 100.0
                    };
                    (tm.task_id.clone(), (tm.total_runs, health))
                })
                .collect()
        }
        Err(_) => HashMap::new(),
    };

    // Print header
    println!(
        "{}  {}  {}  {}  {}  HEALTH",
        pad("ID", 12),
        pad("NAME", 20),
        pad("SCHEDULE", 18),
        pad("STATUS", 8),
        pad("RUNS", 6),
    );

    for task in &tasks {
        let id = task.id.as_str();
        let (runs, health) = metrics_map.get(id).copied().unwrap_or((0, 100.0));

        println!(
            "{}  {}  {}  {}  {}  {:.0}%",
            pad(id, 12),
            pad(&task.name, 20),
            pad(&task.schedule_human, 18),
            pad(&task.status.to_string(), 8),
            pad(&runs.to_string(), 6),
            health,
        );
    }

    Ok(())
}

async fn cmd_add(
    name: String,
    command: String,
    schedule: String,
    working_dir: String,
    budget: Option<f64>,
    template: Option<String>,
    tags: Option<Vec<String>>,
) -> anyhow::Result<()> {
    // If a template slug is provided, fetch the template and merge with overrides
    let (final_name, final_command, final_schedule, final_working_dir, final_budget, final_tags) =
        if let Some(slug) = template {
            let templates_result = send_rpc("templates.list", json!({})).await?;
            let templates: Vec<intern_core::TaskTemplate> =
                serde_json::from_value(templates_result)?;
            let tmpl = templates
                .iter()
                .find(|t| t.slug == slug)
                .ok_or_else(|| anyhow::anyhow!("Template '{}' not found", slug))?;

            // CLI flags override template defaults; use template values as fallback
            let n = if name.is_empty() {
                tmpl.name.clone()
            } else {
                name
            };
            let c = if command.is_empty() {
                tmpl.command.clone()
            } else {
                command
            };
            let s = if schedule.is_empty() {
                tmpl.schedule.clone()
            } else {
                parse_schedule(&schedule)
            };
            let b = budget.or(Some(tmpl.max_budget_per_run));
            let t = tags.or_else(|| Some(tmpl.tags.clone()));
            (n, c, s, working_dir, b, t)
        } else {
            (
                name,
                command,
                parse_schedule(&schedule),
                working_dir,
                budget,
                tags,
            )
        };

    let schedule_human = final_schedule.to_human();

    let input = CreateTaskInput {
        name: final_name,
        command: final_command,
        skill: None,
        schedule: final_schedule,
        schedule_human: Some(schedule_human),
        working_dir: final_working_dir,
        env_vars: None,
        max_budget_per_run: final_budget,
        max_turns: None,
        timeout_secs: None,
        tags: final_tags,
        agents: None,
    };

    let result = send_rpc("task.create", serde_json::to_value(&input)?).await?;
    let task: Task = serde_json::from_value(result)?;

    println!(
        "Created task {} \"{}\" ({}, {})",
        task.id, task.name, task.status, task.schedule_human
    );

    Ok(())
}

async fn cmd_edit(
    id: String,
    name: Option<String>,
    command: Option<String>,
    schedule: Option<String>,
    working_dir: Option<String>,
    budget: Option<f64>,
) -> anyhow::Result<()> {
    let schedule_parsed = schedule.map(|s| parse_schedule(&s));
    let schedule_human = schedule_parsed.as_ref().map(|s| s.to_human());

    let input = intern_core::UpdateTaskInput {
        id: id.clone(),
        name,
        command,
        skill: None,
        schedule: schedule_parsed,
        schedule_human,
        working_dir,
        env_vars: None,
        max_budget_per_run: budget,
        max_turns: None,
        timeout_secs: None,
        tags: None,
        agents: None,
        status: None,
    };

    let result = send_rpc("task.update", serde_json::to_value(&input)?).await?;
    let task: Task = serde_json::from_value(result)?;

    println!("Updated task {} \"{}\"", task.id, task.name);

    Ok(())
}

async fn cmd_rm(id: String, skip_confirm: bool) -> anyhow::Result<()> {
    if !skip_confirm {
        // Fetch task name for the confirmation prompt
        let result = send_rpc("task.get", json!({ "id": id })).await?;
        let task: Task = serde_json::from_value(result)?;

        print!("Delete task '{}' ({})? [y/N]: ", task.name, task.id);
        std::io::stdout().flush()?;

        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        let answer = answer.trim().to_lowercase();

        if answer != "y" && answer != "yes" {
            println!("Cancelled.");
            return Ok(());
        }
    }

    send_rpc("task.delete", json!({ "id": id })).await?;
    println!("Deleted task {id}");

    Ok(())
}

async fn cmd_pause(id: String) -> anyhow::Result<()> {
    let result = send_rpc("task.pause", json!({ "id": id })).await?;
    let task: Task = serde_json::from_value(result)?;
    println!("Paused task {} \"{}\"", task.id, task.name);
    Ok(())
}

async fn cmd_resume(id: String) -> anyhow::Result<()> {
    let result = send_rpc("task.resume", json!({ "id": id })).await?;
    let task: Task = serde_json::from_value(result)?;
    println!("Resumed task {} \"{}\"", task.id, task.name);
    Ok(())
}

async fn cmd_run(id: String, dry_run: bool, json_output: bool) -> anyhow::Result<()> {
    if dry_run {
        let result = send_rpc("task.dry_run", json!({ "id": id })).await?;

        if json_output {
            println!("{}", serde_json::to_string_pretty(&result)?);
            return Ok(());
        }

        let dr: DryRunResult = serde_json::from_value(result)?;
        println!("Dry run for \"{}\" ({}):", dr.task_name, dr.task_id);
        println!("  Command:     {}", dr.resolved_command.join(" "));
        println!("  Working dir: {}", dr.working_dir);
        println!(
            "  Budget:      {}/run, {} remaining today",
            format_cost(dr.max_budget_per_run),
            format_cost((dr.max_budget_per_run * 20.0 - dr.daily_spend_so_far).max(0.0))
        );
        println!("  Schedule:    {}", dr.schedule_human);

        if dr.would_be_skipped {
            if let Some(reason) = &dr.skip_reason {
                println!("  WARNING:     Would be skipped: {reason}");
            }
        }

        return Ok(());
    }

    let result = send_rpc("task.run_now", json!({ "id": id })).await?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    // Try to get the task name for a friendlier message
    let task_result = send_rpc("task.get", json!({ "id": id })).await;
    let task_name = task_result
        .ok()
        .and_then(|v| serde_json::from_value::<Task>(v).ok())
        .map(|t| t.name)
        .unwrap_or_else(|| id.clone());

    println!("Triggered immediate run of \"{task_name}\"");

    Ok(())
}

async fn cmd_stop(id: String) -> anyhow::Result<()> {
    send_rpc("task.stop", json!({ "id": id })).await?;
    println!("Stopped task {id}");
    Ok(())
}

async fn cmd_logs(
    id: Option<String>,
    limit: u32,
    status: Option<String>,
    follow: bool,
) -> anyhow::Result<()> {
    let params = json!({
        "task_id": id,
        "status": status,
        "limit": limit,
    });

    let result = send_rpc("logs.query", params).await?;
    let logs: Vec<ExecutionLog> = serde_json::from_value(result)?;

    if logs.is_empty() {
        println!("No execution logs found.");
        if !follow {
            return Ok(());
        }
    }

    // Print header
    println!(
        "{}  {}  {}  {}  COST",
        pad("STATUS", 6),
        pad("TASK", 20),
        pad("TIME", 24),
        pad("DURATION", 10),
    );

    for log in &logs {
        let status_icon = match log.status {
            intern_core::ExecStatus::Success => "  \u{2713}",
            intern_core::ExecStatus::Failed => "  \u{2715}",
            intern_core::ExecStatus::Timeout => "  T",
            intern_core::ExecStatus::Killed => "  K",
            intern_core::ExecStatus::Skipped => "  -",
        };

        let cost_str = log
            .cost_usd
            .map(format_cost)
            .unwrap_or_else(|| "-".to_string());

        println!(
            "{}  {}  {}  {}  {}",
            pad(status_icon, 6),
            pad(&log.task_name, 20),
            pad(&format_time(&log.started_at), 24),
            pad(&format_duration(log.duration_secs), 10),
            cost_str,
        );
    }

    if follow {
        println!("\nFollowing logs (press Ctrl+C to stop)...\n");
        // For follow mode, poll every 2 seconds for new logs
        let mut last_id = logs.last().map(|l| l.id).unwrap_or(0);

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            let params = json!({
                "task_id": id,
                "status": serde_json::Value::Null,
                "limit": 50,
            });

            let result = match send_rpc("logs.query", params).await {
                Ok(r) => r,
                Err(_) => continue,
            };

            let new_logs: Vec<ExecutionLog> = match serde_json::from_value(result) {
                Ok(l) => l,
                Err(_) => continue,
            };

            for log in &new_logs {
                if log.id <= last_id {
                    continue;
                }
                last_id = log.id;

                let status_icon = match log.status {
                    intern_core::ExecStatus::Success => "  \u{2713}",
                    intern_core::ExecStatus::Failed => "  \u{2715}",
                    intern_core::ExecStatus::Timeout => "  T",
                    intern_core::ExecStatus::Killed => "  K",
                    intern_core::ExecStatus::Skipped => "  -",
                };

                let cost_str = log
                    .cost_usd
                    .map(format_cost)
                    .unwrap_or_else(|| "-".to_string());

                println!(
                    "{}  {}  {}  {}  {}",
                    pad(status_icon, 6),
                    pad(&log.task_name, 20),
                    pad(&format_time(&log.started_at), 24),
                    pad(&format_duration(log.duration_secs), 10),
                    cost_str,
                );
            }
        }
    }

    Ok(())
}

async fn cmd_status() -> anyhow::Result<()> {
    let result = send_rpc("metrics.dashboard", json!({})).await?;
    let metrics: DashboardMetrics = serde_json::from_value(result)?;

    // Count tasks by status
    let daemon_result = send_rpc("daemon.status", json!({})).await?;
    let pid = daemon_result
        .get("pid")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let uptime_str = daemon_result
        .get("uptime")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    // Fetch task list for status breakdown
    let task_result = send_rpc("task.list", json!({})).await;
    let (active, paused, errored) = match task_result {
        Ok(val) => {
            let tasks: Vec<Task> = serde_json::from_value(val).unwrap_or_default();
            let a = tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Active)
                .count();
            let p = tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Paused)
                .count();
            let e = tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Error)
                .count();
            (a, p, e)
        }
        Err(_) => (metrics.active_tasks as usize, 0, 0),
    };

    // Build status breakdown
    let mut breakdown_parts = Vec::new();
    if active > 0 {
        breakdown_parts.push(format!("{active} active"));
    }
    if paused > 0 {
        breakdown_parts.push(format!("{paused} paused"));
    }
    if errored > 0 {
        breakdown_parts.push(format!("{errored} error"));
    }
    let breakdown = if breakdown_parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", breakdown_parts.join(", "))
    };

    println!("Intern -- {} tasks{}", metrics.total_tasks, breakdown);
    println!(
        "Total runs: {}  |  Success: {:.1}%  |  Spend: {}",
        metrics.total_runs,
        metrics.overall_success_rate,
        format_cost(metrics.total_spend)
    );

    // Sparkline for 7-day cost trend
    if !metrics.cost_trend.is_empty() {
        let costs: Vec<f64> = metrics.cost_trend.iter().map(|d| d.total_cost).collect();
        let total_7d: f64 = costs.iter().sum();
        let spark = sparkline(&costs);
        println!("7-day spend: {}  {}", spark, format_cost(total_7d));
    }

    println!("Daemon: PID {pid}, uptime {uptime_str}");

    Ok(())
}

async fn cmd_export(id: String, output: Option<String>) -> anyhow::Result<()> {
    let result = send_rpc("task.export", json!({ "id": id })).await?;
    let export: TaskExport = serde_json::from_value(result)?;
    let yaml = serde_yaml::to_string(&export)?;

    match output {
        Some(path) => {
            let expanded = intern_core::expand_tilde(&path);
            std::fs::write(&expanded, &yaml)
                .with_context(|| format!("Failed to write to {}", expanded.display()))?;
            println!("Exported task {} to {}", id, expanded.display());
        }
        None => {
            print!("{yaml}");
        }
    }

    Ok(())
}

async fn cmd_import(file: String, dry_run: bool) -> anyhow::Result<()> {
    let expanded = intern_core::expand_tilde(&file);
    let contents = std::fs::read_to_string(&expanded)
        .with_context(|| format!("Failed to read {}", expanded.display()))?;
    let export: TaskExport = serde_yaml::from_str(&contents)
        .with_context(|| format!("Failed to parse YAML from {}", expanded.display()))?;

    if dry_run {
        println!("Would import task:");
        println!("  Name:        {}", export.name);
        println!("  Command:     {}", export.command);
        println!("  Schedule:    {}", export.schedule_human);
        println!("  Working dir: {}", export.working_dir);
        println!(
            "  Budget:      {}/run",
            format_cost(export.max_budget_per_run)
        );
        if !export.tags.is_empty() {
            println!("  Tags:        {}", export.tags.join(", "));
        }
        return Ok(());
    }

    let result = send_rpc("task.import", serde_json::to_value(&export)?).await?;
    let task: Task = serde_json::from_value(result)?;
    println!(
        "Imported task {} \"{}\" ({})",
        task.id, task.name, task.status
    );

    Ok(())
}

async fn cmd_daemon_start() -> anyhow::Result<()> {
    // Check if daemon is already running
    let paths = InternPaths::new();
    if paths.pid_file.exists() {
        if let Ok(pid_str) = std::fs::read_to_string(&paths.pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                // Check if process is alive using kill -0
                let alive = std::process::Command::new("kill")
                    .args(["-0", &pid.to_string()])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                if alive {
                    println!("Daemon is already running (PID {pid}).");
                    return Ok(());
                }
            }
        }
    }

    // Find the daemon binary
    let daemon_bin = find_daemon_binary()?;

    // Spawn as a detached background process
    let child = std::process::Command::new(&daemon_bin)
        .arg("--foreground")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("Failed to start daemon binary at {}", daemon_bin.display()))?;

    println!("Daemon started (PID {}).", child.id());

    Ok(())
}

async fn cmd_daemon_stop() -> anyhow::Result<()> {
    let paths = InternPaths::new();

    if !paths.pid_file.exists() {
        anyhow::bail!("No PID file found. Daemon may not be running.");
    }

    let pid_str =
        std::fs::read_to_string(&paths.pid_file).context("Failed to read daemon PID file")?;
    let pid: i32 = pid_str
        .trim()
        .parse()
        .context("Invalid PID in daemon.pid")?;

    // Send SIGTERM via kill command
    let status = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("Failed to execute kill command")?;

    if !status.success() {
        anyhow::bail!("Failed to send SIGTERM to PID {pid}");
    }

    println!("Sent SIGTERM to daemon (PID {pid}).");

    Ok(())
}

async fn cmd_daemon_status() -> anyhow::Result<()> {
    match send_rpc("daemon.status", json!({})).await {
        Ok(result) => {
            let pid = result.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
            let uptime = result
                .get("uptime")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let version = result
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let clients = result
                .get("connected_clients")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let claude = result
                .get("claude_available")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            println!("Daemon:  running");
            println!("PID:     {pid}");
            println!("Version: {version}");
            println!("Uptime:  {uptime}");
            println!("Clients: {clients}");
            println!(
                "Claude:  {}",
                if claude { "available" } else { "not found" }
            );
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("not running") {
                println!("Daemon:  not running");
            } else {
                return Err(e);
            }
        }
    }

    Ok(())
}

async fn cmd_daemon_install() -> anyhow::Result<()> {
    let paths = InternPaths::new();
    let daemon_bin = find_daemon_binary()?;

    let plist_path = paths.launch_agents_dir.join("com.intern.daemon.plist");

    // Ensure LaunchAgents directory exists
    std::fs::create_dir_all(&paths.launch_agents_dir)?;

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.intern.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>--foreground</string>
    </array>
    <key>KeepAlive</key>
    <true/>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{}/daemon.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>{}/daemon.stderr.log</string>
</dict>
</plist>
"#,
        daemon_bin.display(),
        paths.root.display(),
        paths.root.display(),
    );

    std::fs::write(&plist_path, &plist_content)?;
    println!("Wrote plist to {}", plist_path.display());

    // Load via launchctl bootstrap
    let uid = get_uid();
    let bootstrap_result = std::process::Command::new("launchctl")
        .args([
            "bootstrap",
            &format!("gui/{uid}"),
            &plist_path.to_string_lossy(),
        ])
        .output();

    match bootstrap_result {
        Ok(output) if output.status.success() || output.status.code() == Some(37) => {
            println!("Daemon installed and loaded via launchd.");
        }
        Ok(output) => {
            // Fallback to deprecated load
            let fallback = std::process::Command::new("launchctl")
                .args(["load", "-w", &plist_path.to_string_lossy()])
                .output();

            match fallback {
                Ok(fb) if fb.status.success() => {
                    println!("Daemon installed and loaded via launchd (legacy API).");
                }
                _ => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    eprintln!("Warning: launchctl bootstrap failed: {stderr}");
                    println!(
                        "Plist written to {}. Load manually with:\n  launchctl load -w {}",
                        plist_path.display(),
                        plist_path.display()
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("Warning: could not run launchctl: {e}");
            println!(
                "Plist written to {}. Load manually with:\n  launchctl load -w {}",
                plist_path.display(),
                plist_path.display()
            );
        }
    }

    Ok(())
}

async fn cmd_config_get(key: Option<String>) -> anyhow::Result<()> {
    let result = send_rpc("config.get", json!({})).await?;

    match key {
        Some(k) => {
            if let Some(val) = result.get(&k) {
                println!("{k} = {val}");
            } else {
                anyhow::bail!("Unknown config key: {k}");
            }
        }
        None => {
            if let serde_json::Value::Object(map) = &result {
                for (k, v) in map {
                    println!("{k} = {v}");
                }
            } else {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        }
    }

    Ok(())
}

async fn cmd_config_set(key: String, value: String) -> anyhow::Result<()> {
    // Try to parse value as JSON; fall back to string
    let json_value: serde_json::Value =
        serde_json::from_str(&value).unwrap_or_else(|_| serde_json::Value::String(value.clone()));

    let mut params = serde_json::Map::new();
    params.insert(key.clone(), json_value);

    send_rpc("config.update", serde_json::Value::Object(params)).await?;
    println!("Set {key} = {value}");

    Ok(())
}

async fn cmd_init() -> anyhow::Result<()> {
    let paths = InternPaths::new();

    // 1. Create directories
    paths.ensure_dirs()?;
    println!("Created directories at {}", paths.root.display());

    // 2. Write default config.yaml if it doesn't exist
    if !paths.config_file.exists() {
        let default_config = r#"version: 1
claude_binary: claude
default_budget: 5.0
default_timeout: 600
default_max_turns: null
log_retention_days: 30
notifications_enabled: true
max_concurrent_tasks: 4
daily_budget_cap: null
cost_estimate_per_second: 0.01
theme: dark
"#;
        std::fs::write(&paths.config_file, default_config)?;
        println!("Wrote default config to {}", paths.config_file.display());
    } else {
        println!("Config already exists at {}", paths.config_file.display());
    }

    // 3. Print welcome message
    println!();
    println!("Welcome to Intern!");
    println!();
    println!("Get started:");
    println!("  lc daemon start                    # Start the background daemon");
    println!("  lc add --name \"PR Review\" \\");
    println!("         --command \"claude -p 'Review open PRs'\" \\");
    println!("         --schedule \"0 */2 * * *\"     # Add a task");
    println!("  lc list                            # List all tasks");
    println!("  lc status                          # Dashboard overview");
    println!("  lc logs                            # View execution history");
    println!();
    println!("Use templates for quick setup:");
    println!("  lc add --template pr-review --name \"My PR Review\" --schedule \"0 */2 * * *\"");
    println!();
    println!("For more help: lc --help");

    Ok(())
}

async fn cmd_generate(
    intent: String,
    agents: Option<Vec<String>>,
    working_dir: Option<String>,
) -> anyhow::Result<()> {
    let resolved_dir = match working_dir {
        Some(d) => d,
        None => std::env::current_dir()
            .context("Failed to determine current directory")?
            .to_string_lossy()
            .into_owned(),
    };

    let params = json!({
        "intent": intent,
        "agents": agents.unwrap_or_default(),
        "working_dir": resolved_dir,
    });

    let result = send_rpc("prompt.generate", params).await?;

    let name = result.get("name").and_then(|v| v.as_str()).unwrap_or("-");
    let description = result
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let tags = result
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "-".to_string());
    let agents_used = result
        .get("agents")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "-".to_string());
    let saved_to = result
        .get("saved_to")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let command = result.get("command").and_then(|v| v.as_str()).unwrap_or("");

    println!("{}  {}", pad("Generated prompt:", 18), name);
    println!("{}  {}", pad("Description:", 18), description);
    println!("{}  {}", pad("Tags:", 18), tags);
    println!("{}  {}", pad("Agents:", 18), agents_used);
    println!("{}  {}", pad("Saved to:", 18), saved_to);

    if !command.is_empty() {
        println!();
        println!("Preview:");
        let preview = if command.len() > 500 {
            &command[..500]
        } else {
            command
        };
        println!("{preview}");
        if command.len() > 500 {
            println!("... (truncated)");
        }
    }

    Ok(())
}

async fn cmd_agents_list(category: Option<String>) -> anyhow::Result<()> {
    let result = send_rpc("registry.list", json!({})).await?;
    let mut agents: Vec<AgentEntry> = serde_json::from_value(result)?;

    if let Some(ref cat) = category {
        agents.retain(|a| a.category.eq_ignore_ascii_case(cat));
    }

    if agents.is_empty() {
        if category.is_some() {
            println!("No agents found in that category.");
        } else {
            println!("No agents in registry. Populate it with: lc agents refresh");
        }
        return Ok(());
    }

    println!(
        "{}  {}  {}  DESCRIPTION",
        pad("SLUG", 28),
        pad("NAME", 24),
        pad("CATEGORY", 14),
    );

    for agent in &agents {
        let description_col = if agent.description.len() > 60 {
            &agent.description[..60]
        } else {
            &agent.description
        };
        println!(
            "{}  {}  {}  {}",
            pad(&agent.slug, 28),
            pad(&agent.name, 24),
            pad(&agent.category, 14),
            description_col,
        );
    }

    Ok(())
}

async fn cmd_agents_refresh() -> anyhow::Result<()> {
    let result = send_rpc("registry.refresh", json!({})).await?;
    let count = result.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
    println!("Registry refreshed. {count} agents available.");
    Ok(())
}

// ── Utility Functions ────────────────────────────────────

/// Find the intern daemon binary.
///
/// Search order:
/// 1. Same directory as the current executable
/// 2. `~/.cargo/bin/intern`
/// 3. `/usr/local/bin/intern`
/// 4. `which intern`
fn find_daemon_binary() -> anyhow::Result<std::path::PathBuf> {
    // 1. Same directory as current exe
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            let candidate = dir.join("intern");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    // 2. ~/.cargo/bin
    if let Ok(home) = std::env::var("HOME") {
        let candidate = std::path::PathBuf::from(&home).join(".cargo/bin/intern");
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    // 3. /usr/local/bin
    let candidate = std::path::PathBuf::from("/usr/local/bin/intern");
    if candidate.exists() {
        return Ok(candidate);
    }

    // 4. which
    let output = std::process::Command::new("which").arg("intern").output();
    if let Ok(output) = output {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(std::path::PathBuf::from(path));
            }
        }
    }

    anyhow::bail!("Cannot find 'intern' binary. Build it with: cargo build -p intern-daemon");
}

/// Get the current user's UID for launchctl commands.
fn get_uid() -> u32 {
    let output = std::process::Command::new("id")
        .arg("-u")
        .output()
        .expect("failed to run `id -u`");
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("invalid uid from `id -u`")
}

// ── Entrypoint ───────────────────────────────────────────

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::List => cmd_list().await,
        Commands::Add {
            name,
            command,
            schedule,
            working_dir,
            budget,
            template,
            tags,
        } => cmd_add(name, command, schedule, working_dir, budget, template, tags).await,
        Commands::Edit {
            id,
            name,
            command,
            schedule,
            working_dir,
            budget,
        } => cmd_edit(id, name, command, schedule, working_dir, budget).await,
        Commands::Rm { id, y } => cmd_rm(id, y).await,
        Commands::Pause { id } => cmd_pause(id).await,
        Commands::Resume { id } => cmd_resume(id).await,
        Commands::Run { id, dry_run, json } => cmd_run(id, dry_run, json).await,
        Commands::Stop { id } => cmd_stop(id).await,
        Commands::Logs {
            id,
            limit,
            status,
            follow,
        } => cmd_logs(id, limit, status, follow).await,
        Commands::Status => cmd_status().await,
        Commands::Export { id, output } => cmd_export(id, output).await,
        Commands::Import { file, dry_run } => cmd_import(file, dry_run).await,
        Commands::Daemon { action } => match action {
            DaemonAction::Start => cmd_daemon_start().await,
            DaemonAction::Stop => cmd_daemon_stop().await,
            DaemonAction::Status => cmd_daemon_status().await,
            DaemonAction::Install => cmd_daemon_install().await,
        },
        Commands::Config { action } => match action {
            ConfigAction::Get { key } => cmd_config_get(key).await,
            ConfigAction::Set { key, value } => cmd_config_set(key, value).await,
        },
        Commands::Init => cmd_init().await,
        Commands::Generate {
            intent,
            agents,
            working_dir,
        } => cmd_generate(intent, agents, working_dir).await,
        Commands::Agents { action } => match action {
            AgentsAction::List { category } => cmd_agents_list(category).await,
            AgentsAction::Refresh => cmd_agents_refresh().await,
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

// ── Tests ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // -- CLI Argument Parsing -----------------------------------------

    #[test]
    fn parse_list() {
        let cli = Cli::parse_from(["intern", "list"]);
        assert!(matches!(cli.command, Commands::List));
    }

    #[test]
    fn parse_add_minimal() {
        let cli = Cli::parse_from([
            "intern",
            "add",
            "--name",
            "Test Task",
            "--command",
            "echo hello",
            "--schedule",
            "*/5 * * * *",
        ]);
        match cli.command {
            Commands::Add {
                name,
                command,
                schedule,
                working_dir,
                budget,
                template,
                tags,
            } => {
                assert_eq!(name, "Test Task");
                assert_eq!(command, "echo hello");
                assert_eq!(schedule, "*/5 * * * *");
                assert_eq!(working_dir, "~/");
                assert!(budget.is_none());
                assert!(template.is_none());
                assert!(tags.is_none());
            }
            _ => panic!("Expected Add command"),
        }
    }

    #[test]
    fn parse_add_full() {
        let cli = Cli::parse_from([
            "intern",
            "add",
            "--name",
            "PR Review",
            "--command",
            "claude -p 'Review PRs'",
            "--schedule",
            "0 */2 * * *",
            "--working-dir",
            "/Users/test/projects",
            "--budget",
            "10.0",
            "--template",
            "pr-review",
            "--tags",
            "review",
            "automation",
        ]);
        match cli.command {
            Commands::Add {
                name,
                command,
                schedule,
                working_dir,
                budget,
                template,
                tags,
            } => {
                assert_eq!(name, "PR Review");
                assert_eq!(command, "claude -p 'Review PRs'");
                assert_eq!(schedule, "0 */2 * * *");
                assert_eq!(working_dir, "/Users/test/projects");
                assert_eq!(budget, Some(10.0));
                assert_eq!(template, Some("pr-review".to_string()));
                assert_eq!(
                    tags,
                    Some(vec!["review".to_string(), "automation".to_string()])
                );
            }
            _ => panic!("Expected Add command"),
        }
    }

    #[test]
    fn parse_edit() {
        let cli = Cli::parse_from([
            "intern",
            "edit",
            "lc-abc12345",
            "--name",
            "New Name",
            "--schedule",
            "*/10 * * * *",
        ]);
        match cli.command {
            Commands::Edit {
                id, name, schedule, ..
            } => {
                assert_eq!(id, "lc-abc12345");
                assert_eq!(name, Some("New Name".to_string()));
                assert_eq!(schedule, Some("*/10 * * * *".to_string()));
            }
            _ => panic!("Expected Edit command"),
        }
    }

    #[test]
    fn parse_rm_without_confirm() {
        let cli = Cli::parse_from(["intern", "rm", "lc-abc12345"]);
        match cli.command {
            Commands::Rm { id, y } => {
                assert_eq!(id, "lc-abc12345");
                assert!(!y);
            }
            _ => panic!("Expected Rm command"),
        }
    }

    #[test]
    fn parse_rm_with_confirm() {
        let cli = Cli::parse_from(["intern", "rm", "lc-abc12345", "-y"]);
        match cli.command {
            Commands::Rm { id, y } => {
                assert_eq!(id, "lc-abc12345");
                assert!(y);
            }
            _ => panic!("Expected Rm command"),
        }
    }

    #[test]
    fn parse_pause() {
        let cli = Cli::parse_from(["intern", "pause", "lc-abc12345"]);
        match cli.command {
            Commands::Pause { id } => assert_eq!(id, "lc-abc12345"),
            _ => panic!("Expected Pause command"),
        }
    }

    #[test]
    fn parse_resume() {
        let cli = Cli::parse_from(["intern", "resume", "lc-abc12345"]);
        match cli.command {
            Commands::Resume { id } => assert_eq!(id, "lc-abc12345"),
            _ => panic!("Expected Resume command"),
        }
    }

    #[test]
    fn parse_run() {
        let cli = Cli::parse_from(["intern", "run", "lc-abc12345"]);
        match cli.command {
            Commands::Run { id, dry_run, json } => {
                assert_eq!(id, "lc-abc12345");
                assert!(!dry_run);
                assert!(!json);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn parse_run_dry_run() {
        let cli = Cli::parse_from(["intern", "run", "lc-abc12345", "--dry-run"]);
        match cli.command {
            Commands::Run { id, dry_run, json } => {
                assert_eq!(id, "lc-abc12345");
                assert!(dry_run);
                assert!(!json);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn parse_run_json() {
        let cli = Cli::parse_from(["intern", "run", "lc-abc12345", "--dry-run", "--json"]);
        match cli.command {
            Commands::Run { id, dry_run, json } => {
                assert_eq!(id, "lc-abc12345");
                assert!(dry_run);
                assert!(json);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn parse_stop() {
        let cli = Cli::parse_from(["intern", "stop", "lc-abc12345"]);
        match cli.command {
            Commands::Stop { id } => assert_eq!(id, "lc-abc12345"),
            _ => panic!("Expected Stop command"),
        }
    }

    #[test]
    fn parse_logs_no_args() {
        let cli = Cli::parse_from(["intern", "logs"]);
        match cli.command {
            Commands::Logs {
                id,
                limit,
                status,
                follow,
            } => {
                assert!(id.is_none());
                assert_eq!(limit, 20);
                assert!(status.is_none());
                assert!(!follow);
            }
            _ => panic!("Expected Logs command"),
        }
    }

    #[test]
    fn parse_logs_with_args() {
        let cli = Cli::parse_from([
            "intern",
            "logs",
            "lc-abc12345",
            "--limit",
            "50",
            "--status",
            "failed",
            "--follow",
        ]);
        match cli.command {
            Commands::Logs {
                id,
                limit,
                status,
                follow,
            } => {
                assert_eq!(id, Some("lc-abc12345".to_string()));
                assert_eq!(limit, 50);
                assert_eq!(status, Some("failed".to_string()));
                assert!(follow);
            }
            _ => panic!("Expected Logs command"),
        }
    }

    #[test]
    fn parse_status() {
        let cli = Cli::parse_from(["intern", "status"]);
        assert!(matches!(cli.command, Commands::Status));
    }

    #[test]
    fn parse_export() {
        let cli = Cli::parse_from(["intern", "export", "lc-abc12345", "-o", "task.yaml"]);
        match cli.command {
            Commands::Export { id, output } => {
                assert_eq!(id, "lc-abc12345");
                assert_eq!(output, Some("task.yaml".to_string()));
            }
            _ => panic!("Expected Export command"),
        }
    }

    #[test]
    fn parse_export_stdout() {
        let cli = Cli::parse_from(["intern", "export", "lc-abc12345"]);
        match cli.command {
            Commands::Export { id, output } => {
                assert_eq!(id, "lc-abc12345");
                assert!(output.is_none());
            }
            _ => panic!("Expected Export command"),
        }
    }

    #[test]
    fn parse_import() {
        let cli = Cli::parse_from(["intern", "import", "task.yaml"]);
        match cli.command {
            Commands::Import { file, dry_run } => {
                assert_eq!(file, "task.yaml");
                assert!(!dry_run);
            }
            _ => panic!("Expected Import command"),
        }
    }

    #[test]
    fn parse_import_dry_run() {
        let cli = Cli::parse_from(["intern", "import", "task.yaml", "--dry-run"]);
        match cli.command {
            Commands::Import { file, dry_run } => {
                assert_eq!(file, "task.yaml");
                assert!(dry_run);
            }
            _ => panic!("Expected Import command"),
        }
    }

    #[test]
    fn parse_daemon_start() {
        let cli = Cli::parse_from(["intern", "daemon", "start"]);
        match cli.command {
            Commands::Daemon { action } => {
                assert!(matches!(action, DaemonAction::Start));
            }
            _ => panic!("Expected Daemon command"),
        }
    }

    #[test]
    fn parse_daemon_stop() {
        let cli = Cli::parse_from(["intern", "daemon", "stop"]);
        match cli.command {
            Commands::Daemon { action } => {
                assert!(matches!(action, DaemonAction::Stop));
            }
            _ => panic!("Expected Daemon command"),
        }
    }

    #[test]
    fn parse_daemon_status() {
        let cli = Cli::parse_from(["intern", "daemon", "status"]);
        match cli.command {
            Commands::Daemon { action } => {
                assert!(matches!(action, DaemonAction::Status));
            }
            _ => panic!("Expected Daemon command"),
        }
    }

    #[test]
    fn parse_daemon_install() {
        let cli = Cli::parse_from(["intern", "daemon", "install"]);
        match cli.command {
            Commands::Daemon { action } => {
                assert!(matches!(action, DaemonAction::Install));
            }
            _ => panic!("Expected Daemon command"),
        }
    }

    #[test]
    fn parse_config_get_all() {
        let cli = Cli::parse_from(["intern", "config", "get"]);
        match cli.command {
            Commands::Config { action } => match action {
                ConfigAction::Get { key } => assert!(key.is_none()),
                _ => panic!("Expected Get action"),
            },
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn parse_config_get_key() {
        let cli = Cli::parse_from(["intern", "config", "get", "default_budget"]);
        match cli.command {
            Commands::Config { action } => match action {
                ConfigAction::Get { key } => {
                    assert_eq!(key, Some("default_budget".to_string()));
                }
                _ => panic!("Expected Get action"),
            },
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn parse_config_set() {
        let cli = Cli::parse_from(["intern", "config", "set", "default_budget", "10.0"]);
        match cli.command {
            Commands::Config { action } => match action {
                ConfigAction::Set { key, value } => {
                    assert_eq!(key, "default_budget");
                    assert_eq!(value, "10.0");
                }
                _ => panic!("Expected Set action"),
            },
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn parse_init() {
        let cli = Cli::parse_from(["intern", "init"]);
        assert!(matches!(cli.command, Commands::Init));
    }

    // -- Sparkline Generation -----------------------------------------

    #[test]
    fn sparkline_empty() {
        assert_eq!(sparkline(&[]), "");
    }

    #[test]
    fn sparkline_all_zeros() {
        assert_eq!(sparkline(&[0.0, 0.0, 0.0]), "\u{2581}\u{2581}\u{2581}");
    }

    #[test]
    fn sparkline_single_value() {
        let result = sparkline(&[5.0]);
        assert_eq!(result, "\u{2588}");
    }

    #[test]
    fn sparkline_ascending() {
        let result = sparkline(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
        // Each value should map to progressively higher blocks
        assert_eq!(result.chars().count(), 8);
        let chars: Vec<char> = result.chars().collect();
        // 0 -> lowest block, 7/7 = max -> highest block
        assert_eq!(chars[0], '\u{2581}');
        assert_eq!(chars[7], '\u{2588}');
        // Verify strictly non-decreasing
        for i in 0..7 {
            assert!(
                chars[i] <= chars[i + 1],
                "Block at {i} should be <= block at {}",
                i + 1
            );
        }
    }

    #[test]
    fn sparkline_uniform() {
        // All same value -> all max blocks
        let result = sparkline(&[5.0, 5.0, 5.0]);
        assert_eq!(result, "\u{2588}\u{2588}\u{2588}");
    }

    #[test]
    fn sparkline_mixed() {
        let result = sparkline(&[0.0, 10.0, 5.0]);
        let chars: Vec<char> = result.chars().collect();
        assert_eq!(chars.len(), 3);
        assert_eq!(chars[0], '\u{2581}'); // 0 -> lowest
        assert_eq!(chars[1], '\u{2588}'); // 10 -> highest
                                          // 5/10 = 0.5 -> index round(0.5*7) = round(3.5) = 4 -> BLOCKS[4] = \u{2585}
        assert_eq!(chars[2], '\u{2585}');
    }

    // -- Table Formatting ---------------------------------------------

    #[test]
    fn pad_shorter_string() {
        assert_eq!(pad("abc", 8), "abc     ");
    }

    #[test]
    fn pad_exact_length() {
        assert_eq!(pad("abcdefgh", 8), "abcdefgh");
    }

    #[test]
    fn pad_longer_string_truncates() {
        assert_eq!(pad("abcdefghij", 8), "abcdefgh");
    }

    #[test]
    fn format_duration_seconds() {
        assert_eq!(format_duration(47), "47s");
    }

    #[test]
    fn format_duration_minutes() {
        assert_eq!(format_duration(312), "5m 12s");
    }

    #[test]
    fn format_duration_exact_minutes() {
        assert_eq!(format_duration(300), "5m");
    }

    #[test]
    fn format_duration_hours() {
        assert_eq!(format_duration(7380), "2h 3m");
    }

    #[test]
    fn format_duration_exact_hours() {
        assert_eq!(format_duration(7200), "2h");
    }

    #[test]
    fn format_cost_normal() {
        assert_eq!(format_cost(0.31), "$0.31");
    }

    #[test]
    fn format_cost_large() {
        assert_eq!(format_cost(35.56), "$35.56");
    }

    #[test]
    fn format_cost_zero() {
        assert_eq!(format_cost(0.0), "$0.00");
    }

    // -- Schedule Parsing ---------------------------------------------

    #[test]
    fn parse_schedule_as_cron() {
        let sched = parse_schedule("*/15 * * * *");
        match sched {
            Schedule::Cron { expression } => {
                assert_eq!(expression, "*/15 * * * *");
            }
            _ => panic!("Expected Cron schedule"),
        }
    }

    // -- Generate / Agents Parsing ------------------------------------

    #[test]
    fn parse_generate_minimal() {
        let cli = Cli::parse_from([
            "intern",
            "generate",
            "--intent",
            "Review open PRs for security",
        ]);
        match cli.command {
            Commands::Generate {
                intent,
                agents,
                working_dir,
            } => {
                assert_eq!(intent, "Review open PRs for security");
                assert!(agents.is_none());
                assert!(working_dir.is_none());
            }
            _ => panic!("Expected Generate command"),
        }
    }

    #[test]
    fn parse_generate_with_agents() {
        let cli = Cli::parse_from([
            "intern",
            "generate",
            "--intent",
            "Audit dependencies",
            "--agents",
            "security-auditor,code-reviewer",
            "--working-dir",
            "/projects/myapp",
        ]);
        match cli.command {
            Commands::Generate {
                intent,
                agents,
                working_dir,
            } => {
                assert_eq!(intent, "Audit dependencies");
                assert_eq!(
                    agents,
                    Some(vec![
                        "security-auditor".to_string(),
                        "code-reviewer".to_string(),
                    ])
                );
                assert_eq!(working_dir, Some("/projects/myapp".to_string()));
            }
            _ => panic!("Expected Generate command"),
        }
    }

    #[test]
    fn parse_agents_list() {
        let cli = Cli::parse_from(["intern", "agents", "list", "--category", "security"]);
        match cli.command {
            Commands::Agents { action } => match action {
                AgentsAction::List { category } => {
                    assert_eq!(category, Some("security".to_string()));
                }
                _ => panic!("Expected List action"),
            },
            _ => panic!("Expected Agents command"),
        }
    }

    #[test]
    fn parse_agents_refresh() {
        let cli = Cli::parse_from(["intern", "agents", "refresh"]);
        match cli.command {
            Commands::Agents { action } => {
                assert!(matches!(action, AgentsAction::Refresh));
            }
            _ => panic!("Expected Agents command"),
        }
    }
}

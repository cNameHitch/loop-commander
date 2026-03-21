//! `intern-runner` -- standalone binary invoked by launchd for each task execution.
//!
//! Usage: `intern-runner --task-id <id>`
//!
//! Implements the 15-step execution flow defined in `specs.md` Section 5 and
//! `IMPLEMENTATION_PROMPT.md` Section 2B.

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Parser;
use intern_config::{sanitize_task_name, ConfigManager};
use intern_core::{ExecStatus, ExecutionLog, InternPaths, TaskStatus};
use intern_logger::Logger;
use intern_runner::{
    build_command, check_budget, estimate_cost, generate_summary, parse_cost_from_output,
};
use tracing::{error, info, warn};

/// CLI arguments for the intern-runner binary.
#[derive(Parser)]
#[command(name = "intern-runner", about = "Intern task executor")]
struct CliArgs {
    /// The task ID to execute (e.g. lc-a1b2c3d4).
    #[arg(long)]
    task_id: String,
}

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<()> {
    // ── Step 1: Parse CLI args ──────────────────────────
    let cli = CliArgs::parse();
    let task_id = &cli.task_id;

    // ── Step 2: Initialize tracing (to stderr) ──────────
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!(task_id = %task_id, "intern-runner starting");

    // ── Step 3: Load InternPaths, read task YAML ────────────
    let paths = InternPaths::new();
    let config_manager = ConfigManager::new(paths).context("failed to initialize ConfigManager")?;

    let global_config = config_manager.global_config().clone();

    let task = config_manager
        .get_task(task_id)
        .context("failed to read task YAML")?;

    info!(
        task_id = %task_id,
        task_name = %task.name,
        "loaded task configuration"
    );

    // ── Step 4: Open SQLite DB via Logger ────────────────
    let logger =
        Logger::new(&config_manager.paths().db_file).context("failed to open SQLite database")?;

    // ── Step 5: Concurrency check ───────────────────────
    // Use a directory-based counting semaphore under ~/.intern/locks/.
    let locks_dir = config_manager.paths().root.join("locks");
    std::fs::create_dir_all(&locks_dir).context("failed to create locks directory")?;

    let lock_file_path = locks_dir.join(format!("{task_id}.lock"));
    let max_concurrent = global_config.max_concurrent_tasks;

    let lock_file = match acquire_concurrency_slot(
        &locks_dir,
        &lock_file_path,
        max_concurrent,
        Duration::from_secs(60),
    ) {
        Ok(file) => file,
        Err(reason) => {
            warn!(task_id = %task_id, reason = %reason, "skipping due to concurrency limit");
            log_skipped(&logger, task_id, &task.name, &reason);
            info!(task_id = %task_id, "exiting with code 0 (skipped)");
            return Ok(());
        }
    };

    // ── Step 6: Budget check ────────────────────────────
    let daily_cap = global_config
        .daily_budget_cap
        .unwrap_or(task.max_budget_per_run * 20.0);

    match check_budget(&logger, task_id, daily_cap) {
        Ok(true) => {
            info!(task_id = %task_id, daily_cap = daily_cap, "budget check passed");
        }
        Ok(false) => {
            warn!(
                task_id = %task_id,
                daily_cap = daily_cap,
                "daily budget cap reached, skipping"
            );
            log_skipped(&logger, task_id, &task.name, "Daily budget cap reached");
            drop(lock_file);
            cleanup_lock_file(&lock_file_path);
            return Ok(());
        }
        Err(e) => {
            warn!(
                task_id = %task_id,
                "budget check failed, proceeding anyway: {e}"
            );
        }
    }

    // ── Step 7: Update task status to Running ───────────
    let started_at = Utc::now();
    {
        let mut running_task = task.clone();
        running_task.status = TaskStatus::Running;
        running_task.updated_at = started_at;
        if let Err(e) = config_manager.save_task(&running_task) {
            warn!("failed to update task status to Running: {e}");
        }
    }

    // ── Step 8: Build and execute command ────────────────

    // Construct the context file path.
    let sanitized = sanitize_task_name(&task.name);
    let context_file_path = {
        let primary = task
            .working_dir
            .join(".claude")
            .join("commands")
            .join(format!("{sanitized}.md"));
        if primary.exists() {
            primary
        } else {
            let short_id = &task.id.as_str()[3..task.id.as_str().len().min(11)];
            task.working_dir
                .join(".claude")
                .join("commands")
                .join(format!("{sanitized}-{short_id}.md"))
        }
    };

    let command_argv = build_command(&task, Some(&context_file_path));
    info!(task_id = %task_id, argv = ?command_argv, "built command");

    let (exec_status, exit_code, stdout_str, stderr_str, duration_secs, finished_at) =
        execute_task(task_id, &task, &command_argv).await;

    info!(
        task_id = %task_id,
        status = %exec_status,
        exit_code = exit_code,
        duration_secs = duration_secs,
        "task execution completed"
    );

    // ── Step 11: Parse cost from output (CC-10) ─────────
    let (tokens_used, mut cost_usd, _) = parse_cost_from_output(&stdout_str);

    // If no actual cost was extracted, use duration-based fallback.
    let cost_is_estimate = if cost_usd.is_none() {
        cost_usd = Some(estimate_cost(
            duration_secs,
            global_config.cost_estimate_per_second,
        ));
        true
    } else {
        false
    };

    // ── Step 12: Generate summary ───────────────────────
    let summary = generate_summary(&stdout_str, &stderr_str);

    // ── Step 13: Write ExecutionLog to SQLite ────────────
    let log_entry = ExecutionLog {
        id: 0,
        task_id: task_id.clone(),
        task_name: task.name.clone(),
        started_at,
        finished_at,
        duration_secs,
        exit_code,
        status: exec_status,
        stdout: stdout_str,
        stderr: stderr_str,
        tokens_used,
        cost_usd,
        cost_is_estimate,
        summary,
    };

    match logger.insert_log(&log_entry) {
        Ok(log_id) => {
            info!(task_id = %task_id, log_id = log_id, "execution log written");
        }
        Err(e) => {
            error!(task_id = %task_id, "failed to write execution log: {e}");
        }
    }

    // ── Step 14: Update task status ─────────────────────
    // Always restore to Active so the schedule continues running.
    // Failures are recorded in the execution log, not the task status.
    // The only exception is if the task was already Paused or Disabled
    // before this run -- in that case we preserve the original status.
    let new_status = match task.status {
        TaskStatus::Paused | TaskStatus::Disabled => task.status,
        _ => TaskStatus::Active,
    };
    restore_task_status(&config_manager, &task, new_status);

    // ── Step 15: Release lock and exit ──────────────────
    drop(lock_file);
    cleanup_lock_file(&lock_file_path);

    info!(task_id = %task_id, exit_code = exit_code, "intern-runner finished");

    // Exit with the captured exit code.
    std::process::exit(exit_code);
}

/// Build, spawn, and await the child process, handling timeout.
///
/// Returns `(exec_status, exit_code, stdout, stderr, duration_secs, finished_at)`.
async fn execute_task(
    task_id: &str,
    task: &intern_core::Task,
    command_argv: &[String],
) -> (ExecStatus, i32, String, String, u64, chrono::DateTime<Utc>) {
    let started_at = Utc::now();

    if command_argv.is_empty() {
        error!(task_id = %task_id, "empty command, aborting");
        let now = Utc::now();
        return (
            ExecStatus::Failed,
            1,
            String::new(),
            "Empty command".to_string(),
            0,
            now,
        );
    }

    // Resolve working directory.
    let working_dir = if task.working_dir.to_string_lossy().starts_with('~') {
        intern_core::expand_tilde(&task.working_dir.to_string_lossy())
    } else {
        task.working_dir.clone()
    };

    // Ensure the working directory exists (create if missing).
    if !working_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&working_dir) {
            error!(task_id = %task_id, dir = %working_dir.display(), "failed to create working directory: {e}");
        }
    }

    // Resolve the executable to an absolute path. Under launchd the PATH is
    // minimal (/usr/bin:/bin:/usr/sbin:/sbin), so a bare "claude" won't be
    // found. We check well-known install locations first.
    let executable = resolve_executable(&command_argv[0]);

    let mut cmd = tokio::process::Command::new(&executable);
    cmd.args(&command_argv[1..])
        .current_dir(&working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // Merge task-specific environment variables.
    for (key, value) in &task.env_vars {
        cmd.env(key, value);
    }

    // ── Step 9: Spawn and await with timeout ────────────
    let timeout_duration = Duration::from_secs(task.timeout_secs);

    let (exec_status, exit_code, stdout_str, stderr_str) =
        match tokio::time::timeout(timeout_duration, spawn_and_wait(&mut cmd)).await {
            Ok(Ok((status, code, stdout, stderr))) => (status, code, stdout, stderr),
            Ok(Err(e)) => {
                error!(task_id = %task_id, "failed to spawn process: {e}");
                (
                    ExecStatus::Failed,
                    1,
                    String::new(),
                    format!("Spawn error: {e}"),
                )
            }
            Err(_elapsed) => {
                warn!(task_id = %task_id, "task timed out after {}s", task.timeout_secs);
                (
                    ExecStatus::Timeout,
                    124, // conventional timeout exit code
                    String::new(),
                    format!("Task timed out after {}s", task.timeout_secs),
                )
            }
        };

    let finished_at = Utc::now();
    let duration_secs = (finished_at - started_at).num_seconds().unsigned_abs();

    (
        exec_status,
        exit_code,
        stdout_str,
        stderr_str,
        duration_secs,
        finished_at,
    )
}

/// Spawn the child process and wait for it to complete.
///
/// Returns `(exec_status, exit_code, stdout, stderr)`.
async fn spawn_and_wait(
    cmd: &mut tokio::process::Command,
) -> Result<(ExecStatus, i32, String, String)> {
    let output = cmd
        .output()
        .await
        .context("failed to execute child process")?;

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    let status = if output.status.success() {
        ExecStatus::Success
    } else if output.status.code().is_none() {
        // Process was killed by a signal.
        ExecStatus::Killed
    } else {
        ExecStatus::Failed
    };

    Ok((status, exit_code, stdout, stderr))
}

/// Log a Skipped execution to the database.
fn log_skipped(logger: &Logger, task_id: &str, task_name: &str, reason: &str) {
    let now = Utc::now();
    let log_entry = ExecutionLog {
        id: 0,
        task_id: task_id.to_string(),
        task_name: task_name.to_string(),
        started_at: now,
        finished_at: now,
        duration_secs: 0,
        exit_code: 0,
        status: ExecStatus::Skipped,
        stdout: String::new(),
        stderr: String::new(),
        tokens_used: None,
        cost_usd: None,
        cost_is_estimate: false,
        summary: reason.to_string(),
    };
    if let Err(e) = logger.insert_log(&log_entry) {
        error!("failed to write Skipped log: {e}");
    }
}

/// Best-effort update of the task status in YAML.
///
/// Does not propagate errors -- this is a best-effort operation as specified
/// in the execution flow.
fn restore_task_status(
    config_manager: &ConfigManager,
    task: &intern_core::Task,
    status: TaskStatus,
) {
    let mut updated = task.clone();
    updated.status = status;
    updated.updated_at = Utc::now();
    if let Err(e) = config_manager.save_task(&updated) {
        warn!(
            task_id = %task.id,
            status = %status,
            "failed to update task status: {e}"
        );
    }
}

/// Resolve a command name to an absolute path.
///
/// If the command is already an absolute path or contains a path separator,
/// return it as-is. Otherwise, search well-known locations where tools like
/// `claude` are typically installed. This is necessary because launchd runs
/// processes with a minimal PATH that excludes user-local directories.
fn resolve_executable(cmd: &str) -> String {
    use std::path::PathBuf;

    // Already absolute or contains a path component -- use as-is.
    if cmd.starts_with('/') || cmd.contains('/') {
        return cmd.to_string();
    }

    // Well-known directories where CLI tools are installed on macOS.
    let mut candidates: Vec<PathBuf> = Vec::new();

    // Try HOME env var first, then fall back to /Users/<user> on macOS.
    let home_dir = std::env::var("HOME").map(PathBuf::from).or_else(|_| {
        // launchd may not set HOME; fall back to passwd entry via id -un.
        std::process::Command::new("/usr/bin/id")
            .arg("-un")
            .output()
            .ok()
            .and_then(|o| {
                let user = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if user.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(format!("/Users/{user}")))
                }
            })
            .ok_or(())
    });

    if let Ok(home) = home_dir {
        candidates.push(home.join(".local/bin").join(cmd));
        candidates.push(home.join(".cargo/bin").join(cmd));
        candidates.push(home.join(".nvm/current/bin").join(cmd));
    }
    candidates.push(PathBuf::from("/usr/local/bin").join(cmd));
    candidates.push(PathBuf::from("/opt/homebrew/bin").join(cmd));

    for candidate in &candidates {
        if candidate.is_file() {
            info!(cmd = %cmd, resolved = %candidate.display(), "resolved executable to absolute path");
            return candidate.to_string_lossy().into_owned();
        }
    }

    // Fallback: use `which` (may succeed if PATH is adequate).
    if let Ok(output) = std::process::Command::new("/usr/bin/which")
        .arg(cmd)
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                info!(cmd = %cmd, resolved = %path, "resolved executable via which");
                return path;
            }
        }
    }

    warn!(cmd = %cmd, "could not resolve executable to absolute path, using bare name");
    cmd.to_string()
}

/// Remove a lock file from disk.
fn cleanup_lock_file(path: &Path) {
    if let Err(e) = std::fs::remove_file(path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            warn!("failed to remove lock file {}: {e}", path.display());
        }
    }
}

/// Acquire a concurrency slot using a directory-based counting semaphore.
///
/// Creates a lock file at `lock_file_path` and checks that the number of
/// existing `.lock` files in `locks_dir` does not exceed `max_concurrent`.
///
/// Retries for up to `timeout` with short sleeps between attempts.
///
/// Returns the open `File` handle (caller must keep it alive to hold the slot)
/// or an error string explaining why the slot could not be acquired.
fn acquire_concurrency_slot(
    locks_dir: &Path,
    lock_file_path: &Path,
    max_concurrent: u32,
    timeout: Duration,
) -> std::result::Result<std::fs::File, String> {
    use std::io::Write;

    let deadline = std::time::Instant::now() + timeout;
    let retry_interval = Duration::from_millis(500);

    loop {
        // Count existing .lock files (each represents a running task).
        let current_count = count_lock_files(locks_dir);

        if current_count < max_concurrent {
            // Try to create our lock file.
            match std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(lock_file_path)
            {
                Ok(mut file) => {
                    // Write our PID for debugging.
                    let _ = write!(file, "{}", std::process::id());
                    let _ = file.sync_all();
                    return Ok(file);
                }
                Err(e) => {
                    return Err(format!(
                        "failed to create lock file {}: {e}",
                        lock_file_path.display()
                    ));
                }
            }
        }

        if std::time::Instant::now() >= deadline {
            return Err(format!(
                "Concurrency limit reached ({max_concurrent} tasks already running), \
                 timed out after {}s",
                timeout.as_secs()
            ));
        }

        std::thread::sleep(retry_interval);
    }
}

/// Count the number of `.lock` files in the given directory.
fn count_lock_files(dir: &Path) -> u32 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };

    #[allow(clippy::cast_possible_truncation)]
    let count = entries
        .filter_map(std::result::Result::ok)
        .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("lock"))
        .count() as u32;

    count
}

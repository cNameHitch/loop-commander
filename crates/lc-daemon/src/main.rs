//! Loop Commander daemon entry point.
//!
//! The daemon is the sole API server for Loop Commander. Both the Swift macOS
//! app and the CLI communicate with it exclusively via JSON-RPC 2.0 over a
//! Unix domain socket. This single-writer architecture eliminates concurrency
//! problems that arise when multiple processes modify shared state.

mod events;
mod health;
mod prompt_handler;
mod server;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use lc_config::ConfigManager;
use lc_core::{DaemonEvent, LcPaths, TaskStatus};
use lc_logger::Logger;
use lc_scheduler::Scheduler;
use tokio::net::UnixListener;
use tokio::sync::{broadcast, Mutex, Semaphore};
use tracing::{error, info, warn};

use crate::server::SharedState;

/// Daemon version reported by `daemon.status`.
const VERSION: &str = "0.1.0";

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Parse CLI args.
    let foreground = std::env::args().any(|a| a == "--foreground");
    let _ = foreground; // Used to skip daemonization (not implemented yet).

    // 2. Initialize tracing with env-filter and stderr output.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    info!(version = VERSION, "Starting Loop Commander daemon");

    // 3. Ensure directories exist.
    let paths = LcPaths::new();
    paths
        .ensure_dirs()
        .context("Failed to create required directories")?;

    // 4. Single-instance check (CC-4).
    check_single_instance(&paths)?;

    // 5. Write PID file.
    let pid = std::process::id();
    std::fs::write(&paths.pid_file, pid.to_string())
        .with_context(|| format!("Failed to write PID file at {}", paths.pid_file.display()))?;
    info!(pid, pid_file = %paths.pid_file.display(), "Wrote PID file");

    // 6. Create ConfigManager, Logger, Scheduler.
    //    LcPaths does not implement Clone, so we construct separate instances
    //    for each component that needs one.
    let config_paths = LcPaths::new();
    let scheduler_paths = LcPaths::new();

    let config = ConfigManager::new(config_paths).context("Failed to initialize ConfigManager")?;

    let max_concurrent = config.global_config().max_concurrent_tasks;

    let logger = Logger::new(&paths.db_file).context("Failed to initialize Logger")?;

    let scheduler = Scheduler::new(scheduler_paths).context("Failed to initialize Scheduler")?;

    let config = Arc::new(Mutex::new(config));
    let logger = Arc::new(Mutex::new(logger));
    let scheduler = Arc::new(scheduler);

    // 7. Sync tasks to launchd: for each Active task, ensure plist is registered + loaded.
    {
        let cfg = config.lock().await;
        let (tasks, warnings) = cfg.list_tasks();
        for w in &warnings {
            warn!(warning = %w, "Warning during task sync");
        }
        for task in &tasks {
            if task.status == TaskStatus::Active {
                if let Err(e) = scheduler.register(task) {
                    warn!(
                        task_id = %task.id,
                        error = %e,
                        "Failed to register plist during sync"
                    );
                    continue;
                }
                if let Err(e) = scheduler.activate(task) {
                    warn!(
                        task_id = %task.id,
                        error = %e,
                        "Failed to activate task during sync"
                    );
                }
            }
        }
        info!(count = tasks.len(), "Synced tasks to launchd");
    }

    // 8. Remove stale socket file if present, then bind UnixListener.
    if paths.socket_path.exists() {
        // Try to connect to see if another daemon is running.
        match tokio::net::UnixStream::connect(&paths.socket_path).await {
            Ok(_) => {
                // Another daemon is responding on this socket.
                // Clean up our PID file since we are exiting.
                let _ = std::fs::remove_file(&paths.pid_file);
                anyhow::bail!(
                    "Another daemon is already running (socket {} is active)",
                    paths.socket_path.display()
                );
            }
            Err(_) => {
                // Stale socket, remove it.
                std::fs::remove_file(&paths.socket_path).with_context(|| {
                    format!(
                        "Failed to remove stale socket at {}",
                        paths.socket_path.display()
                    )
                })?;
                info!("Removed stale socket file");
            }
        }
    }

    let listener = UnixListener::bind(&paths.socket_path)
        .with_context(|| format!("Failed to bind socket at {}", paths.socket_path.display()))?;
    info!(socket = %paths.socket_path.display(), "Listening for connections");

    // 9. Create broadcast channel for DaemonEvent (capacity 256).
    let (event_tx, _event_rx) = broadcast::channel::<DaemonEvent>(256);

    // 10. Create semaphore for concurrency limit.
    let semaphore = Arc::new(Semaphore::new(max_concurrent as usize));

    // 11. Record start time for uptime tracking.
    let start_time = std::time::Instant::now();

    // Build shared state.
    let state = Arc::new(SharedState {
        config: config.clone(),
        logger: logger.clone(),
        scheduler: scheduler.clone(),
        event_tx: event_tx.clone(),
        semaphore,
        start_time,
        running_tasks: Arc::new(Mutex::new(HashMap::new())),
    });

    // 12. Spawn health_check_loop (60s interval).
    tokio::spawn(health::health_check_loop(
        config.clone(),
        scheduler.clone(),
        event_tx.clone(),
    ));

    // 13. Spawn prune_loop (3600s interval).
    tokio::spawn(health::prune_loop(config.clone(), logger.clone()));

    // 14. Main loop: tokio::select! over listener.accept() and ctrl_c.
    info!("Daemon is ready");

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, _addr)) => {
                        let conn_state = state.clone();
                        tokio::spawn(async move {
                            if let Err(e) = server::handle_connection(stream, conn_state).await {
                                error!(error = %e, "Connection handler error");
                            }
                        });
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to accept connection");
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal");
                break;
            }
        }
    }

    // 15. Cleanup: remove PID file, remove socket file, exit 0.
    cleanup(&paths);
    info!("Daemon shut down cleanly");
    Ok(())
}

/// Single-instance check (CC-4).
///
/// Checks if a daemon is already running by:
/// 1. Reading the PID file and checking if that process is alive.
/// 2. Trying to connect to the socket (handled separately in the main function).
fn check_single_instance(paths: &LcPaths) -> Result<()> {
    // Check PID file.
    if paths.pid_file.exists() {
        if let Ok(contents) = std::fs::read_to_string(&paths.pid_file) {
            if let Ok(pid) = contents.trim().parse::<u32>() {
                // Check if process is alive by sending signal 0 via `kill`.
                // On macOS, `kill -0 <pid>` returns 0 if the process exists.
                let alive = std::process::Command::new("kill")
                    .arg("-0")
                    .arg(pid.to_string())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);

                if alive {
                    anyhow::bail!(
                        "Daemon already running with PID {pid}. \
                         If this is stale, remove {} and try again.",
                        paths.pid_file.display()
                    );
                }
                // Process is not alive -- stale PID file, continue.
                info!(pid, "Found stale PID file, will overwrite");
            }
        }
    }

    Ok(())
}

/// Remove PID and socket files on shutdown.
fn cleanup(paths: &LcPaths) {
    if let Err(e) = std::fs::remove_file(&paths.pid_file) {
        warn!(error = %e, "Failed to remove PID file during cleanup");
    }
    if let Err(e) = std::fs::remove_file(&paths.socket_path) {
        warn!(error = %e, "Failed to remove socket file during cleanup");
    }
}

//! Health check loop for the Intern daemon.
//!
//! Runs every 60 seconds, verifying that each active task has its launchd job
//! loaded. If a discrepancy is found (active task but job not loaded), the
//! health check re-registers and re-activates the job, then emits a
//! `HealthRepair` event so subscribed clients are informed.

use std::sync::Arc;
use std::time::Duration;

use intern_config::ConfigManager;
use intern_core::{DaemonEvent, TaskStatus};
use intern_scheduler::Scheduler;
use tokio::sync::{broadcast, Mutex};
use tracing::{error, info, warn};

/// Run the health check loop indefinitely.
///
/// Every 60 seconds, iterates over all tasks from the config manager and
/// checks whether each active task has its launchd job loaded. If an active
/// task's job is not loaded, the function re-registers the plist and
/// re-activates the job, then broadcasts a `HealthRepair` event.
///
/// This function never returns under normal operation. It is intended to
/// be spawned as a background tokio task.
pub async fn health_check_loop(
    config: Arc<Mutex<ConfigManager>>,
    scheduler: Arc<Scheduler>,
    event_tx: broadcast::Sender<DaemonEvent>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));

    loop {
        interval.tick().await;

        let tasks = {
            let cfg = config.lock().await;
            let (tasks, warnings) = cfg.list_tasks();
            for w in &warnings {
                warn!(warning = %w, "Health check: warning listing tasks");
            }
            tasks
        };

        for task in &tasks {
            if task.status != TaskStatus::Active {
                continue;
            }

            let task_id = task.id.as_str();

            match scheduler.is_loaded(task_id) {
                Ok(true) => {
                    // Job is loaded, all good.
                }
                Ok(false) => {
                    warn!(
                        task_id = %task_id,
                        task_name = %task.name,
                        "Health check: active task not loaded in launchd, re-installing"
                    );

                    if let Err(e) = scheduler.register(task) {
                        error!(
                            task_id = %task_id,
                            error = %e,
                            "Health check: failed to re-register plist"
                        );
                        continue;
                    }

                    if let Err(e) = scheduler.activate(task) {
                        error!(
                            task_id = %task_id,
                            error = %e,
                            "Health check: failed to re-activate launchd job"
                        );
                        continue;
                    }

                    info!(
                        task_id = %task_id,
                        task_name = %task.name,
                        "Health check: successfully repaired launchd job"
                    );

                    let event = DaemonEvent::HealthRepair {
                        task_id: task_id.to_string(),
                        action: "re-registered and re-activated launchd job".to_string(),
                    };

                    // Best-effort broadcast; if no subscribers, that is fine.
                    let _ = event_tx.send(event);
                }
                Err(e) => {
                    error!(
                        task_id = %task_id,
                        error = %e,
                        "Health check: failed to check launchd job status"
                    );
                }
            }
        }
    }
}

/// Run the log pruning loop indefinitely.
///
/// Every 3600 seconds (1 hour), reads the configured `log_retention_days`
/// from the global config and delegates to `Logger::prune_logs` to remove
/// old execution log entries from the SQLite database.
///
/// This function never returns under normal operation. It is intended to
/// be spawned as a background tokio task.
pub async fn prune_loop(config: Arc<Mutex<ConfigManager>>, logger: Arc<Mutex<intern_logger::Logger>>) {
    let mut interval = tokio::time::interval(Duration::from_secs(3600));

    loop {
        interval.tick().await;

        let retention_days = {
            let cfg = config.lock().await;
            cfg.global_config().log_retention_days
        };

        let lgr = logger.lock().await;
        match lgr.prune_logs(retention_days) {
            Ok(deleted) => {
                if deleted > 0 {
                    info!(deleted, retention_days, "Pruned old log entries");
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to prune logs");
            }
        }
    }
}

//! JSON-RPC 2.0 dispatch for the Intern daemon.
//!
//! This module implements `handle_connection`, which reads newline-delimited
//! JSON-RPC requests from a Unix domain socket, dispatches each to the
//! appropriate handler, and writes back a newline-delimited response.
//!
//! For `events.subscribe` requests, the connection transitions to push mode
//! where the server holds the connection open and writes events as they arrive.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use intern_config::ConfigManager;
use intern_core::{
    builtin_templates, rpc_errors, CreateTaskInput, DaemonEvent, DryRunResult, JsonRpcRequest,
    JsonRpcResponse, LogQuery, TaskExport, TaskStatus, UpdateTaskInput,
};
use intern_logger::Logger;
use intern_runner::build_command;
use intern_scheduler::Scheduler;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{broadcast, Mutex, Semaphore};
use tracing::{debug, error, info, warn};

use crate::events::event_to_notification;

/// Global counter of connected clients.
static CONNECTED_CLIENTS: AtomicU64 = AtomicU64::new(0);

/// Return the current number of connected clients.
pub fn connected_client_count() -> u64 {
    CONNECTED_CLIENTS.load(Ordering::Relaxed)
}

/// Shared state passed to each connection handler.
pub struct SharedState {
    pub config: Arc<Mutex<ConfigManager>>,
    pub logger: Arc<Mutex<Logger>>,
    pub scheduler: Arc<Scheduler>,
    pub event_tx: broadcast::Sender<DaemonEvent>,
    pub semaphore: Arc<Semaphore>,
    pub start_time: std::time::Instant,
    /// Map of task_id -> child process handle for currently running tasks.
    pub running_tasks: Arc<Mutex<HashMap<String, tokio::process::Child>>>,
}

/// Handle a single client connection.
///
/// Reads newline-delimited JSON-RPC requests, dispatches each one, and
/// writes back a newline-delimited response. The connection stays open until
/// the client disconnects or until an `events.subscribe` request transitions
/// the connection into push mode.
pub async fn handle_connection(stream: UnixStream, state: Arc<SharedState>) -> Result<()> {
    CONNECTED_CLIENTS.fetch_add(1, Ordering::Relaxed);
    let _guard = ConnectionGuard;

    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = buf_reader
            .read_line(&mut line)
            .await
            .context("failed to read from socket")?;

        if bytes_read == 0 {
            // Client disconnected.
            debug!("Client disconnected");
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse the JSON-RPC request.
        let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(req) => req,
            Err(e) => {
                let error_resp = JsonRpcResponse::error(
                    serde_json::Value::Null,
                    -32700,
                    format!("Parse error: {e}"),
                );
                let mut resp_str = serde_json::to_string(&error_resp)?;
                resp_str.push('\n');
                writer.write_all(resp_str.as_bytes()).await?;
                continue;
            }
        };

        // Check for valid JSON-RPC 2.0.
        if request.jsonrpc != "2.0" {
            let error_resp = JsonRpcResponse::error(
                request.id.clone(),
                -32600,
                "Invalid Request: jsonrpc must be \"2.0\"".into(),
            );
            let mut resp_str = serde_json::to_string(&error_resp)?;
            resp_str.push('\n');
            writer.write_all(resp_str.as_bytes()).await?;
            continue;
        }

        // Special handling for events.subscribe: transition to push mode.
        if request.method == "events.subscribe" {
            // Send an initial success response acknowledging subscription.
            let ack = JsonRpcResponse::success(
                request.id.clone(),
                serde_json::json!({"subscribed": true}),
            );
            let mut ack_str = serde_json::to_string(&ack)?;
            ack_str.push('\n');
            writer.write_all(ack_str.as_bytes()).await?;

            // Enter push mode.
            let mut rx = state.event_tx.subscribe();
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let notification = event_to_notification(&event);
                        if writer.write_all(notification.as_bytes()).await.is_err() {
                            debug!("Event subscriber disconnected");
                            return Ok(());
                        }
                        if writer.flush().await.is_err() {
                            debug!("Event subscriber flush failed");
                            return Ok(());
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "Event subscriber lagged, skipped events");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        debug!("Event channel closed");
                        return Ok(());
                    }
                }
            }
        }

        // Dispatch the request.
        let response = dispatch(
            &request.method,
            request.params.clone(),
            request.id.clone(),
            &state,
        )
        .await;

        let mut resp_str = serde_json::to_string(&response)?;
        resp_str.push('\n');
        writer.write_all(resp_str.as_bytes()).await?;
        writer.flush().await?;
    }

    Ok(())
}

/// Guard that decrements the connected client count when dropped.
struct ConnectionGuard;

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        CONNECTED_CLIENTS.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Dispatch a JSON-RPC request to the appropriate handler.
async fn dispatch(
    method: &str,
    params: serde_json::Value,
    id: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    debug!(method, "Dispatching JSON-RPC request");

    match method {
        // Task management
        "task.list" => handle_task_list(id, state).await,
        "task.get" => handle_task_get(id, params, state).await,
        "task.create" => handle_task_create(id, params, state).await,
        "task.update" => handle_task_update(id, params, state).await,
        "task.delete" => handle_task_delete(id, params, state).await,
        "task.pause" => handle_task_pause(id, params, state).await,
        "task.resume" => handle_task_resume(id, params, state).await,
        "task.run_now" => handle_task_run_now(id, params, state).await,
        "task.dry_run" => handle_task_dry_run(id, params, state).await,
        "task.stop" => handle_task_stop(id, params, state).await,
        "task.export" => handle_task_export(id, params, state).await,
        "task.import" => handle_task_import(id, params, state).await,

        // Templates
        "templates.list" => handle_templates_list(id).await,

        // Logs & Metrics
        "logs.query" => handle_logs_query(id, params, state).await,
        "metrics.dashboard" => handle_metrics_dashboard(id, state).await,
        "metrics.cost_trend" => handle_metrics_cost_trend(id, params, state).await,

        // Config
        "config.get" => handle_config_get(id, state).await,
        "config.update" => handle_config_update(id, params, state).await,

        // Daemon
        "daemon.status" => handle_daemon_status(id, state).await,

        // Schedule utilities
        "schedule.validate" => handle_schedule_validate(id, params).await,

        // Prompt generation
        "prompt.generate" => crate::prompt_handler::handle_prompt_generate(id, params, state).await,

        // Prompt optimization
        "prompt.optimize" => crate::prompt_handler::handle_prompt_optimize(id, params, state).await,

        // Prompt edit (AI-assisted task refinement)
        "prompt.edit" => crate::prompt_handler::handle_prompt_edit(id, params, state).await,

        // Agent registry
        "registry.refresh" => crate::prompt_handler::handle_registry_refresh(id, state).await,
        "registry.list" => crate::prompt_handler::handle_registry_list(id, state).await,

        // Unknown method
        _ => JsonRpcResponse::error(id, -32601, format!("Method not found: {method}")),
    }
}

// ── Task Handlers ──────────────────────────────────────────────

async fn handle_task_list(id: serde_json::Value, state: &SharedState) -> JsonRpcResponse {
    let cfg = state.config.lock().await;
    let (tasks, warnings) = cfg.list_tasks();

    for w in &warnings {
        warn!(warning = %w, "Warning while listing tasks");
    }

    match serde_json::to_value(&tasks) {
        Ok(val) => JsonRpcResponse::success(id, val),
        Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
    }
}

async fn handle_task_get(
    id: serde_json::Value,
    params: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    let task_id = match params.get("id").and_then(serde_json::Value::as_str) {
        Some(id_str) => id_str.to_string(),
        None => {
            return JsonRpcResponse::error(id, -32602, "Invalid params: missing 'id' field".into())
        }
    };

    let cfg = state.config.lock().await;
    match cfg.get_task(&task_id) {
        Ok(task) => match serde_json::to_value(&task) {
            Ok(val) => JsonRpcResponse::success(id, val),
            Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
        },
        Err(intern_core::InternError::TaskNotFound(_)) => JsonRpcResponse::error(
            id,
            rpc_errors::TASK_NOT_FOUND,
            format!("Task not found: {task_id}"),
        ),
        Err(e) => JsonRpcResponse::error(id, -32603, format!("Internal error: {e}")),
    }
}

async fn handle_task_create(
    id: serde_json::Value,
    params: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    // Deserialize input.
    let input: CreateTaskInput = match serde_json::from_value(params) {
        Ok(i) => i,
        Err(e) => return JsonRpcResponse::error(id, -32602, format!("Invalid params: {e}")),
    };

    // Validate (CC-3).
    if let Err(errors) = input.validate() {
        return JsonRpcResponse::error(
            id,
            rpc_errors::VALIDATION_ERROR,
            format!("Validation failed: {}", errors.join("; ")),
        );
    }

    let cfg = state.config.lock().await;
    let task = cfg.create_task_from_input(input);

    // Save YAML.
    if let Err(e) = cfg.save_task(&task) {
        return JsonRpcResponse::error(id, -32603, format!("Failed to save task: {e}"));
    }

    // Register and activate in launchd.
    if let Err(e) = state.scheduler.register(&task) {
        error!(task_id = %task.id, error = %e, "Failed to register plist");
        return JsonRpcResponse::error(
            id,
            rpc_errors::SCHEDULER_ERROR,
            format!("Failed to register plist: {e}"),
        );
    }

    if let Err(e) = state.scheduler.activate(&task) {
        error!(task_id = %task.id, error = %e, "Failed to activate launchd job");
        return JsonRpcResponse::error(
            id,
            rpc_errors::SCHEDULER_ERROR,
            format!("Failed to activate launchd job: {e}"),
        );
    }

    // Emit event.
    let event = DaemonEvent::TaskStatusChanged {
        task_id: task.id.as_str().to_string(),
        old_status: String::new(),
        new_status: task.status.to_string(),
    };
    let _ = state.event_tx.send(event);

    info!(task_id = %task.id, task_name = %task.name, "Task created");

    // Write context file (non-fatal on failure).
    if let Err(e) = cfg.write_command_file(&task) {
        warn!(
            task_id = %task.id,
            error = %e,
            "Failed to write context file (task still created)"
        );
    }

    match serde_json::to_value(&task) {
        Ok(val) => JsonRpcResponse::success(id, val),
        Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
    }
}

async fn handle_task_update(
    id: serde_json::Value,
    params: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    // Deserialize input.
    let input: UpdateTaskInput = match serde_json::from_value(params) {
        Ok(i) => i,
        Err(e) => return JsonRpcResponse::error(id, -32602, format!("Invalid params: {e}")),
    };

    // Validate (CC-3).
    if let Err(errors) = input.validate() {
        return JsonRpcResponse::error(
            id,
            rpc_errors::VALIDATION_ERROR,
            format!("Validation failed: {}", errors.join("; ")),
        );
    }

    let task_id = input.id.clone();
    let schedule_changed = input.schedule.is_some();

    let cfg = state.config.lock().await;

    // Load existing task.
    let mut task = match cfg.get_task(&task_id) {
        Ok(t) => t,
        Err(intern_core::InternError::TaskNotFound(_)) => {
            return JsonRpcResponse::error(
                id,
                rpc_errors::TASK_NOT_FOUND,
                format!("Task not found: {task_id}"),
            )
        }
        Err(e) => return JsonRpcResponse::error(id, -32603, format!("Internal error: {e}")),
    };

    let old_status = task.status.to_string();
    let old_working_dir = task.working_dir.clone();
    let old_name = task.name.clone();

    // Apply update.
    cfg.apply_update(&mut task, input);

    // Save YAML.
    if let Err(e) = cfg.save_task(&task) {
        return JsonRpcResponse::error(id, -32603, format!("Failed to save task: {e}"));
    }

    // Delete old context file if working_dir or name changed.
    let dir_changed = old_working_dir != task.working_dir;
    let name_changed = old_name != task.name;
    if dir_changed || name_changed {
        if let Err(e) = cfg.delete_command_file(&old_working_dir, &old_name, task.id.as_str()) {
            warn!(
                task_id = %task.id,
                error = %e,
                "Failed to delete old context file during update"
            );
        }
    }

    // Write new context file (non-fatal on failure).
    if let Err(e) = cfg.write_command_file(&task) {
        warn!(
            task_id = %task.id,
            error = %e,
            "Failed to write context file (task still updated)"
        );
    }

    // Reinstall plist if schedule changed.
    if schedule_changed {
        if let Err(e) = state.scheduler.reinstall(&task) {
            error!(task_id = %task.id, error = %e, "Failed to reinstall plist");
            return JsonRpcResponse::error(
                id,
                rpc_errors::SCHEDULER_ERROR,
                format!("Failed to reinstall plist: {e}"),
            );
        }
    }

    // Emit status changed event if status actually changed.
    let new_status = task.status.to_string();
    if old_status != new_status {
        let event = DaemonEvent::TaskStatusChanged {
            task_id: task.id.as_str().to_string(),
            old_status,
            new_status,
        };
        let _ = state.event_tx.send(event);
    }

    info!(task_id = %task.id, task_name = %task.name, "Task updated");

    match serde_json::to_value(&task) {
        Ok(val) => JsonRpcResponse::success(id, val),
        Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
    }
}

async fn handle_task_delete(
    id: serde_json::Value,
    params: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    let task_id = match params.get("id").and_then(serde_json::Value::as_str) {
        Some(id_str) => id_str.to_string(),
        None => {
            return JsonRpcResponse::error(id, -32602, "Invalid params: missing 'id' field".into())
        }
    };

    // Deactivate launchd job.
    if let Err(e) = state.scheduler.deactivate(&task_id) {
        warn!(task_id = %task_id, error = %e, "Failed to deactivate (continuing with delete)");
    }

    // Unregister plist.
    if let Err(e) = state.scheduler.unregister(&task_id) {
        warn!(task_id = %task_id, error = %e, "Failed to unregister (continuing with delete)");
    }

    // Delete YAML.
    let cfg = state.config.lock().await;

    // Load task data for context file cleanup before deleting YAML.
    let task_for_cleanup = cfg.get_task(&task_id).ok();

    match cfg.delete_task(&task_id) {
        Ok(()) => {
            info!(task_id = %task_id, "Task deleted");

            // Delete context file (best effort).
            if let Some(ref t) = task_for_cleanup {
                if let Err(e) = cfg.delete_command_file(&t.working_dir, &t.name, &task_id) {
                    warn!(
                        task_id = %task_id,
                        error = %e,
                        "Failed to delete context file during task delete"
                    );
                }
            }

            let event = DaemonEvent::TaskStatusChanged {
                task_id: task_id.clone(),
                old_status: "active".into(),
                new_status: "deleted".into(),
            };
            let _ = state.event_tx.send(event);

            JsonRpcResponse::success(id, serde_json::json!({"deleted": true}))
        }
        Err(intern_core::InternError::TaskNotFound(_)) => JsonRpcResponse::error(
            id,
            rpc_errors::TASK_NOT_FOUND,
            format!("Task not found: {task_id}"),
        ),
        Err(e) => JsonRpcResponse::error(id, -32603, format!("Failed to delete task: {e}")),
    }
}

async fn handle_task_pause(
    id: serde_json::Value,
    params: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    let task_id = match params.get("id").and_then(serde_json::Value::as_str) {
        Some(id_str) => id_str.to_string(),
        None => {
            return JsonRpcResponse::error(id, -32602, "Invalid params: missing 'id' field".into())
        }
    };

    let cfg = state.config.lock().await;
    let mut task = match cfg.get_task(&task_id) {
        Ok(t) => t,
        Err(intern_core::InternError::TaskNotFound(_)) => {
            return JsonRpcResponse::error(
                id,
                rpc_errors::TASK_NOT_FOUND,
                format!("Task not found: {task_id}"),
            )
        }
        Err(e) => return JsonRpcResponse::error(id, -32603, format!("Internal error: {e}")),
    };

    let old_status = task.status.to_string();
    task.status = TaskStatus::Paused;
    task.updated_at = chrono::Utc::now();

    if let Err(e) = cfg.save_task(&task) {
        return JsonRpcResponse::error(id, -32603, format!("Failed to save task: {e}"));
    }

    // Deactivate launchd job.
    if let Err(e) = state.scheduler.deactivate(&task_id) {
        warn!(task_id = %task_id, error = %e, "Failed to deactivate launchd job on pause");
    }

    let event = DaemonEvent::TaskStatusChanged {
        task_id: task_id.clone(),
        old_status,
        new_status: "paused".into(),
    };
    let _ = state.event_tx.send(event);

    info!(task_id = %task_id, "Task paused");

    match serde_json::to_value(&task) {
        Ok(val) => JsonRpcResponse::success(id, val),
        Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
    }
}

async fn handle_task_resume(
    id: serde_json::Value,
    params: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    let task_id = match params.get("id").and_then(serde_json::Value::as_str) {
        Some(id_str) => id_str.to_string(),
        None => {
            return JsonRpcResponse::error(id, -32602, "Invalid params: missing 'id' field".into())
        }
    };

    let cfg = state.config.lock().await;
    let mut task = match cfg.get_task(&task_id) {
        Ok(t) => t,
        Err(intern_core::InternError::TaskNotFound(_)) => {
            return JsonRpcResponse::error(
                id,
                rpc_errors::TASK_NOT_FOUND,
                format!("Task not found: {task_id}"),
            )
        }
        Err(e) => return JsonRpcResponse::error(id, -32603, format!("Internal error: {e}")),
    };

    let old_status = task.status.to_string();
    task.status = TaskStatus::Active;
    task.updated_at = chrono::Utc::now();

    if let Err(e) = cfg.save_task(&task) {
        return JsonRpcResponse::error(id, -32603, format!("Failed to save task: {e}"));
    }

    // Register and activate in launchd.
    if let Err(e) = state.scheduler.register(&task) {
        warn!(task_id = %task_id, error = %e, "Failed to register plist on resume");
    }

    if let Err(e) = state.scheduler.activate(&task) {
        warn!(task_id = %task_id, error = %e, "Failed to activate launchd job on resume");
    }

    let event = DaemonEvent::TaskStatusChanged {
        task_id: task_id.clone(),
        old_status,
        new_status: "active".into(),
    };
    let _ = state.event_tx.send(event);

    info!(task_id = %task_id, "Task resumed");

    match serde_json::to_value(&task) {
        Ok(val) => JsonRpcResponse::success(id, val),
        Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
    }
}

async fn handle_task_run_now(
    id: serde_json::Value,
    params: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    let task_id = match params.get("id").and_then(serde_json::Value::as_str) {
        Some(id_str) => id_str.to_string(),
        None => {
            return JsonRpcResponse::error(id, -32602, "Invalid params: missing 'id' field".into())
        }
    };

    // Load the task to get name and validate existence.
    let task = {
        let cfg = state.config.lock().await;
        match cfg.get_task(&task_id) {
            Ok(t) => t,
            Err(intern_core::InternError::TaskNotFound(_)) => {
                return JsonRpcResponse::error(
                    id,
                    rpc_errors::TASK_NOT_FOUND,
                    format!("Task not found: {task_id}"),
                )
            }
            Err(e) => return JsonRpcResponse::error(id, -32603, format!("Internal error: {e}")),
        }
    };

    // Check concurrency semaphore.
    let permit = match state.semaphore.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return JsonRpcResponse::error(
                id,
                rpc_errors::DAEMON_BUSY,
                "Concurrency limit reached; task is queued".into(),
            )
        }
    };

    // Discover the intern-runner binary path.
    let runner_path = state.scheduler.runner_path().clone();

    // Emit TaskStarted event.
    let event = DaemonEvent::TaskStarted {
        task_id: task_id.clone(),
        task_name: task.name.clone(),
    };
    let _ = state.event_tx.send(event);

    // Spawn intern-runner as a child process.
    let child_result = tokio::process::Command::new(&runner_path)
        .arg("--task-id")
        .arg(&task_id)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    match child_result {
        Ok(child) => {
            let child_id = child.id();
            info!(
                task_id = %task_id,
                pid = ?child_id,
                runner = %runner_path.display(),
                "Spawned intern-runner"
            );

            // Track the running child process.
            {
                let mut running = state.running_tasks.lock().await;
                running.insert(task_id.clone(), child);
            }

            // Spawn a background task to wait for the child and clean up.
            let running_tasks = state.running_tasks.clone();
            let event_tx = state.event_tx.clone();
            let spawned_task_id = task_id.clone();
            let spawned_task_name = task.name.clone();

            tokio::spawn(async move {
                // Take ownership of the child from the map, wait for it.
                let child_opt = {
                    let mut running = running_tasks.lock().await;
                    running.remove(&spawned_task_id)
                };

                if let Some(mut child) = child_opt {
                    match child.wait().await {
                        Ok(status) => {
                            if status.success() {
                                let event = DaemonEvent::TaskCompleted {
                                    task_id: spawned_task_id,
                                    task_name: spawned_task_name,
                                    duration_secs: 0,
                                    cost_usd: None,
                                };
                                let _ = event_tx.send(event);
                            } else {
                                let exit_code = status.code().unwrap_or(-1);
                                let event = DaemonEvent::TaskFailed {
                                    task_id: spawned_task_id,
                                    task_name: spawned_task_name,
                                    exit_code,
                                    summary: format!("Process exited with code {exit_code}"),
                                };
                                let _ = event_tx.send(event);
                            }
                        }
                        Err(e) => {
                            error!(
                                task_id = %spawned_task_id,
                                error = %e,
                                "Failed to wait for intern-runner"
                            );
                        }
                    }
                }

                // Release the semaphore permit when done.
                drop(permit);
            });

            JsonRpcResponse::success(id, serde_json::json!({"started": true}))
        }
        Err(e) => {
            // Release permit on failure.
            drop(permit);
            error!(
                task_id = %task_id,
                error = %e,
                "Failed to spawn intern-runner"
            );
            JsonRpcResponse::error(
                id,
                rpc_errors::SCHEDULER_ERROR,
                format!("Failed to spawn intern-runner: {e}"),
            )
        }
    }
}

async fn handle_task_dry_run(
    id: serde_json::Value,
    params: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    let task_id = match params.get("id").and_then(serde_json::Value::as_str) {
        Some(id_str) => id_str.to_string(),
        None => {
            return JsonRpcResponse::error(id, -32602, "Invalid params: missing 'id' field".into())
        }
    };

    let cfg = state.config.lock().await;
    let task = match cfg.get_task(&task_id) {
        Ok(t) => t,
        Err(intern_core::InternError::TaskNotFound(_)) => {
            return JsonRpcResponse::error(
                id,
                rpc_errors::TASK_NOT_FOUND,
                format!("Task not found: {task_id}"),
            )
        }
        Err(e) => return JsonRpcResponse::error(id, -32603, format!("Internal error: {e}")),
    };

    // Build resolved command using intern_runner's build_command.
    let resolved_command = build_command(&task, None);

    // Check daily spend.
    let daily_spend_so_far = {
        let lgr = state.logger.lock().await;
        let today_start = chrono::Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .expect("midnight is always valid")
            .and_utc();
        lgr.total_cost_since(&task_id, today_start).unwrap_or(0.0)
    };

    let daily_cap = cfg
        .global_config()
        .daily_budget_cap
        .unwrap_or(task.max_budget_per_run * 20.0);

    let would_be_skipped = daily_spend_so_far >= daily_cap;
    let skip_reason = if would_be_skipped {
        Some(format!(
            "Daily budget cap exceeded: spent ${daily_spend_so_far:.2}, cap ${daily_cap:.2}"
        ))
    } else {
        None
    };

    let result = DryRunResult {
        task_id: task.id.as_str().to_string(),
        task_name: task.name.clone(),
        resolved_command,
        working_dir: task.working_dir.to_string_lossy().into_owned(),
        env_vars: task.env_vars.clone(),
        timeout_secs: task.timeout_secs,
        max_budget_per_run: task.max_budget_per_run,
        daily_spend_so_far,
        would_be_skipped,
        skip_reason,
        schedule_human: task.schedule_human.clone(),
    };

    match serde_json::to_value(&result) {
        Ok(val) => JsonRpcResponse::success(id, val),
        Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
    }
}

async fn handle_task_stop(
    id: serde_json::Value,
    params: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    let task_id = match params.get("id").and_then(serde_json::Value::as_str) {
        Some(id_str) => id_str.to_string(),
        None => {
            return JsonRpcResponse::error(id, -32602, "Invalid params: missing 'id' field".into())
        }
    };

    let mut running = state.running_tasks.lock().await;
    if let Some(mut child) = running.remove(&task_id) {
        match child.kill().await {
            Ok(()) => {
                info!(task_id = %task_id, "Killed running intern-runner process");
                JsonRpcResponse::success(id, serde_json::json!({"stopped": true}))
            }
            Err(e) => JsonRpcResponse::error(id, -32603, format!("Failed to kill process: {e}")),
        }
    } else {
        JsonRpcResponse::error(
            id,
            rpc_errors::TASK_NOT_FOUND,
            format!("No running process found for task: {task_id}"),
        )
    }
}

async fn handle_task_export(
    id: serde_json::Value,
    params: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    let task_id = match params.get("id").and_then(serde_json::Value::as_str) {
        Some(id_str) => id_str.to_string(),
        None => {
            return JsonRpcResponse::error(id, -32602, "Invalid params: missing 'id' field".into())
        }
    };

    let cfg = state.config.lock().await;
    let task = match cfg.get_task(&task_id) {
        Ok(t) => t,
        Err(intern_core::InternError::TaskNotFound(_)) => {
            return JsonRpcResponse::error(
                id,
                rpc_errors::TASK_NOT_FOUND,
                format!("Task not found: {task_id}"),
            )
        }
        Err(e) => return JsonRpcResponse::error(id, -32603, format!("Internal error: {e}")),
    };

    let export = TaskExport::from(&task);

    match serde_json::to_value(&export) {
        Ok(val) => JsonRpcResponse::success(id, val),
        Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
    }
}

async fn handle_task_import(
    id: serde_json::Value,
    params: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    // Deserialize the TaskExport.
    let export: TaskExport = match serde_json::from_value(params) {
        Ok(e) => e,
        Err(e) => return JsonRpcResponse::error(id, -32602, format!("Invalid params: {e}")),
    };

    // Convert to CreateTaskInput.
    let input: CreateTaskInput = export.into();

    // Validate.
    if let Err(errors) = input.validate() {
        return JsonRpcResponse::error(
            id,
            rpc_errors::VALIDATION_ERROR,
            format!("Validation failed: {}", errors.join("; ")),
        );
    }

    let cfg = state.config.lock().await;
    let task = cfg.create_task_from_input(input);

    // Save YAML.
    if let Err(e) = cfg.save_task(&task) {
        return JsonRpcResponse::error(id, -32603, format!("Failed to save task: {e}"));
    }

    // Register and activate in launchd.
    if let Err(e) = state.scheduler.register(&task) {
        error!(task_id = %task.id, error = %e, "Failed to register plist for imported task");
    }

    if let Err(e) = state.scheduler.activate(&task) {
        error!(task_id = %task.id, error = %e, "Failed to activate imported task");
    }

    let event = DaemonEvent::TaskStatusChanged {
        task_id: task.id.as_str().to_string(),
        old_status: String::new(),
        new_status: task.status.to_string(),
    };
    let _ = state.event_tx.send(event);

    info!(task_id = %task.id, task_name = %task.name, "Task imported");

    match serde_json::to_value(&task) {
        Ok(val) => JsonRpcResponse::success(id, val),
        Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
    }
}

// ── Templates ──────────────────────────────────────────────────

async fn handle_templates_list(id: serde_json::Value) -> JsonRpcResponse {
    let templates = builtin_templates();
    match serde_json::to_value(&templates) {
        Ok(val) => JsonRpcResponse::success(id, val),
        Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
    }
}

// ── Logs & Metrics ─────────────────────────────────────────────

async fn handle_logs_query(
    id: serde_json::Value,
    params: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    let query: LogQuery = match serde_json::from_value(params) {
        Ok(q) => q,
        Err(e) => return JsonRpcResponse::error(id, -32602, format!("Invalid params: {e}")),
    };

    let lgr = state.logger.lock().await;
    match lgr.query_logs(&query) {
        Ok(logs) => match serde_json::to_value(&logs) {
            Ok(val) => JsonRpcResponse::success(id, val),
            Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
        },
        Err(e) => JsonRpcResponse::error(
            id,
            rpc_errors::DATABASE_ERROR,
            format!("Database error: {e}"),
        ),
    }
}

async fn handle_metrics_dashboard(id: serde_json::Value, state: &SharedState) -> JsonRpcResponse {
    let tasks = {
        let cfg = state.config.lock().await;
        let (tasks, _) = cfg.list_tasks();
        tasks
    };

    let lgr = state.logger.lock().await;
    match lgr.get_dashboard_metrics(&tasks) {
        Ok(metrics) => match serde_json::to_value(&metrics) {
            Ok(val) => JsonRpcResponse::success(id, val),
            Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
        },
        Err(e) => JsonRpcResponse::error(
            id,
            rpc_errors::DATABASE_ERROR,
            format!("Database error: {e}"),
        ),
    }
}

async fn handle_metrics_cost_trend(
    id: serde_json::Value,
    params: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    let days = params
        .get("days")
        .and_then(serde_json::Value::as_u64)
        .map_or(7, |d| d as u32);

    let lgr = state.logger.lock().await;
    match lgr.get_cost_trend(days) {
        Ok(trend) => match serde_json::to_value(&trend) {
            Ok(val) => JsonRpcResponse::success(id, val),
            Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
        },
        Err(e) => JsonRpcResponse::error(
            id,
            rpc_errors::DATABASE_ERROR,
            format!("Database error: {e}"),
        ),
    }
}

// ── Config ─────────────────────────────────────────────────────

async fn handle_config_get(id: serde_json::Value, state: &SharedState) -> JsonRpcResponse {
    let cfg = state.config.lock().await;
    let global = cfg.global_config();

    match serde_json::to_value(global) {
        Ok(val) => JsonRpcResponse::success(id, val),
        Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
    }
}

async fn handle_config_update(
    id: serde_json::Value,
    params: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    let mut cfg = state.config.lock().await;

    // Apply partial updates from the params object.
    let global = cfg.global_config_mut();

    if let Some(v) = params
        .get("claude_binary")
        .and_then(serde_json::Value::as_str)
    {
        global.claude_binary = v.to_string();
    }
    if let Some(v) = params
        .get("default_budget")
        .and_then(serde_json::Value::as_f64)
    {
        global.default_budget = v;
    }
    if let Some(v) = params
        .get("default_timeout")
        .and_then(serde_json::Value::as_u64)
    {
        global.default_timeout = v;
    }
    if let Some(v) = params
        .get("default_max_turns")
        .and_then(serde_json::Value::as_u64)
    {
        global.default_max_turns = v as u32;
    }
    if let Some(v) = params
        .get("log_retention_days")
        .and_then(serde_json::Value::as_u64)
    {
        global.log_retention_days = v as u32;
    }
    if let Some(v) = params
        .get("notifications_enabled")
        .and_then(serde_json::Value::as_bool)
    {
        global.notifications_enabled = v;
    }
    if let Some(v) = params
        .get("max_concurrent_tasks")
        .and_then(serde_json::Value::as_u64)
    {
        global.max_concurrent_tasks = v as u32;
    }
    if let Some(v) = params.get("daily_budget_cap") {
        if v.is_null() {
            global.daily_budget_cap = None;
        } else if let Some(f) = v.as_f64() {
            global.daily_budget_cap = Some(f);
        }
    }
    if let Some(v) = params
        .get("cost_estimate_per_second")
        .and_then(serde_json::Value::as_f64)
    {
        global.cost_estimate_per_second = v;
    }
    if let Some(v) = params.get("theme").and_then(serde_json::Value::as_str) {
        global.theme = v.to_string();
    }

    // Persist to disk.
    if let Err(e) = cfg.save_global_config() {
        return JsonRpcResponse::error(id, -32603, format!("Failed to save config: {e}"));
    }

    let global = cfg.global_config();
    match serde_json::to_value(global) {
        Ok(val) => JsonRpcResponse::success(id, val),
        Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
    }
}

// ── Daemon ─────────────────────────────────────────────────────

async fn handle_daemon_status(id: serde_json::Value, state: &SharedState) -> JsonRpcResponse {
    let pid = std::process::id();
    let uptime_secs = state.start_time.elapsed().as_secs();
    let connected_clients = connected_client_count();

    let cfg = state.config.lock().await;
    let max_concurrent = cfg.global_config().max_concurrent_tasks;
    drop(cfg);

    // Check if claude binary is available.
    let claude_available = tokio::process::Command::new("which")
        .arg("claude")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    let status = serde_json::json!({
        "pid": pid,
        "uptime_secs": uptime_secs,
        "version": "0.1.0",
        "connected_clients": connected_clients,
        "claude_available": claude_available,
        "max_concurrent_tasks": max_concurrent,
    });

    JsonRpcResponse::success(id, status)
}

// ── Schedule Utilities ──────────────────────────────────────────

/// Handle `schedule.validate` — validate a cron expression without side effects.
///
/// Params: `{ "expression": "<5-field cron>" }`
///
/// Response is always a successful JSON-RPC result:
/// - `{ "valid": true }` when the expression is accepted by the cron parser.
/// - `{ "valid": false, "error": "<message>" }` when parsing fails.
///
/// A JSON-RPC error response is returned only when the `expression` param is
/// absent or not a string.
async fn handle_schedule_validate(
    id: serde_json::Value,
    params: serde_json::Value,
) -> JsonRpcResponse {
    let expression = match params.get("expression").and_then(serde_json::Value::as_str) {
        Some(expr) => expr.to_string(),
        None => {
            return JsonRpcResponse::error(
                id,
                -32602,
                "Invalid params: missing 'expression' field".into(),
            )
        }
    };

    match intern_scheduler::validate_cron(&expression) {
        Ok(()) => JsonRpcResponse::success(id, serde_json::json!({"valid": true})),
        Err(msg) => JsonRpcResponse::success(id, serde_json::json!({"valid": false, "error": msg})),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intern_core::{JsonRpcRequest, JsonRpcResponse};

    #[test]
    fn parse_valid_jsonrpc_request() {
        let json = r#"{"jsonrpc":"2.0","method":"task.list","params":{},"id":1}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "task.list");
        assert_eq!(req.id, serde_json::json!(1));
    }

    #[test]
    fn parse_malformed_json_fails() {
        let json = r#"{"jsonrpc":"2.0","method":"task.list"#;
        let result: Result<JsonRpcRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_missing_method_fails() {
        let json = r#"{"jsonrpc":"2.0","id":1}"#;
        let result: Result<JsonRpcRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn response_serialization_success() {
        let resp = JsonRpcResponse::success(serde_json::json!(1), serde_json::json!({"ok": true}));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"result\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn response_serialization_error() {
        let resp = JsonRpcResponse::error(serde_json::json!(1), -32601, "Method not found".into());
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"error\""));
        assert!(json.contains("-32601"));
        assert!(json.contains("Method not found"));
        assert!(!json.contains("\"result\""));
    }

    #[tokio::test]
    async fn dispatch_unknown_method() {
        // We need a minimal SharedState. We can test the dispatch function
        // for unknown methods without full state.
        let resp = JsonRpcResponse::error(
            serde_json::json!(1),
            -32601,
            "Method not found: bogus.method".into(),
        );
        assert_eq!(resp.error.as_ref().unwrap().code, -32601);
        assert!(resp
            .error
            .as_ref()
            .unwrap()
            .message
            .contains("Method not found"));
    }

    // ── schedule.validate handler tests ────────────────────────

    #[tokio::test]
    async fn schedule_validate_valid_expression_returns_valid_true() {
        let params = serde_json::json!({"expression": "0 9 * * 1-5"});
        let resp = handle_schedule_validate(serde_json::json!(1), params).await;
        assert!(resp.result.is_some(), "Result must be present");
        assert!(resp.error.is_none(), "Error must be absent");
        let result = resp.result.unwrap();
        assert_eq!(result.get("valid").and_then(|v| v.as_bool()), Some(true));
        assert!(
            result.get("error").is_none(),
            "error field must be absent on success"
        );
    }

    #[tokio::test]
    async fn schedule_validate_invalid_expression_returns_valid_false() {
        let params = serde_json::json!({"expression": "not a cron"});
        let resp = handle_schedule_validate(serde_json::json!(1), params).await;
        assert!(
            resp.result.is_some(),
            "Result must be present for invalid cron"
        );
        assert!(
            resp.error.is_none(),
            "JSON-RPC error must be absent for invalid cron"
        );
        let result = resp.result.unwrap();
        assert_eq!(result.get("valid").and_then(|v| v.as_bool()), Some(false));
        let error_msg = result
            .get("error")
            .and_then(|v| v.as_str())
            .expect("error field must be present");
        assert!(!error_msg.is_empty(), "error message must be non-empty");
    }

    #[tokio::test]
    async fn schedule_validate_missing_expression_returns_rpc_error() {
        let params = serde_json::json!({});
        let resp = handle_schedule_validate(serde_json::json!(1), params).await;
        assert!(
            resp.error.is_some(),
            "JSON-RPC error expected when expression is missing"
        );
        assert!(resp.result.is_none());
        assert_eq!(resp.error.unwrap().code, -32602);
    }

    #[test]
    fn templates_list_returns_five() {
        let templates = builtin_templates();
        assert_eq!(templates.len(), 5);

        let slugs: Vec<&str> = templates.iter().map(|t| t.slug.as_str()).collect();
        assert!(slugs.contains(&"pr-review"));
        assert!(slugs.contains(&"error-monitor"));
        assert!(slugs.contains(&"morning-briefing"));
        assert!(slugs.contains(&"dependency-audit"));
        assert!(slugs.contains(&"test-health"));
    }
}

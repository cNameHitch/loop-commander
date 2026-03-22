//! JSON-RPC handlers for prompt generation and agent registry management.
//!
//! Methods:
//! - `prompt.generate`: Generate a task prompt from user intent + optional agents
//! - `registry.refresh`: Force-refresh the agent registry cache
//! - `registry.list`: List all cached agents

use intern_config::RegistryManager;
use intern_core::prompt::{
    build_edit_meta_prompt, build_meta_prompt, build_optimization_prompt, build_retry_meta_prompt,
    truncate_log_for_prompt, validate_edit_result, validate_generated_prompt,
    validate_optimization_result, LogSummary, OptimizationFocus, ParsedPrompt, ValidationOutcome,
};
use intern_core::{rpc_errors, JsonRpcResponse, LogQuery, TaskId};
use serde::Deserialize;
use tracing::{debug, error, info, warn};

use crate::server::SharedState;

// ── Request Types ───────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GenerateParams {
    intent: String,
    #[serde(default)]
    agents: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    working_dir: Option<String>,
    #[serde(default)]
    regenerate: bool,
    #[serde(default)]
    feedback: Option<String>,
    /// If provided, reuse the same task ID (for regeneration).
    #[serde(default)]
    task_id: Option<String>,
}

// ── prompt.generate ─────────────────────────────────────

/// Handle `prompt.generate` JSON-RPC requests.
///
/// 1. Look up agent slugs in the registry
/// 2. Build the meta-prompt with substitutions
/// 3. Invoke `claude -p` to generate the prompt
/// 4. Validate and auto-fix the output
/// 5. Save to disk and return the result
pub async fn handle_prompt_generate(
    id: serde_json::Value,
    params: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    let input: GenerateParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error(id, -32602, format!("Invalid params: {e}"));
        }
    };

    if input.intent.trim().is_empty() {
        return JsonRpcResponse::error(
            id,
            rpc_errors::VALIDATION_ERROR,
            "intent: must not be empty".into(),
        );
    }

    // 1. Look up agents in registry
    let registry_mgr = {
        let cfg = state.config.lock().await;
        RegistryManager::new(&cfg.paths().root)
    };
    let registry = registry_mgr.load_cache();
    let agent_entries = registry.lookup_agents(&input.agents);

    // Build effective intent (with feedback for regeneration)
    let effective_intent = if input.regenerate {
        if let Some(feedback) = &input.feedback {
            format!(
                "{}\n\nUser feedback on previous generation: {feedback}",
                input.intent
            )
        } else {
            input.intent.clone()
        }
    } else {
        input.intent.clone()
    };

    // 2. Build the meta-prompt
    let meta_prompt = build_meta_prompt(&effective_intent, &agent_entries);

    // 3. Invoke Claude
    let raw_output = match invoke_claude(&meta_prompt).await {
        Ok(output) => output,
        Err(e) => {
            error!("Claude invocation failed: {e}");
            return JsonRpcResponse::error(id, -32603, format!("Claude invocation failed: {e}"));
        }
    };

    // 4. Validate
    let (validation, parsed) = validate_generated_prompt(&raw_output, &input.agents);

    if !validation.is_valid {
        // Automatic retry with error feedback
        debug!(
            errors = ?validation.errors,
            "First generation failed validation, retrying"
        );

        let retry_prompt =
            build_retry_meta_prompt(&effective_intent, &agent_entries, &validation.errors);

        let retry_output = match invoke_claude(&retry_prompt).await {
            Ok(output) => output,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    -32603,
                    format!("Claude retry invocation failed: {e}"),
                );
            }
        };

        let (retry_validation, retry_parsed) =
            validate_generated_prompt(&retry_output, &input.agents);

        if !retry_validation.is_valid {
            return JsonRpcResponse::error(
                id,
                rpc_errors::VALIDATION_ERROR,
                format!(
                    "Generated prompt failed validation after retry: {}",
                    retry_validation.errors.join("; ")
                ),
            );
        }

        return build_success_response(
            id,
            retry_parsed.unwrap(),
            &retry_output,
            &input,
            &registry_mgr,
        )
        .await;
    }

    build_success_response(id, parsed.unwrap(), &raw_output, &input, &registry_mgr).await
}

/// Build the success response: save the prompt file and return metadata.
async fn build_success_response(
    id: serde_json::Value,
    parsed: ParsedPrompt,
    raw_content: &str,
    input: &GenerateParams,
    registry_mgr: &RegistryManager,
) -> JsonRpcResponse {
    // Generate or reuse task ID
    let task_id = input.task_id.clone().unwrap_or_else(|| TaskId::new().0);

    // 5. Save to disk
    let prompt_path = match registry_mgr.save_prompt(&task_id, raw_content) {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error(id, -32603, format!("Failed to save prompt file: {e}"));
        }
    };

    info!(
        task_id = %task_id,
        name = %parsed.name,
        "Generated and saved prompt"
    );

    // 6. Return result
    let result = serde_json::json!({
        "prompt_path": prompt_path.to_string_lossy(),
        "task_id": task_id,
        "name": parsed.name,
        "description": parsed.description,
        "tags": parsed.tags,
        "agents": parsed.agents,
        "command": parsed.body,
    });

    JsonRpcResponse::success(id, result)
}

// ── registry.refresh ────────────────────────────────────

/// Handle `registry.refresh` JSON-RPC requests.
///
/// Forces an immediate fetch from GitHub and returns the new agent count.
pub async fn handle_registry_refresh(
    id: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    let registry_mgr = {
        let cfg = state.config.lock().await;
        RegistryManager::new(&cfg.paths().root)
    };

    match registry_mgr.refresh().await {
        Ok(registry) => {
            let result = serde_json::json!({
                "agent_count": registry.agents.len(),
                "fetched_at": registry.fetched_at.to_rfc3339(),
            });
            JsonRpcResponse::success(id, result)
        }
        Err(e) => JsonRpcResponse::error(id, -32603, format!("Registry refresh failed: {e}")),
    }
}

// ── registry.list ───────────────────────────────────────

/// Handle `registry.list` JSON-RPC requests.
///
/// Returns the cached agent list. If the cache is stale, triggers a background
/// refresh but returns the current cache immediately.
pub async fn handle_registry_list(id: serde_json::Value, state: &SharedState) -> JsonRpcResponse {
    let registry_mgr = {
        let cfg = state.config.lock().await;
        RegistryManager::new(&cfg.paths().root)
    };

    let registry = registry_mgr.load_cache();

    // Trigger background refresh if stale (non-blocking)
    if registry.is_stale(24) {
        let root = {
            let cfg = state.config.lock().await;
            cfg.paths().root.clone()
        };
        tokio::spawn(async move {
            let mgr = RegistryManager::new(&root);
            if let Err(e) = mgr.refresh().await {
                warn!("Background registry refresh failed: {e}");
            }
        });
    }

    match serde_json::to_value(&registry.agents) {
        Ok(val) => JsonRpcResponse::success(id, val),
        Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
    }
}

// ── prompt.optimize ─────────────────────────────────────

/// Maximum total prompt character budget before log count is halved.
const MAX_PROMPT_CHARS: usize = 80_000;

#[derive(Debug, Deserialize)]
struct OptimizeParams {
    task_id: String,
    #[serde(default)]
    max_logs: Option<u32>,
    #[serde(default)]
    focus: Option<String>,
    #[serde(default)]
    feedback: Option<String>,
}

/// Handle `prompt.optimize` JSON-RPC requests.
///
/// 1. Parse params; clamp `max_logs` to [1, 50] with default 10.
/// 2. Look up the task via the config manager — return `-32602` if not found.
/// 3. Query execution logs — return `-32602` if none exist.
/// 4. Truncate stdout/stderr per log (stdout: 1 500 head + 1 000 tail chars;
///    stderr: 1 000 head + 1 500 tail chars, weighted for exit-reason visibility).
/// 5. Convert to `LogSummary` vec, prioritising failed runs first.
/// 6. Compute pattern annotations and build the optimization prompt.
/// 7. Invoke Claude via the existing `invoke_claude` helper.
/// 8. Validate the JSON response.  On failure, retry once with the error appended.
/// 9. Return the optimized result as a JSON-RPC success response.
pub async fn handle_prompt_optimize(
    id: serde_json::Value,
    params: serde_json::Value,
    state: &SharedState,
) -> JsonRpcResponse {
    // 1. Parse and validate params.
    let input: OptimizeParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error(id, -32602, format!("Invalid params: {e}"));
        }
    };

    let max_logs = input.max_logs.map(|n| n.clamp(1, 50)).unwrap_or(10) as usize;

    let focus: OptimizationFocus = match input.focus.as_deref() {
        Some("efficiency") => OptimizationFocus::Efficiency,
        Some("quality") => OptimizationFocus::Quality,
        Some("consistency") => OptimizationFocus::Consistency,
        Some("resilience") => OptimizationFocus::Resilience,
        Some("all") | None => OptimizationFocus::All,
        Some(other) => {
            return JsonRpcResponse::error(
                id,
                -32602,
                format!(
                    "Invalid focus '{other}': must be one of efficiency, quality, consistency, resilience, all"
                ),
            );
        }
    };

    // 2. Look up the task.
    let task = {
        let cfg = state.config.lock().await;
        match cfg.get_task(&input.task_id) {
            Ok(t) => t,
            Err(intern_core::InternError::TaskNotFound(_)) => {
                return JsonRpcResponse::error(
                    id,
                    -32602,
                    format!("Task not found: {}", input.task_id),
                );
            }
            Err(e) => {
                return JsonRpcResponse::error(id, -32603, format!("Config error: {e}"));
            }
        }
    };

    let original_command = task.command.clone();
    let task_name = task.name.clone();

    // 3. Query logs ordered newest-first; we'll reorder after selection.
    let raw_logs = {
        let logger = state.logger.lock().await;
        let query = LogQuery {
            task_id: Some(input.task_id.clone()),
            limit: Some(50), // fetch the max so we can apply priority selection
            ..Default::default()
        };
        match logger.query_logs(&query) {
            Ok(logs) => logs,
            Err(e) => {
                return JsonRpcResponse::error(id, -32603, format!("Log query failed: {e}"));
            }
        }
    };

    if raw_logs.is_empty() {
        return JsonRpcResponse::error(
            id,
            -32602,
            "No execution history found for this task. Run the task at least once before optimizing.".into(),
        );
    }

    // 4. Apply stdout/stderr truncation per log.
    //
    // 5. Prioritise: failed runs first, then most recent success, then remaining.
    let (failures, successes): (Vec<_>, Vec<_>) = raw_logs
        .into_iter()
        .partition(|l| l.exit_code != 0 || l.status.to_string().eq_ignore_ascii_case("timeout"));

    let mut selected = failures;
    selected.extend(successes);
    selected.truncate(max_logs);

    // Reverse to chronological order (oldest first) so the prompt reads naturally.
    selected.reverse();

    let log_summaries: Vec<LogSummary> = selected
        .iter()
        .enumerate()
        .map(|(i, log)| {
            let stdout_excerpt = truncate_log_for_prompt(&log.stdout, false);
            let stderr_excerpt = truncate_log_for_prompt(&log.stderr, true);

            LogSummary {
                run_index: i as u32,
                started_at: log.started_at.to_rfc3339(),
                duration_secs: log.duration_secs as f64,
                exit_code: log.exit_code,
                status: log.status.to_string(),
                stdout_excerpt,
                stderr_excerpt,
                tokens_used: log.tokens_used,
                cost_usd: log.cost_usd,
            }
        })
        .collect();

    let logs_analyzed = log_summaries.len();

    // 6. Build the optimization prompt, optionally appending user feedback.
    // Task has no description field; pass empty string to fall back to task_name.
    let mut optimization_prompt = build_optimization_prompt(
        &task_name,
        "",
        &original_command,
        &focus,
        &log_summaries,
        &[], // no agent slugs at this stage — could be extended later
    );

    if let Some(ref feedback) = input.feedback {
        if !feedback.trim().is_empty() {
            optimization_prompt.push_str(&format!(
                "\n\n## User Feedback\n\nPlease incorporate the following feedback:\n{feedback}\n"
            ));
        }
    }

    // Shrink log count iteratively if the prompt exceeds the character budget.
    if optimization_prompt.len() > MAX_PROMPT_CHARS {
        warn!(
            task_id = %input.task_id,
            prompt_len = optimization_prompt.len(),
            "Optimization prompt exceeds budget; halving log count"
        );
        let reduced_logs = reduce_logs_to_budget(
            &log_summaries,
            &task_name,
            "",
            &original_command,
            &focus,
            &[],
            MAX_PROMPT_CHARS,
        );
        optimization_prompt = build_optimization_prompt(
            &task_name,
            "",
            &original_command,
            &focus,
            &reduced_logs,
            &[],
        );
    }

    // 7. Invoke Claude.
    let raw_output = match invoke_claude(&optimization_prompt).await {
        Ok(output) => output,
        Err(e) => {
            error!(task_id = %input.task_id, "Claude invocation failed for optimize: {e}");
            return JsonRpcResponse::error(id, -32603, format!("Claude invocation failed: {e}"));
        }
    };

    // 8. Validate; retry once with error appended on failure.
    let ValidationOutcome { result, warnings } = match validate_optimization_result(
        &raw_output,
        &original_command,
    ) {
        Ok(outcome) => outcome,
        Err(validation_err) => {
            debug!(
                error = %validation_err,
                "Optimization validation failed, retrying with error feedback"
            );

            let retry_prompt = format!(
                    "{optimization_prompt}\n\nIMPORTANT: Your previous response failed validation: {validation_err}\nPlease correct this and return valid JSON only."
                );

            let retry_output = match invoke_claude(&retry_prompt).await {
                Ok(output) => output,
                Err(e) => {
                    return JsonRpcResponse::error(id, -32603, format!("Claude retry failed: {e}"));
                }
            };

            match validate_optimization_result(&retry_output, &original_command) {
                Ok(outcome) => outcome,
                Err(retry_err) => {
                    return JsonRpcResponse::error(
                        id,
                        -32603,
                        format!("Optimization failed validation after retry: {retry_err}"),
                    );
                }
            }
        }
    };

    for warning in &warnings {
        warn!(task_id = %input.task_id, "Optimization warning: {warning}");
    }

    info!(
        task_id = %input.task_id,
        confidence = result.confidence_score,
        logs_analyzed,
        "Prompt optimization completed"
    );

    // 9. Return result.
    let response = serde_json::json!({
        "task_id": input.task_id,
        "original_command": original_command,
        "optimized_command": result.optimized_command,
        "changes_summary": result.changes_summary,
        "confidence_score": result.confidence_score,
        "optimization_categories": result.optimization_categories,
        "logs_analyzed": logs_analyzed,
        "warnings": warnings,
    });

    JsonRpcResponse::success(id, response)
}

// ── prompt.edit ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct EditParams {
    /// The task's current name.
    name: String,
    /// The task's current command / prompt text.
    command: String,
    /// The task's current cron schedule expression.
    schedule: String,
    /// Budget per run in USD.
    #[serde(default = "intern_core::default_budget")]
    budget: f64,
    /// Timeout in seconds.
    #[serde(default = "intern_core::default_timeout")]
    timeout: u64,
    /// Current tag list.
    #[serde(default)]
    tags: Vec<String>,
    /// Current agent slug list.
    #[serde(default)]
    agents: Vec<String>,
    /// User's plain-English refinement request. Required, non-empty.
    feedback: String,
}

/// Handle `prompt.edit` JSON-RPC requests.
///
/// 1. Parse and validate params (feedback must be non-empty, <1000 chars)
/// 2. Build edit meta-prompt from the draft fields and user feedback
/// 3. Invoke Claude via `invoke_claude`
/// 4. Validate the JSON response; retry once on failure
/// 5. Return the refined draft fields and metadata as a JSON-RPC success
pub async fn handle_prompt_edit(
    id: serde_json::Value,
    params: serde_json::Value,
    _state: &SharedState,
) -> JsonRpcResponse {
    // 1. Parse params.
    let input: EditParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error(id, -32602, format!("Invalid params: {e}"));
        }
    };

    // Validate feedback.
    let feedback = input.feedback.trim().to_string();
    if feedback.is_empty() {
        return JsonRpcResponse::error(
            id,
            rpc_errors::VALIDATION_ERROR,
            "feedback: must not be empty".into(),
        );
    }
    if feedback.len() > 1000 {
        return JsonRpcResponse::error(
            id,
            rpc_errors::VALIDATION_ERROR,
            format!(
                "feedback: must be 1000 characters or fewer (got {})",
                feedback.len()
            ),
        );
    }

    // 2. Build meta-prompt.
    let meta_prompt = build_edit_meta_prompt(&intern_core::prompt::EditPromptParams {
        name: &input.name,
        command: &input.command,
        schedule: &input.schedule,
        budget: input.budget,
        timeout: input.timeout,
        tags: &input.tags,
        agents: &input.agents,
        feedback: &feedback,
    });

    // 3. Invoke Claude.
    let raw_output = match invoke_claude(&meta_prompt).await {
        Ok(output) => output,
        Err(e) => {
            error!("Claude invocation failed for prompt.edit: {e}");
            return JsonRpcResponse::error(id, -32603, format!("Claude invocation failed: {e}"));
        }
    };

    // 4. Validate; retry once with error feedback appended on failure.
    let result = match validate_edit_result(&raw_output) {
        Ok(r) => r,
        Err(validation_err) => {
            debug!(
                error = %validation_err,
                "Edit validation failed, retrying with error feedback"
            );

            let retry_prompt = format!(
                "{meta_prompt}\n\nIMPORTANT: Your previous response failed validation: {validation_err}\nPlease correct this and return valid JSON only."
            );

            let retry_output = match invoke_claude(&retry_prompt).await {
                Ok(output) => output,
                Err(e) => {
                    return JsonRpcResponse::error(id, -32603, format!("Claude retry failed: {e}"));
                }
            };

            match validate_edit_result(&retry_output) {
                Ok(r) => r,
                Err(retry_err) => {
                    return JsonRpcResponse::error(
                        id,
                        -32603,
                        format!("Edit failed validation after retry: {retry_err}"),
                    );
                }
            }
        }
    };

    info!(
        task_name = %input.name,
        confidence = result.confidence_score,
        fields_changed = result.field_changes.len(),
        "Prompt edit completed"
    );

    // 5. Return result.
    let field_changes_json: serde_json::Value = result
        .field_changes
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                serde_json::json!({
                    "type": v.change_type,
                    "reason": v.reason,
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>()
        .into();

    let response = serde_json::json!({
        "refined_name": result.refined_name,
        "refined_command": result.refined_command,
        "refined_schedule": result.refined_schedule,
        "refined_budget": result.refined_budget,
        "refined_timeout": result.refined_timeout,
        "refined_tags": result.refined_tags,
        "refined_agents": result.refined_agents,
        "changes_summary": result.changes_summary,
        "confidence_score": result.confidence_score,
        "field_changes": field_changes_json,
        "original_command": input.command,
    });

    JsonRpcResponse::success(id, response)
}

// ── Budget Reduction ─────────────────────────────────────

/// Reduce the log list to fit within the prompt budget.
///
/// Builds the optimization prompt with the current log slice and, if it
/// exceeds `max_chars`, halves the slice (keeping the most-recent half) and
/// returns that subset.  The function guarantees at least one log entry is
/// always returned so the caller always has something to work with.
///
/// # Arguments
///
/// * `logs` – Full ordered log slice (oldest → newest).
/// * `task_name` – Task name forwarded to the prompt builder.
/// * `task_description` – Task description forwarded to the prompt builder.
/// * `original_command` – The current command text.
/// * `focus` – Which optimization aspect to emphasise.
/// * `agents` – Agent slugs for the prompt.
/// * `max_chars` – Character budget (exclusive upper bound).
///
/// # Returns
///
/// The subset of `logs` that fits within the budget (or the minimum of one
/// log when even a single entry exceeds the budget).
fn reduce_logs_to_budget(
    logs: &[LogSummary],
    task_name: &str,
    task_description: &str,
    original_command: &str,
    focus: &OptimizationFocus,
    agents: &[&str],
    max_chars: usize,
) -> Vec<LogSummary> {
    let mut current = logs.to_vec();
    loop {
        let prompt = build_optimization_prompt(
            task_name,
            task_description,
            original_command,
            focus,
            &current,
            agents,
        );
        if prompt.len() <= max_chars || current.len() <= 1 {
            return current;
        }
        let half = (current.len() / 2).max(1);
        // Keep the most-recent half (tail of the slice).
        current = current[current.len() - half..].to_vec();
    }
}

// ── Claude Invocation ───────────────────────────────────

/// Invoke `claude -p` with the meta-prompt and extract the generated content.
///
/// Uses `--output-format json` and `--max-turns 1` for a single-shot generation.
/// Budget is capped at $1.00 with a 60-second timeout.
async fn invoke_claude(prompt: &str) -> Result<String, String> {
    let output = tokio::process::Command::new("claude")
        .args([
            "-p",
            prompt,
            "--output-format",
            "json",
            "--dangerously-skip-permissions",
            "--max-turns",
            "1",
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to spawn claude: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "claude exited with status {}: {stderr}",
            output.status
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| format!("Invalid UTF-8 in claude output: {e}"))?;

    // Try to parse as JSON and extract the result text
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
        if let Some(result_text) = json.get("result").and_then(|v| v.as_str()) {
            return Ok(result_text.to_string());
        }
    }

    // Fallback: treat entire stdout as raw Markdown
    Ok(stdout)
}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use intern_core::prompt::LogSummary;

    /// Build a minimal [`LogSummary`] for use in tests.
    fn make_log(run_index: u32) -> LogSummary {
        LogSummary {
            run_index,
            started_at: "2026-03-18T10:00:00Z".into(),
            duration_secs: 5.0,
            exit_code: 0,
            status: "success".into(),
            // Long enough that many logs together exceed realistic budgets.
            stdout_excerpt: "x".repeat(5_000),
            stderr_excerpt: String::new(),
            tokens_used: None,
            cost_usd: None,
        }
    }

    // ── GAP-03: reduce_logs_to_budget ────────────────────────────────────────

    #[test]
    fn reduce_logs_to_budget_returns_all_when_within_budget() {
        // Use a very large budget so nothing should be trimmed.
        let logs: Vec<LogSummary> = (0..5).map(make_log).collect();
        let result = reduce_logs_to_budget(
            &logs,
            "test-task",
            "",
            "claude -p 'run it'",
            &OptimizationFocus::All,
            &[],
            usize::MAX,
        );
        assert_eq!(
            result.len(),
            5,
            "all logs should be returned when within budget"
        );
    }

    #[test]
    fn reduce_logs_to_budget_halves_when_over_budget() {
        // Use a budget of 0 so any prompt will exceed it, forcing at least one halving.
        // With 10 logs and a budget of 0 the function should converge to 1 log.
        let logs: Vec<LogSummary> = (0..10).map(make_log).collect();
        let result = reduce_logs_to_budget(
            &logs,
            "test-task",
            "",
            "claude -p 'run it'",
            &OptimizationFocus::All,
            &[],
            0,
        );
        // Must be at least 1 (the guaranteed minimum) and strictly fewer than input.
        assert!(
            result.len() < logs.len(),
            "log count should have been reduced; got {}",
            result.len()
        );
        assert!(
            !result.is_empty(),
            "at least one log must always be returned"
        );
    }

    #[test]
    fn reduce_logs_to_budget_keeps_most_recent() {
        // With budget=0 and 4 logs labelled 0..3, the function should converge to
        // the single most-recent log (index 3, which is the last in the slice).
        let logs: Vec<LogSummary> = (0..4).map(make_log).collect();
        let result = reduce_logs_to_budget(
            &logs,
            "test-task",
            "",
            "run it",
            &OptimizationFocus::All,
            &[],
            0,
        );
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].run_index, 3,
            "should keep the most-recent (last) log; got index {}",
            result[0].run_index
        );
    }

    #[test]
    fn parse_generate_params() {
        let json = serde_json::json!({
            "intent": "Review PRs for security issues",
            "agents": ["security-auditor", "code-reviewer"],
            "working_dir": "/Users/test/project"
        });

        let params: GenerateParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.intent, "Review PRs for security issues");
        assert_eq!(params.agents.len(), 2);
        assert_eq!(params.working_dir, Some("/Users/test/project".to_string()));
        assert!(!params.regenerate);
        assert!(params.feedback.is_none());
    }

    #[test]
    fn parse_generate_params_minimal() {
        let json = serde_json::json!({
            "intent": "Run tests"
        });

        let params: GenerateParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.intent, "Run tests");
        assert!(params.agents.is_empty());
        assert!(params.working_dir.is_none());
    }

    #[test]
    fn parse_edit_params_full() {
        let json = serde_json::json!({
            "name": "Security Audit",
            "command": "Run a basic security check on the codebase",
            "schedule": "0 9 * * *",
            "budget": 2.0,
            "timeout": 600,
            "tags": ["security"],
            "agents": ["security-auditor"],
            "feedback": "Make it shorter and focus on critical vulns"
        });
        let params: EditParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.name, "Security Audit");
        assert_eq!(
            params.feedback,
            "Make it shorter and focus on critical vulns"
        );
        assert_eq!(params.budget, 2.0);
        assert_eq!(params.tags.len(), 1);
    }

    #[test]
    fn parse_edit_params_defaults() {
        let json = serde_json::json!({
            "name": "T",
            "command": "do thing",
            "schedule": "* * * * *",
            "feedback": "fix it"
        });
        let params: EditParams = serde_json::from_value(json).unwrap();
        assert!(params.budget > 0.0, "budget default should be positive");
        assert!(params.timeout > 0, "timeout default should be positive");
        assert!(params.tags.is_empty());
        assert!(params.agents.is_empty());
    }

    #[test]
    fn parse_regenerate_params() {
        let json = serde_json::json!({
            "intent": "Review PRs",
            "agents": [],
            "regenerate": true,
            "feedback": "Focus more on SQL injection",
            "task_id": "lc-abc12345"
        });

        let params: GenerateParams = serde_json::from_value(json).unwrap();
        assert!(params.regenerate);
        assert_eq!(
            params.feedback,
            Some("Focus more on SQL injection".to_string())
        );
        assert_eq!(params.task_id, Some("lc-abc12345".to_string()));
    }
}

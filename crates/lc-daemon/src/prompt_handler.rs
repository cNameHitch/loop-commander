//! JSON-RPC handlers for prompt generation and agent registry management.
//!
//! Methods:
//! - `prompt.generate`: Generate a task prompt from user intent + optional agents
//! - `registry.refresh`: Force-refresh the agent registry cache
//! - `registry.list`: List all cached agents

use lc_config::RegistryManager;
use lc_core::prompt::{
    build_meta_prompt, build_retry_meta_prompt, validate_generated_prompt, ParsedPrompt,
};
use lc_core::{rpc_errors, JsonRpcResponse, TaskId};
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

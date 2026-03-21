//! Shared logic for the `intern-runner` task executor.
//!
//! This module provides functions that are used by both the runner binary and
//! the daemon's dry-run feature. All command-building follows CC-2 (no shell
//! interpolation) and all cost extraction follows CC-10 (actual then fallback).

use anyhow::Result;
use chrono::Utc;
use intern_core::Task;
use intern_logger::Logger;

/// Build the command argv array for executing a task.
///
/// # Rules (CC-2)
///
/// - If `task.command` starts with `"claude"` and no context file provides an
///   `## Instructions` section: parse the command string into argv tokens by
///   splitting on whitespace while respecting single and double quotes.
/// - Otherwise: build `["claude", "-p", <prompt>, "--output-format", "json"]`,
///   where `<prompt>` is the `## Instructions` section from the context file
///   when present, or `task.command` as a fallback.
/// - Always append `["--output-format", "json"]` if not already present (needed
///   for cost extraction from Claude Code output).
/// - If `task.max_turns` is `Some(n)`: append `["--max-turns", &n.to_string()]`.
/// - **NEVER** use shell interpolation or `sh -c`.
#[must_use]
pub fn build_command(task: &Task, context_file: Option<&std::path::Path>) -> Vec<String> {
    let trimmed = task.command.trim();

    // If a context file exists, read the Instructions section.
    let effective_prompt: Option<String> = context_file
        .filter(|p| p.exists())
        .and_then(read_instructions_from_context_file);

    let mut argv = if trimmed.starts_with("claude") && effective_prompt.is_none() {
        // User wrote an explicit claude invocation — parse it as-is.
        parse_shell_tokens(trimmed)
    } else {
        // Prompt-only command: wrap in claude -p.
        // Use context file content if available, otherwise fall back to task.command.
        let prompt = effective_prompt.unwrap_or_else(|| task.command.clone());
        vec![
            "claude".to_string(),
            "-p".to_string(),
            prompt,
            "--output-format".to_string(),
            "json".to_string(),
        ]
    };

    // Ensure --dangerously-skip-permissions is present for unattended execution.
    if !argv.iter().any(|a| a == "--dangerously-skip-permissions") {
        argv.push("--dangerously-skip-permissions".to_string());
    }

    // Ensure --output-format json is present for cost extraction.
    if !argv.iter().any(|a| a == "--output-format") {
        argv.push("--output-format".to_string());
        argv.push("json".to_string());
    }

    // Append --max-turns if configured.
    if let Some(n) = task.max_turns {
        argv.push("--max-turns".to_string());
        argv.push(n.to_string());
    }

    argv
}

/// Read the `## Instructions` section from a context file.
///
/// Returns the trimmed text between `## Instructions` and the next `##`
/// heading (or end of file). Returns `None` if the file cannot be read or
/// the section is absent.
pub(crate) fn read_instructions_from_context_file(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut in_section = false;
    let mut lines: Vec<&str> = Vec::new();

    for line in content.lines() {
        if line.trim_start().starts_with("## Instructions") {
            in_section = true;
            continue;
        }
        if in_section {
            if line.starts_with("## ") {
                break;
            }
            lines.push(line);
        }
    }

    if lines.is_empty() {
        return None;
    }

    let joined = lines.join("\n");
    let trimmed = joined.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Parse a command string into argv tokens, splitting on whitespace while
/// respecting single and double quotes.
///
/// This is a simplified shell-like tokenizer that handles:
/// - Unquoted tokens split on whitespace
/// - Single-quoted strings (contents taken literally)
/// - Double-quoted strings (contents taken literally, no escape processing)
///
/// It does NOT handle escape characters, variable expansion, or any other
/// shell features -- by design (CC-2).
fn parse_shell_tokens(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    for ch in input.chars() {
        match ch {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            c if c.is_whitespace() && !in_single_quote && !in_double_quote => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Parse Claude Code JSON output for token usage and cost information.
///
/// Attempts to extract actual cost data from the JSON output that Claude Code
/// produces when run with `--output-format json`. The output may contain
/// multiple JSON objects (one per line); we try each line.
///
/// # Returns
///
/// A tuple of `(tokens_used, cost_usd, is_estimate)`:
/// - `tokens_used`: total input + output tokens if found
/// - `cost_usd`: actual USD cost if found
/// - `is_estimate`: `true` if we could not extract real data (caller should
///   use `estimate_cost` as a fallback)
#[must_use]
pub fn parse_cost_from_output(stdout: &str) -> (Option<u64>, Option<f64>, bool) {
    // Try parsing the entire stdout as a single JSON object first.
    if let Some(result) = try_extract_cost_from_json(stdout) {
        return result;
    }

    // Try each line individually (Claude Code may output newline-delimited JSON).
    for line in stdout.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with('{') {
            if let Some(result) = try_extract_cost_from_json(trimmed) {
                return result;
            }
        }
    }

    // Could not parse anything useful.
    (None, None, true)
}

/// Try to extract cost/token data from a JSON string.
///
/// Returns `Some((tokens, cost, is_estimate))` if the JSON was valid and
/// contained at least one useful field, or `None` if parsing failed entirely.
fn try_extract_cost_from_json(json_str: &str) -> Option<(Option<u64>, Option<f64>, bool)> {
    let value: serde_json::Value = serde_json::from_str(json_str).ok()?;

    let mut tokens: Option<u64> = None;
    let mut cost: Option<f64> = None;

    // Look for cost_usd at the top level or nested under "result".
    cost = cost.or_else(|| extract_f64(&value, "cost_usd"));
    cost = cost.or_else(|| extract_f64(&value, "total_cost"));
    cost = cost.or_else(|| value.get("result").and_then(|r| extract_f64(r, "cost_usd")));
    cost = cost.or_else(|| {
        value
            .get("result")
            .and_then(|r| extract_f64(r, "total_cost"))
    });

    // Look for token usage.
    if let Some(usage) = value
        .get("usage")
        .or_else(|| value.get("result").and_then(|r| r.get("usage")))
    {
        let input = extract_u64(usage, "input_tokens").unwrap_or(0);
        let output = extract_u64(usage, "output_tokens").unwrap_or(0);
        if input > 0 || output > 0 {
            tokens = Some(input + output);
        }
    }

    // Also check for a flat "tokens_used" or "total_tokens" field.
    tokens = tokens
        .or_else(|| extract_u64(&value, "tokens_used"))
        .or_else(|| extract_u64(&value, "total_tokens"));

    if tokens.is_some() || cost.is_some() {
        Some((tokens, cost, false))
    } else {
        // JSON was valid but contained no cost/token data.
        None
    }
}

/// Extract a `u64` from a JSON object by key name.
fn extract_u64(value: &serde_json::Value, key: &str) -> Option<u64> {
    value.get(key).and_then(serde_json::Value::as_u64)
}

/// Extract an `f64` from a JSON object by key name.
fn extract_f64(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key).and_then(serde_json::Value::as_f64)
}

/// Fallback cost estimation based on execution duration.
///
/// Used when actual cost data cannot be extracted from Claude Code output
/// (CC-10). The formula is: `duration_secs * rate`.
///
/// The default rate is `$0.01/second` (configurable via
/// `GlobalConfig.cost_estimate_per_second`).
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn estimate_cost(duration_secs: u64, rate: f64) -> f64 {
    duration_secs as f64 * rate
}

/// Check whether the daily budget has been exceeded for a task.
///
/// Queries the logger for the total cost of the given task since the start of
/// today (UTC). Returns `Ok(true)` if under budget, `Ok(false)` if the daily
/// cap has been reached or exceeded.
///
/// # Errors
///
/// Returns an error if the database query fails.
///
/// # Panics
///
/// Panics if midnight (00:00:00) cannot be represented as a valid time, which
/// should never happen with the chrono library.
pub fn check_budget(logger: &Logger, task_id: &str, daily_cap: f64) -> Result<bool> {
    let today_start = Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("midnight is always valid")
        .and_utc();

    let spent = logger.total_cost_since(task_id, today_start)?;

    Ok(spent < daily_cap)
}

/// Generate a summary from captured stdout/stderr.
///
/// Takes the first 200 characters of stdout. If stdout is empty, falls back
/// to the first 200 characters of stderr. If both are empty, returns a default
/// message.
#[must_use]
pub fn generate_summary(stdout: &str, stderr: &str) -> String {
    let source = if stdout.trim().is_empty() {
        stderr
    } else {
        stdout
    };

    let trimmed = source.trim();
    if trimmed.is_empty() {
        return "(no output)".to_string();
    }

    if trimmed.len() <= 200 {
        trimmed.to_string()
    } else {
        let mut summary: String = trimmed.chars().take(200).collect();
        summary.push_str("...");
        summary
    }
}

// ── Tests ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use intern_core::{Schedule, Task, TaskId, TaskStatus};
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// Create a minimal task for testing `build_command`.
    fn make_task(command: &str, max_turns: Option<u32>) -> Task {
        Task {
            id: TaskId("lc-test0001".to_string()),
            name: "Test Task".to_string(),
            command: command.to_string(),
            skill: None,
            schedule: Schedule::Interval { seconds: 300 },
            schedule_human: "Every 5m".to_string(),
            working_dir: PathBuf::from("/tmp"),
            env_vars: HashMap::new(),
            max_budget_per_run: 5.0,
            max_turns,
            timeout_secs: 600,
            status: TaskStatus::Active,
            tags: vec![],
            agents: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // ── build_command ────────────────────────────────────

    #[test]
    fn build_command_claude_prefix_parsed_correctly() {
        let task = make_task("claude -p 'Review all open PRs' --verbose", None);
        let argv = build_command(&task, None);

        assert_eq!(argv[0], "claude");
        assert_eq!(argv[1], "-p");
        assert_eq!(argv[2], "Review all open PRs");
        assert_eq!(argv[3], "--verbose");
        // --output-format json should be appended
        assert!(argv.contains(&"--output-format".to_string()));
        assert!(argv.contains(&"json".to_string()));
    }

    #[test]
    fn build_command_claude_with_existing_output_format() {
        let task = make_task("claude -p 'Do stuff' --output-format json", None);
        let argv = build_command(&task, None);

        // Should not duplicate --output-format
        let count = argv.iter().filter(|a| *a == "--output-format").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn build_command_non_claude_wraps_with_claude_p() {
        let task = make_task("Review all open PRs and fix issues", None);
        let argv = build_command(&task, None);

        assert_eq!(argv[0], "claude");
        assert_eq!(argv[1], "-p");
        assert_eq!(argv[2], "Review all open PRs and fix issues");
        assert_eq!(argv[3], "--output-format");
        assert_eq!(argv[4], "json");
    }

    #[test]
    fn build_command_max_turns_appended() {
        let task = make_task("echo hello", Some(25));
        let argv = build_command(&task, None);

        let turns_idx = argv
            .iter()
            .position(|a| a == "--max-turns")
            .expect("--max-turns should be present");
        assert_eq!(argv[turns_idx + 1], "25");
    }

    #[test]
    fn build_command_no_max_turns_when_none() {
        let task = make_task("echo hello", None);
        let argv = build_command(&task, None);

        assert!(!argv.contains(&"--max-turns".to_string()));
    }

    #[test]
    fn build_command_claude_double_quotes() {
        let task = make_task(r#"claude -p "Review PRs with 'special' chars""#, None);
        let argv = build_command(&task, None);

        assert_eq!(argv[0], "claude");
        assert_eq!(argv[1], "-p");
        assert_eq!(argv[2], "Review PRs with 'special' chars");
    }

    // ── parse_cost_from_output ───────────────────────────

    #[test]
    fn parse_cost_valid_json_with_cost_and_tokens() {
        let output = r#"{"cost_usd": 0.42, "usage": {"input_tokens": 1000, "output_tokens": 500}}"#;
        let (tokens, cost, is_estimate) = parse_cost_from_output(output);

        assert_eq!(tokens, Some(1500));
        assert!((cost.unwrap() - 0.42).abs() < f64::EPSILON);
        assert!(!is_estimate);
    }

    #[test]
    fn parse_cost_valid_json_cost_only() {
        let output = r#"{"cost_usd": 1.23}"#;
        let (tokens, cost, is_estimate) = parse_cost_from_output(output);

        assert!(tokens.is_none());
        assert!((cost.unwrap() - 1.23).abs() < f64::EPSILON);
        assert!(!is_estimate);
    }

    #[test]
    fn parse_cost_valid_json_tokens_only() {
        let output = r#"{"usage": {"input_tokens": 2000, "output_tokens": 800}}"#;
        let (tokens, cost, is_estimate) = parse_cost_from_output(output);

        assert_eq!(tokens, Some(2800));
        assert!(cost.is_none());
        assert!(!is_estimate);
    }

    #[test]
    fn parse_cost_nested_under_result() {
        let output = r#"{"result": {"cost_usd": 0.55, "usage": {"input_tokens": 300, "output_tokens": 100}}}"#;
        let (tokens, cost, is_estimate) = parse_cost_from_output(output);

        assert_eq!(tokens, Some(400));
        assert!((cost.unwrap() - 0.55).abs() < f64::EPSILON);
        assert!(!is_estimate);
    }

    #[test]
    fn parse_cost_invalid_json_returns_none() {
        let output = "this is not json at all";
        let (tokens, cost, is_estimate) = parse_cost_from_output(output);

        assert!(tokens.is_none());
        assert!(cost.is_none());
        assert!(is_estimate);
    }

    #[test]
    fn parse_cost_empty_string_returns_none() {
        let (tokens, cost, is_estimate) = parse_cost_from_output("");

        assert!(tokens.is_none());
        assert!(cost.is_none());
        assert!(is_estimate);
    }

    #[test]
    fn parse_cost_json_without_cost_fields_returns_none() {
        let output = r#"{"status": "ok", "message": "done"}"#;
        let (tokens, cost, is_estimate) = parse_cost_from_output(output);

        assert!(tokens.is_none());
        assert!(cost.is_none());
        assert!(is_estimate);
    }

    #[test]
    fn parse_cost_multiline_json_last_line() {
        let output = "some log output\nmore logs\n{\"cost_usd\": 0.99}";
        let (_tokens, cost, is_estimate) = parse_cost_from_output(output);

        assert!((cost.unwrap() - 0.99).abs() < f64::EPSILON);
        assert!(!is_estimate);
    }

    // ── estimate_cost ────────────────────────────────────

    #[test]
    fn estimate_cost_basic_calculation() {
        let cost = estimate_cost(100, 0.01);
        assert!((cost - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn estimate_cost_zero_duration() {
        let cost = estimate_cost(0, 0.01);
        assert!((cost - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn estimate_cost_custom_rate() {
        let cost = estimate_cost(60, 0.05);
        assert!((cost - 3.0).abs() < f64::EPSILON);
    }

    // ── generate_summary ─────────────────────────────────

    #[test]
    fn generate_summary_short_stdout() {
        let summary = generate_summary("hello world", "");
        assert_eq!(summary, "hello world");
    }

    #[test]
    fn generate_summary_falls_back_to_stderr() {
        let summary = generate_summary("", "error occurred");
        assert_eq!(summary, "error occurred");
    }

    #[test]
    fn generate_summary_empty_both() {
        let summary = generate_summary("", "");
        assert_eq!(summary, "(no output)");
    }

    #[test]
    fn generate_summary_truncates_long_stdout() {
        let long_output = "x".repeat(300);
        let summary = generate_summary(&long_output, "");
        assert_eq!(summary.len(), 203); // 200 chars + "..."
        assert!(summary.ends_with("..."));
    }

    #[test]
    fn generate_summary_trims_whitespace() {
        let summary = generate_summary("  \n  ", "  fallback  ");
        assert_eq!(summary, "fallback");
    }

    // ── parse_shell_tokens ───────────────────────────────

    #[test]
    fn parse_tokens_simple_command() {
        let tokens = parse_shell_tokens("claude -p hello");
        assert_eq!(tokens, vec!["claude", "-p", "hello"]);
    }

    #[test]
    fn parse_tokens_single_quotes() {
        let tokens = parse_shell_tokens("claude -p 'hello world'");
        assert_eq!(tokens, vec!["claude", "-p", "hello world"]);
    }

    #[test]
    fn parse_tokens_double_quotes() {
        let tokens = parse_shell_tokens(r#"claude -p "hello world""#);
        assert_eq!(tokens, vec!["claude", "-p", "hello world"]);
    }

    #[test]
    fn parse_tokens_mixed_quotes() {
        let tokens = parse_shell_tokens(r#"claude -p "it's a test""#);
        assert_eq!(tokens, vec!["claude", "-p", "it's a test"]);
    }

    #[test]
    fn parse_tokens_multiple_spaces() {
        let tokens = parse_shell_tokens("claude   -p   hello");
        assert_eq!(tokens, vec!["claude", "-p", "hello"]);
    }

    #[test]
    fn parse_tokens_empty_string() {
        let tokens = parse_shell_tokens("");
        assert!(tokens.is_empty());
    }

    // ── Integration: verify tokio::process::Command works ─

    #[tokio::test]
    async fn integration_echo_command_runs() {
        let output = tokio::process::Command::new("echo")
            .arg("hello from intern-runner test")
            .output()
            .await
            .expect("failed to run echo");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("hello from intern-runner test"));
    }

    // ── check_budget ─────────────────────────────────────

    #[test]
    fn check_budget_under_limit() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let logger = Logger::new(&db_path).unwrap();

        // No logs at all, so cost is 0 -- should be under any positive budget.
        let under = check_budget(&logger, "lc-test0001", 10.0).unwrap();
        assert!(under);
    }

    #[test]
    fn check_budget_over_limit() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let logger = Logger::new(&db_path).unwrap();

        // Insert a log with high cost for today.
        let log = intern_core::ExecutionLog {
            id: 0,
            task_id: "lc-test0001".to_string(),
            task_name: "Test".to_string(),
            started_at: Utc::now(),
            finished_at: Utc::now(),
            duration_secs: 60,
            exit_code: 0,
            status: intern_core::ExecStatus::Success,
            stdout: String::new(),
            stderr: String::new(),
            tokens_used: None,
            cost_usd: Some(50.0),
            cost_is_estimate: false,
            summary: "test".to_string(),
        };
        logger.insert_log(&log).unwrap();

        // Budget cap of 10.0, but we've already spent 50.0.
        let under = check_budget(&logger, "lc-test0001", 10.0).unwrap();
        assert!(!under);
    }

    // ── context file tests ───────────────────────────────

    #[test]
    fn build_command_with_context_file() {
        let task = make_task("Do something", None);

        // Write a temporary context file.
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("test-task.md");
        std::fs::write(&file_path, "## Instructions\n\nDo something from file\n").unwrap();

        let argv = build_command(&task, Some(&file_path));
        assert_eq!(argv[0], "claude");
        assert_eq!(argv[1], "-p");
        assert_eq!(argv[2], "Do something from file");
    }

    #[test]
    fn build_command_context_file_missing_falls_back() {
        let task = make_task("Fallback command", None);
        let missing_path = std::path::PathBuf::from("/nonexistent/path.md");

        let argv = build_command(&task, Some(&missing_path));
        assert_eq!(argv[2], "Fallback command");
    }

    #[test]
    fn build_command_context_file_no_instructions_section() {
        let task = make_task("Original command", None);

        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("test.md");
        std::fs::write(
            &file_path,
            "# Title\n\nSome content without instructions section\n",
        )
        .unwrap();

        let argv = build_command(&task, Some(&file_path));
        assert_eq!(argv[2], "Original command");
    }

    #[test]
    fn read_instructions_extracts_section() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("test.md");
        let content = "<!-- intern:task-id=lc-test0001 intern:version=1 -->\n\n\
            # test-task\n\n\
            ## Automated Execution Notice\n\nSome notice\n\n\
            ## Execution Constraints\n\n| Constraint | Value |\n\n\
            ## Instructions\n\n\
            This is the actual instruction content.\n\
            With multiple lines.\n";
        std::fs::write(&file_path, content).unwrap();

        let instructions = read_instructions_from_context_file(&file_path);
        assert!(instructions.is_some());
        let text = instructions.unwrap();
        assert!(text.contains("This is the actual instruction content."));
        assert!(text.contains("With multiple lines."));
    }
}

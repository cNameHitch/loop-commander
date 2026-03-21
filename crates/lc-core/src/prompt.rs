//! AI prompt generation system for Loop Commander.
//!
//! This module provides:
//! - The meta-prompt template used to generate task prompts via Claude
//! - Agent registry data types
//! - Prompt validation for generated output
//! - Frontmatter parsing utilities

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Meta-Prompt Template ────────────────────────────────

/// The meta-prompt sent to Claude to generate a task prompt from user intent.
///
/// Template variables:
/// - `{{USER_INTENT}}`: The user's plain English description
/// - `{{AGENT_BLOCK}}`: Formatted agent descriptions or `(none)`
pub const META_PROMPT: &str = r#"You are a prompt engineering system for Loop Commander, a macOS scheduler that runs Claude Code tasks on a cron schedule. Your job is to transform a user's plain English intent into a production-ready prompt that Claude Code will execute autonomously.

## Inputs

**User Intent:**
{{USER_INTENT}}

**Tagged Agents:**
{{AGENT_BLOCK}}

## Instructions

Generate a complete Markdown file with YAML frontmatter and a prompt body. Follow these rules exactly:

### YAML Frontmatter

Include these fields:
- `name`: A kebab-case slug derived from the intent (max 50 chars, lowercase, hyphens only)
- `description`: A one-sentence summary of what the task does (max 200 chars)
- `tags`: An array of 1-5 lowercase tags relevant to the task domain
- `agents`: An array of the agent slugs provided in the Tagged Agents section (empty array if none)

### Prompt Body

Write the prompt body as the instructions that Claude Code will receive via `claude -p`. Structure it as follows:

1. **Role Statement** (1-2 sentences): Define what Claude is acting as for this task. If agents were tagged, reference them by name: "Acting as @agent-name, ..." to establish the persona and expertise.

2. **Objective** (1-3 sentences): State the primary goal clearly and unambiguously. Use imperative mood.

3. **Scope and Constraints**: A bulleted list of what to do and what NOT to do. Include:
   - File paths or patterns to target (if inferable from the intent)
   - What types of issues to look for or actions to take
   - Boundaries: what is out of scope
   - Time/resource awareness: this runs unattended, so avoid interactive prompts or commands that require user input

4. **Steps**: A numbered list of concrete actions Claude should take, in order. Each step should be a single, verifiable action.

5. **Output Format**: Specify what Claude should produce:
   - If the task is analytical (review, audit, monitor): structured findings as a Markdown summary
   - If the task is actionable (fix, update, create): describe the changes to make and how to report them
   - Always end with a one-paragraph summary of what was done

6. **Edge Cases**: A short bulleted list of "If X, then Y" rules for situations the task might encounter:
   - What to do if there is nothing to process (e.g., no open PRs, no errors found)
   - What to do if an error occurs during execution
   - What to do if the scope is ambiguous

### Quality Requirements

- The prompt must be self-contained. Claude Code receives ONLY this prompt text -- no additional context.
- Do not use placeholder values like "TODO" or "INSERT_HERE". Every instruction must be concrete.
- Do not include instructions to "ask the user" or "confirm with the user" -- this runs unattended.
- Reference each tagged agent with the @agent-name syntax in the Role Statement and wherever their specific expertise applies in the Steps section.
- The prompt must work for a general repository. Do not assume specific file names, languages, or frameworks unless the user's intent specifies them.

### Few-Shot Examples

**Example 1: Security-focused PR review with agents**

Input intent: "Review PRs for security issues"
Tagged agents: @security-auditor (Performs security audits on codebases), @code-reviewer (Reviews code for quality and correctness)

Output:
---
name: pr-security-review
description: Reviews open pull requests for security vulnerabilities and code quality issues
tags: [security, code-review, pull-requests]
agents: [security-auditor, code-reviewer]
---

Acting as @security-auditor and @code-reviewer, you are a security-focused code review specialist analyzing open pull requests for vulnerabilities and quality issues.

**Objective:** Review all open pull requests in this repository, focusing on security vulnerabilities first, then general code quality.

**Scope and Constraints:**
- Review only open, unmerged pull requests
- Focus on: injection flaws, authentication bypasses, data exposure, insecure dependencies, race conditions, and input validation gaps
- Also check for: logic errors, missing error handling, test coverage gaps
- Do NOT merge or approve any PR -- only analyze and comment
- Do NOT modify any code directly
- Skip draft PRs unless they have the "ready-for-review" label

**Steps:**
1. List all open pull requests using `gh pr list --state open --json number,title,headRefName,additions,deletions`.
2. For each PR, fetch the diff with `gh pr diff <number>`.
3. As @security-auditor, scan the diff for security vulnerabilities: check for hardcoded secrets, SQL injection vectors, XSS opportunities, insecure deserialization, path traversal, and dependency changes that introduce known CVEs.
4. As @code-reviewer, evaluate code quality: check for proper error handling, input validation, resource cleanup, and adherence to existing code patterns.
5. For each finding, categorize it as CRITICAL, HIGH, MEDIUM, or LOW severity.
6. Post a review comment on each PR with findings using `gh pr review <number> --comment --body "<findings>"`.
7. Produce a summary report of all PRs reviewed.

**Output Format:**
For each PR, produce a findings block:

### PR #<number>: <title>
- **Security Issues:** <count> (list with severity)
- **Quality Issues:** <count> (list with severity)
- **Recommendation:** APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION

End with a summary paragraph stating how many PRs were reviewed, total findings by severity, and any PRs requiring immediate attention.

**Edge Cases:**
- If there are no open PRs, report "No open pull requests found. Nothing to review." and exit cleanly.
- If a PR diff is too large (>5000 lines), focus only on security-critical files (auth, crypto, input handling, API routes) and note that a full review was not possible.
- If `gh` CLI is not authenticated, report the error and exit rather than proceeding with incomplete data.

**Example 2: Simple task without agents**

Input intent: "Check if tests pass every morning"
Tagged agents: (none)

Output:
---
name: morning-test-check
description: Runs the test suite and reports results with failure analysis
tags: [testing, ci, daily]
agents: []
---

You are a test suite monitor responsible for running and analyzing the project's test suite.

**Objective:** Execute the full test suite, analyze any failures, and produce a status report.

**Scope and Constraints:**
- Run the project's test command (detect from package.json, Cargo.toml, Makefile, or pyproject.toml)
- Analyze failures to distinguish between genuine bugs and flaky tests
- Do NOT fix any tests or modify code
- Do NOT push any changes

**Steps:**
1. Detect the project type by checking for package.json, Cargo.toml, go.mod, pyproject.toml, or Makefile in the working directory.
2. Run the appropriate test command (e.g., `npm test`, `cargo test`, `go test ./...`, `pytest`).
3. Capture stdout and stderr.
4. If all tests pass, produce a short success report.
5. If any tests fail, for each failure: extract the test name, the assertion or error message, and the file/line where it occurred.
6. Check git log for recent changes to the failing test files to identify likely causes.

**Output Format:**
## Test Report - [date]
- **Status:** PASS / FAIL
- **Total:** <n> tests, <p> passed, <f> failed, <s> skipped
- **Duration:** <time>

### Failures (if any)
For each failure:
- **Test:** <name>
- **Error:** <message>
- **File:** <path>:<line>
- **Likely Cause:** <analysis>

**Edge Cases:**
- If no test runner is detected, report "No test configuration found" and list what was checked.
- If the test command times out after 5 minutes, kill it and report a partial result.
- If tests require environment variables or services that are unavailable, report which dependencies are missing.

## Output

Emit ONLY the Markdown file content (frontmatter + body). Do not wrap it in a code fence. Do not add any commentary before or after the file content."#;

/// Substitute template variables in the meta-prompt.
pub fn build_meta_prompt(user_intent: &str, agents: &[AgentEntry]) -> String {
    let agent_block = if agents.is_empty() {
        "(none)".to_string()
    } else {
        agents
            .iter()
            .map(|a| format!("@{}: {}", a.slug, a.description))
            .collect::<Vec<_>>()
            .join("\n")
    };

    META_PROMPT
        .replace("{{USER_INTENT}}", user_intent)
        .replace("{{AGENT_BLOCK}}", &agent_block)
}

/// Build the meta-prompt with validation error feedback appended (for retry).
pub fn build_retry_meta_prompt(
    user_intent: &str,
    agents: &[AgentEntry],
    validation_errors: &[String],
) -> String {
    let mut prompt = build_meta_prompt(user_intent, agents);
    prompt.push_str("\n\nIMPORTANT: Your previous output failed validation for these reasons:\n");
    for err in validation_errors {
        prompt.push_str(&format!("- {err}\n"));
    }
    prompt.push_str("Please correct these issues in your next attempt.\n");
    prompt
}

// ── Agent Registry Types ────────────────────────────────

/// A single agent from the awesome-claude-code-subagents registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEntry {
    /// Slug identifier, e.g., "security-auditor".
    pub slug: String,
    /// Human-readable name, e.g., "Security Auditor".
    pub name: String,
    /// One-sentence description of what this agent does.
    pub description: String,
    /// Category grouping, e.g., "security", "testing", "devops".
    pub category: String,
}

/// The full cached agent registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegistry {
    /// When this registry was last fetched from GitHub.
    pub fetched_at: DateTime<Utc>,
    /// The list of available agents.
    pub agents: Vec<AgentEntry>,
}

impl AgentRegistry {
    /// Create an empty registry.
    pub fn empty() -> Self {
        Self {
            fetched_at: Utc::now(),
            agents: Vec::new(),
        }
    }

    /// Check if the registry is older than `max_age_hours`.
    pub fn is_stale(&self, max_age_hours: i64) -> bool {
        let age = Utc::now() - self.fetched_at;
        age.num_hours() >= max_age_hours
    }

    /// Look up agents by their slugs. Returns entries for slugs that were found.
    pub fn lookup_agents(&self, slugs: &[String]) -> Vec<AgentEntry> {
        slugs
            .iter()
            .filter_map(|slug| self.agents.iter().find(|a| a.slug == *slug).cloned())
            .collect()
    }
}

// ── Frontmatter Parsing ─────────────────────────────────

/// Parsed prompt file with frontmatter and body separated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPrompt {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub agents: Vec<String>,
    pub body: String,
}

/// Parse a Markdown file with YAML frontmatter into its constituent parts.
///
/// Expected format:
/// ```text
/// ---
/// name: some-name
/// description: Some description
/// tags: [tag1, tag2]
/// agents: [agent1]
/// ---
///
/// Prompt body here...
/// ```
pub fn parse_prompt_file(content: &str) -> Result<ParsedPrompt, String> {
    let content = content.trim();

    if !content.starts_with("---") {
        return Err("File does not start with frontmatter delimiter '---'".into());
    }

    // Find the closing delimiter
    let after_first = &content[3..];
    let closing_pos = after_first
        .find("\n---")
        .ok_or("No closing frontmatter delimiter '---' found")?;

    let frontmatter_str = &after_first[..closing_pos].trim();
    let body_start = 3 + closing_pos + 4; // skip "---" + "\n---"
    let body = if body_start < content.len() {
        content[body_start..].trim().to_string()
    } else {
        String::new()
    };

    // Parse YAML frontmatter
    let frontmatter: serde_yaml::Value = serde_yaml::from_str(frontmatter_str)
        .map_err(|e| format!("Failed to parse frontmatter YAML: {e}"))?;

    let name = frontmatter
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let description = frontmatter
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let tags = frontmatter
        .get("tags")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let agents = frontmatter
        .get("agents")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(ParsedPrompt {
        name,
        description,
        tags,
        agents,
        body,
    })
}

// ── Prompt Validation ───────────────────────────────────

/// Result of validating a generated prompt.
#[derive(Debug, Clone)]
pub struct PromptValidation {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub auto_fixes_applied: Vec<String>,
}

/// Validate the raw output from Claude's prompt generation.
///
/// Checks structural requirements and content quality. Returns a
/// `PromptValidation` indicating whether the output is usable.
pub fn validate_generated_prompt(
    raw_output: &str,
    expected_agents: &[String],
) -> (PromptValidation, Option<ParsedPrompt>) {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut auto_fixes = Vec::new();

    // 1. Parse frontmatter
    let parsed = match parse_prompt_file(raw_output) {
        Ok(p) => p,
        Err(e) => {
            errors.push(format!("Frontmatter: {e}"));
            return (
                PromptValidation {
                    is_valid: false,
                    errors,
                    warnings,
                    auto_fixes_applied: auto_fixes,
                },
                None,
            );
        }
    };

    let mut parsed = parsed;

    // 2. Validate name
    if parsed.name.is_empty() {
        errors.push("name: must not be empty".into());
    } else {
        if parsed.name.len() > 50 {
            errors.push("name: must be 50 characters or fewer".into());
        }
        if !parsed
            .name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit())
        {
            errors
                .push("name: must be kebab-case (lowercase letters, digits, hyphens only)".into());
        }
    }

    // 3. Validate description
    if parsed.description.is_empty() {
        errors.push("description: must not be empty".into());
    } else if parsed.description.len() > 200 {
        errors.push("description: must be 200 characters or fewer".into());
    }

    // 4. Validate tags (auto-fix if missing)
    if parsed.tags.is_empty() {
        parsed.tags = vec!["generated".to_string()];
        auto_fixes.push("tags: defaulted to [\"generated\"] because none were provided".into());
    } else if parsed.tags.len() > 5 {
        errors.push("tags: must have 1-5 tags".into());
    }

    // 5. Validate agents (auto-fix if missing but expected)
    if parsed.agents.is_empty() && !expected_agents.is_empty() {
        parsed.agents = expected_agents.to_vec();
        auto_fixes.push("agents: populated from input because none were in output".into());
    }

    // 6. Validate body
    if parsed.body.len() < 100 {
        errors.push("body: prompt is too short to be useful (minimum 100 characters)".into());
    }

    // 7. Check for placeholders
    let placeholder_patterns = ["TODO", "INSERT_HERE", "PLACEHOLDER"];
    for pattern in &placeholder_patterns {
        if parsed.body.contains(pattern) {
            errors.push(format!(
                "body: contains placeholder text '{pattern}' — all instructions must be concrete"
            ));
        }
    }

    // 8. Check for interactive instructions
    let interactive_patterns = ["ask the user", "confirm with", "wait for input"];
    for pattern in &interactive_patterns {
        if parsed.body.to_lowercase().contains(pattern) {
            errors.push(format!(
                "body: contains interactive instruction '{pattern}' — task runs unattended"
            ));
        }
    }

    // Warn about missing agent references
    for slug in expected_agents {
        if !parsed.body.contains(&format!("@{slug}")) {
            warnings.push(format!(
                "body: does not reference @{slug} — consider using agent expertise"
            ));
        }
    }

    let is_valid = errors.is_empty();

    (
        PromptValidation {
            is_valid,
            errors,
            warnings,
            auto_fixes_applied: auto_fixes,
        },
        Some(parsed),
    )
}

// ── Optimization Meta-Prompt ────────────────────────────

/// The meta-prompt sent to Claude to optimize a task's command based on
/// its execution history.
///
/// Template variables:
/// - `{{TASK_NAME}}`: The task's name
/// - `{{TASK_DESCRIPTION}}`: One-sentence description of the task
/// - `{{CURRENT_COMMAND}}`: The current command text
/// - `{{EXECUTION_HISTORY}}`: Formatted, truncated log entries with pattern annotations
/// - `{{AGENT_BLOCK}}`: Tagged agent descriptions, or an empty string if none
pub const OPTIMIZATION_META_PROMPT: &str =
    include_str!("../templates/optimization_meta_prompt.txt");

// ── Optimization Types ──────────────────────────────────

/// Which aspect of the prompt to focus the optimization on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationFocus {
    Efficiency,
    Quality,
    Consistency,
    Resilience,
    All,
}

impl Default for OptimizationFocus {
    fn default() -> Self {
        Self::All
    }
}

impl std::fmt::Display for OptimizationFocus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Efficiency => "efficiency",
            Self::Quality => "quality",
            Self::Consistency => "consistency",
            Self::Resilience => "resilience",
            Self::All => "all",
        };
        write!(f, "{s}")
    }
}

/// A category of optimization applied to the command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationCategory {
    Efficiency,
    Quality,
    Consistency,
    Resilience,
    Scope,
}

/// The structured output Claude returns for a prompt optimization request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationResult {
    /// The full optimized command text.
    pub optimized_command: String,
    /// Plain English description of what changed and why (max 200 words).
    pub changes_summary: String,
    /// Confidence score in [0, 100].
    pub confidence_score: u8,
    /// Which categories of optimization were applied.
    ///
    /// Kept as `Vec<String>` so Swift can receive the raw values and map them
    /// locally without requiring the Rust enum on the wire.
    pub optimization_categories: Vec<String>,
}

/// A condensed view of one execution log entry formatted for the optimization prompt.
#[derive(Debug, Clone, Serialize)]
pub struct LogSummary {
    /// Zero-based index of this run in the ordered history.
    pub run_index: u32,
    /// ISO-8601 timestamp when the run started.
    pub started_at: String,
    /// Wall-clock duration of the run in seconds.
    pub duration_secs: f64,
    /// Exit code of the process (0 = success).
    pub exit_code: i32,
    /// Human-readable status label (e.g., `"success"`, `"failure"`, `"timeout"`).
    pub status: String,
    /// Truncated stdout for prompt inclusion.
    pub stdout_excerpt: String,
    /// Truncated stderr for prompt inclusion.
    pub stderr_excerpt: String,
    /// Number of tokens consumed, if available.
    pub tokens_used: Option<u64>,
    /// Cost in USD, if available.
    pub cost_usd: Option<f64>,
}

// ── Optimization Helpers ────────────────────────────────

/// Character budgets for stdout/stderr truncation when building the optimization prompt.
pub const STDOUT_HEAD_CHARS: usize = 1_500;
/// Character budget for the tail of stdout (continuation after truncation marker).
pub const STDOUT_TAIL_CHARS: usize = 1_000;
/// Character budget for the head of stderr (error messages tend to be near the top).
pub const STDERR_HEAD_CHARS: usize = 1_000;
/// Character budget for the tail of stderr (exit reasons tend to appear at the end).
pub const STDERR_TAIL_CHARS: usize = 1_500;

/// Truncate a log excerpt for inclusion in the optimization prompt.
///
/// Uses asymmetric head/tail budgets: stdout is biased toward the head (first
/// output matters most for correctness signals), while stderr is biased toward
/// the tail (exit reasons and stack traces appear last).
///
/// # Examples
///
/// ```
/// use lc_core::prompt::truncate_log_for_prompt;
///
/// let short = "hello world";
/// assert_eq!(truncate_log_for_prompt(short, false), "hello world");
/// assert_eq!(truncate_log_for_prompt(short, true), "hello world");
/// ```
pub fn truncate_log_for_prompt(text: &str, is_stderr: bool) -> String {
    if is_stderr {
        truncate_for_prompt(text, STDERR_HEAD_CHARS, STDERR_TAIL_CHARS)
    } else {
        truncate_for_prompt(text, STDOUT_HEAD_CHARS, STDOUT_TAIL_CHARS)
    }
}

/// Truncate `text` to at most `head_chars + tail_chars` characters.
///
/// When the text is longer than the combined budget a mid-marker of the form
/// `\n... [truncated N chars] ...\n` is inserted between the head and tail
/// slices.  The function always splits on UTF-8 character boundaries.
///
/// # Examples
///
/// ```
/// use lc_core::prompt::truncate_for_prompt;
///
/// // Budget exceeds length: text returned unchanged.
/// let short = "hello world";
/// assert_eq!(truncate_for_prompt(short, 6, 6), "hello world");
///
/// // Budget smaller than text: head + marker + tail.
/// let long = "abcdefghij";
/// let result = truncate_for_prompt(long, 3, 3);
/// assert!(result.starts_with("abc"));
/// assert!(result.ends_with("hij"));
/// assert!(result.contains("truncated"));
/// ```
pub fn truncate_for_prompt(text: &str, head_chars: usize, tail_chars: usize) -> String {
    let total_budget = head_chars + tail_chars;
    let char_count = text.chars().count();

    if char_count <= total_budget {
        return text.to_string();
    }

    let truncated_count = char_count - total_budget;

    // Collect head slice up to `head_chars` chars.
    let head: String = text.chars().take(head_chars).collect();

    // Collect tail slice — last `tail_chars` chars.
    let tail: String = text
        .chars()
        .skip(char_count - tail_chars)
        .collect();

    format!("{head}\n... [truncated {truncated_count} chars] ...\n{tail}")
}

/// Format a single [`LogSummary`] entry as a human-readable block for the
/// optimization prompt.
///
/// The format follows the SSD section 7.2 conventions:
/// - Success runs show the `stdout_excerpt`.
/// - Failure runs show `stderr_excerpt` first, then `stdout_excerpt`.
/// - Timeout runs are annotated with a scope-reduction note.
///
/// # Examples
///
/// ```
/// use lc_core::prompt::{LogSummary, format_log_for_prompt};
///
/// let log = LogSummary {
///     run_index: 0,
///     started_at: "2026-03-18T10:00:00Z".into(),
///     duration_secs: 12.4,
///     exit_code: 0,
///     status: "success".into(),
///     stdout_excerpt: "No issues found.".into(),
///     stderr_excerpt: String::new(),
///     tokens_used: None,
///     cost_usd: Some(0.0031),
/// };
/// let formatted = format_log_for_prompt(&log);
/// assert!(formatted.contains("SUCCESS"));
/// assert!(formatted.contains("12.4s"));
/// ```
pub fn format_log_for_prompt(log: &LogSummary) -> String {
    let status_upper = log.status.to_uppercase();
    let cost_str = log
        .cost_usd
        .map(|c| format!("${c:.4}"))
        .unwrap_or_else(|| "—".to_string());

    let is_timeout = log.status.eq_ignore_ascii_case("timeout");
    let is_success = log.exit_code == 0 && !is_timeout;

    let header = if is_success {
        format!(
            "[RUN {:08}] {} | {} | {}s | {}",
            log.run_index, log.started_at, status_upper, log.duration_secs, cost_str
        )
    } else {
        format!(
            "[RUN {:08}] {} | {} (exit {}) | {}s | {}",
            log.run_index,
            log.started_at,
            status_upper,
            log.exit_code,
            log.duration_secs,
            cost_str
        )
    };

    let mut body = String::new();

    if is_success {
        if !log.stdout_excerpt.is_empty() {
            body.push_str(&format!("stdout: {}\n", log.stdout_excerpt));
        }
    } else if is_timeout {
        if !log.stdout_excerpt.is_empty() {
            body.push_str(&format!("stdout (first chars): {}\n", log.stdout_excerpt));
        }
        body.push_str("note: Run was terminated before completion. Consider scope reduction.\n");
    } else {
        // Failure run: show stderr first, then stdout.
        if !log.stderr_excerpt.is_empty() {
            body.push_str(&format!("stderr: {}\n", log.stderr_excerpt));
        }
        if !log.stdout_excerpt.is_empty() {
            body.push_str(&format!("stdout: {}\n", log.stdout_excerpt));
        }
    }

    if body.is_empty() {
        format!("{header}\n(no output captured)\n")
    } else {
        format!("{header}\n{body}")
    }
}

/// Compute a pre-annotation block summarising patterns across all log entries.
///
/// The block is prepended to the execution history section so Claude can
/// immediately see aggregate signals without re-deriving them.
///
/// # Examples
///
/// ```
/// use lc_core::prompt::{LogSummary, compute_pattern_annotations};
///
/// let logs = vec![
///     LogSummary {
///         run_index: 0,
///         started_at: "2026-03-18T10:00:00Z".into(),
///         duration_secs: 10.0,
///         exit_code: 0,
///         status: "success".into(),
///         stdout_excerpt: "ok".into(),
///         stderr_excerpt: String::new(),
///         tokens_used: Some(100),
///         cost_usd: Some(0.001),
///     },
/// ];
/// let annotations = compute_pattern_annotations(&logs);
/// assert!(annotations.contains("Success rate"));
/// ```
pub fn compute_pattern_annotations(logs: &[LogSummary]) -> String {
    if logs.is_empty() {
        return "## Pre-Computed Patterns\n(no logs available)\n".to_string();
    }

    let total = logs.len();
    let success_count = logs
        .iter()
        .filter(|l| l.exit_code == 0 && !l.status.eq_ignore_ascii_case("timeout"))
        .count();
    let failure_count = total - success_count;

    let success_label = match (success_count as f64 / total as f64 * 100.0) as u32 {
        90..=100 => "EXCELLENT",
        70..=89 => "GOOD",
        40..=69 => "MODERATE",
        _ => "POOR",
    };

    let mut lines: Vec<String> = Vec::new();
    lines.push("## Pre-Computed Patterns".to_string());
    lines.push(format!(
        "- Success rate: {success_count}/{total} runs ({:.0}%) — {success_label}",
        success_count as f64 / total as f64 * 100.0
    ));

    // Most common exit code on failure.
    if failure_count > 0 {
        let mut code_counts: std::collections::HashMap<i32, usize> =
            std::collections::HashMap::new();
        for log in logs.iter().filter(|l| l.exit_code != 0) {
            *code_counts.entry(log.exit_code).or_insert(0) += 1;
        }
        if let Some((&code, &count)) = code_counts.iter().max_by_key(|(_, v)| *v) {
            lines.push(format!(
                "- Most common exit code on failure: {code} ({count} of {failure_count} failure runs)"
            ));
        }
    }

    // Cost stats.
    let costs: Vec<f64> = logs.iter().filter_map(|l| l.cost_usd).collect();
    if !costs.is_empty() {
        let avg_cost = costs.iter().sum::<f64>() / costs.len() as f64;
        let max_cost = costs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let max_run = logs
            .iter()
            .filter_map(|l| l.cost_usd.map(|c| (c, l.run_index)))
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        if let Some((_, idx)) = max_run {
            lines.push(format!(
                "- Average cost: ${avg_cost:.4} | Highest: ${max_cost:.4} (RUN {:08})",
                idx
            ));
        } else {
            lines.push(format!(
                "- Average cost: ${avg_cost:.4} | Highest: ${max_cost:.4}"
            ));
        }
    }

    // Duration stats.
    let durations: Vec<f64> = logs.iter().map(|l| l.duration_secs).collect();
    if !durations.is_empty() {
        let avg_dur = durations.iter().sum::<f64>() / durations.len() as f64;
        let long_runs = durations.iter().filter(|&&d| d > 50.0).count();
        let mut dur_line = format!("- Average duration: {avg_dur:.1}s");
        if long_runs > 0 {
            dur_line.push_str(&format!(" | {long_runs} run(s) exceeded 50s"));
        }
        lines.push(dur_line);
    }

    // stdout length variance across success runs.
    let stdout_lens: Vec<usize> = logs
        .iter()
        .filter(|l| l.exit_code == 0)
        .map(|l| l.stdout_excerpt.len())
        .collect();
    if stdout_lens.len() >= 2 {
        let min_len = stdout_lens.iter().min().copied().unwrap_or(0);
        let max_len = stdout_lens.iter().max().copied().unwrap_or(0);
        let variance_label = if max_len > min_len * 3 { "HIGH" } else { "LOW" };
        lines.push(format!(
            "- stdout length variance: {variance_label} (min {min_len} chars, max {max_len} chars across success runs)"
        ));
    }

    lines.join("\n") + "\n"
}

/// Build the full optimization prompt by substituting all template variables.
///
/// `task_description` is used for the `{{TASK_DESCRIPTION}}` template variable.
/// When empty, `task_name` is used as the fallback description.
///
/// `agents` is a slice of agent slug strings (e.g. `&["security-auditor"]`).
/// When empty, the `{{AGENT_BLOCK}}` variable is replaced with a focus-only block.
/// When non-empty, a formatted `## Tagged Agents` section is inserted.
///
/// The `focus` value is embedded as a hint in the `{{AGENT_BLOCK}}` section so
/// Claude knows which category to prioritise.
///
/// # Examples
///
/// ```
/// use lc_core::prompt::{LogSummary, OptimizationFocus, build_optimization_prompt};
///
/// let logs = vec![LogSummary {
///     run_index: 0,
///     started_at: "2026-03-18T10:00:00Z".into(),
///     duration_secs: 5.0,
///     exit_code: 0,
///     status: "success".into(),
///     stdout_excerpt: "done".into(),
///     stderr_excerpt: String::new(),
///     tokens_used: None,
///     cost_usd: None,
/// }];
///
/// let prompt = build_optimization_prompt(
///     "my-task",
///     "Analyzes the codebase for issues",
///     "Review the codebase",
///     &OptimizationFocus::All,
///     &logs,
///     &[],
/// );
/// assert!(prompt.contains("my-task"));
/// assert!(prompt.contains("Analyzes the codebase for issues"));
/// assert!(prompt.contains("Review the codebase"));
/// assert!(!prompt.contains("{{TASK_NAME}}"));
/// assert!(!prompt.contains("{{TASK_DESCRIPTION}}"));
/// ```
pub fn build_optimization_prompt(
    task_name: &str,
    task_description: &str,
    original_command: &str,
    focus: &OptimizationFocus,
    logs: &[LogSummary],
    agents: &[&str],
) -> String {
    // Fallback: when description is blank, display the task name instead.
    let effective_description = if task_description.trim().is_empty() {
        task_name
    } else {
        task_description
    };

    // Build agent block.
    let agent_block = if agents.is_empty() {
        String::new()
    } else {
        let agent_list = agents
            .iter()
            .map(|s| format!("- @{s}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("## Tagged Agents\n\nOptimization focus: {focus}\n\n{agent_list}\n")
    };

    // Build execution history section.
    let pattern_annotations = compute_pattern_annotations(logs);
    let log_entries: String = logs.iter().map(format_log_for_prompt).collect::<Vec<_>>().join("\n");
    let execution_history = format!("{pattern_annotations}\n{log_entries}");

    // If no agents, still embed focus hint.
    let agent_block_final = if agent_block.is_empty() {
        format!("## Optimization Focus\n\n{focus}\n")
    } else {
        agent_block
    };

    OPTIMIZATION_META_PROMPT
        .replace("{{TASK_NAME}}", task_name)
        .replace("{{TASK_DESCRIPTION}}", effective_description)
        .replace("{{CURRENT_COMMAND}}", original_command)
        .replace("{{EXECUTION_HISTORY}}", &execution_history)
        .replace("{{AGENT_BLOCK}}", &agent_block_final)
}

/// The outcome of validating a prompt optimization result.
///
/// Combines the parsed [`OptimizationResult`] with any content-level warnings
/// detected during validation.  Warnings do not fail validation but should be
/// surfaced to callers for logging or inclusion in the RPC response.
#[derive(Debug, Clone)]
pub struct ValidationOutcome {
    /// The successfully parsed and structurally valid optimization result.
    pub result: OptimizationResult,
    /// Zero or more content-level warning messages.
    ///
    /// Conditions that produce a warning (spec §7.4):
    /// - `optimized_command` differs from the original yet `confidence_score` is 100.
    /// - `optimized_command` is more than 3× the length of the original (scope expansion).
    /// - `optimized_command` is less than 25% the length of the original (capability loss).
    pub warnings: Vec<String>,
}

/// Parse and validate Claude's raw JSON output from a prompt optimization request.
///
/// Returns `Ok(ValidationOutcome)` if the output passes all structural checks.
/// Returns `Err(String)` with a human-readable description of the first
/// validation failure encountered.
///
/// Structural (hard) checks performed:
/// - Output is valid JSON
/// - `optimized_command` is non-empty
/// - `confidence_score` is in [0, 100]
/// - `optimization_categories` is non-empty
/// - `changes_summary` is at least 20 characters
///
/// Content-level warnings (spec §7.4) are returned inside [`ValidationOutcome`]:
/// - Command changed but `confidence_score` is 100
/// - Optimized command > 3× original length (scope expansion)
/// - Optimized command < 25% of original length (capability loss)
///
/// # Examples
///
/// ```
/// use lc_core::prompt::validate_optimization_result;
///
/// let raw = r#"{
///   "optimized_command": "claude -p 'do the thing efficiently'",
///   "changes_summary": "Added efficiency constraints to reduce token usage.",
///   "confidence_score": 75,
///   "optimization_categories": ["efficiency"]
/// }"#;
///
/// let outcome = validate_optimization_result(raw, "claude -p 'do the thing'").unwrap();
/// assert_eq!(outcome.result.confidence_score, 75);
/// assert!(outcome.warnings.is_empty());
/// ```
pub fn validate_optimization_result(
    raw_output: &str,
    original_command: &str,
) -> Result<ValidationOutcome, String> {
    // Strip any leading/trailing whitespace and optional markdown fences.
    let trimmed = raw_output.trim();
    let json_str = if trimmed.starts_with("```") {
        // Strip ```json ... ``` fences.
        trimmed
            .lines()
            .skip(1) // skip opening fence
            .take_while(|l| !l.starts_with("```"))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        trimmed.to_string()
    };

    let value: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Output is not valid JSON: {e}"))?;

    // optimized_command
    let optimized_command = value
        .get("optimized_command")
        .and_then(|v| v.as_str())
        .ok_or("Missing or non-string field: optimized_command")?
        .to_string();
    if optimized_command.trim().is_empty() {
        return Err("optimized_command must not be empty".into());
    }

    // changes_summary
    let changes_summary = value
        .get("changes_summary")
        .and_then(|v| v.as_str())
        .ok_or("Missing or non-string field: changes_summary")?
        .to_string();
    if changes_summary.len() < 20 {
        return Err(format!(
            "changes_summary is too short ({} chars); minimum 20 required",
            changes_summary.len()
        ));
    }

    // confidence_score — validate range [0, 100] before narrowing to u8.
    let raw_score = value
        .get("confidence_score")
        .and_then(|v| v.as_u64())
        .ok_or("Missing or non-integer field: confidence_score")?;
    if raw_score > 100 {
        return Err(format!(
            "confidence_score {raw_score} is out of range; must be in [0, 100]"
        ));
    }
    #[allow(clippy::cast_possible_truncation)]
    let confidence_score = raw_score as u8;

    // optimization_categories
    let categories_raw = value
        .get("optimization_categories")
        .and_then(|v| v.as_array())
        .ok_or("Missing or non-array field: optimization_categories")?;
    if categories_raw.is_empty() {
        return Err("optimization_categories must not be empty".into());
    }
    let optimization_categories: Vec<String> = categories_raw
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    if optimization_categories.is_empty() {
        return Err("optimization_categories must contain at least one string value".into());
    }

    let result = OptimizationResult {
        optimized_command,
        changes_summary,
        confidence_score,
        optimization_categories,
    };

    // ── Content-level warnings (spec §7.4) ──────────────────────────────────
    let mut warnings: Vec<String> = Vec::new();

    let orig_len = original_command.len();
    let opt_len = result.optimized_command.len();
    let command_changed = result.optimized_command != original_command;

    if command_changed && confidence_score == 100 {
        warnings.push(
            "confidence_score is 100 but the command was changed; \
             a perfect confidence score is unlikely when modifications are made"
                .to_string(),
        );
    }

    if orig_len > 0 && opt_len > orig_len * 3 {
        warnings.push(format!(
            "optimized_command ({opt_len} chars) is more than 3× the original \
             ({orig_len} chars); review for unintended scope expansion"
        ));
    }

    if orig_len > 0 && opt_len * 4 < orig_len {
        warnings.push(format!(
            "optimized_command ({opt_len} chars) is less than 25% of the original \
             ({orig_len} chars); review for unintended capability loss"
        ));
    }

    Ok(ValidationOutcome { result, warnings })
}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Template Substitution ───────────────────────────

    #[test]
    fn build_meta_prompt_no_agents() {
        let prompt = build_meta_prompt("Check tests", &[]);
        assert!(prompt.contains("Check tests"));
        assert!(prompt.contains("(none)"));
        assert!(!prompt.contains("{{USER_INTENT}}"));
        assert!(!prompt.contains("{{AGENT_BLOCK}}"));
    }

    #[test]
    fn build_meta_prompt_with_agents() {
        let agents = vec![
            AgentEntry {
                slug: "security-auditor".into(),
                name: "Security Auditor".into(),
                description: "Performs security audits".into(),
                category: "security".into(),
            },
            AgentEntry {
                slug: "code-reviewer".into(),
                name: "Code Reviewer".into(),
                description: "Reviews code quality".into(),
                category: "quality".into(),
            },
        ];
        let prompt = build_meta_prompt("Review PRs", &agents);
        assert!(prompt.contains("Review PRs"));
        assert!(prompt.contains("@security-auditor: Performs security audits"));
        assert!(prompt.contains("@code-reviewer: Reviews code quality"));
    }

    #[test]
    fn build_retry_prompt_includes_errors() {
        let errors = vec![
            "name: must not be empty".to_string(),
            "body: too short".to_string(),
        ];
        let prompt = build_retry_meta_prompt("Test intent", &[], &errors);
        assert!(prompt.contains("IMPORTANT: Your previous output failed validation"));
        assert!(prompt.contains("- name: must not be empty"));
        assert!(prompt.contains("- body: too short"));
    }

    // ── Agent Block Formatting ──────────────────────────

    #[test]
    fn agent_block_single_agent() {
        let agents = vec![AgentEntry {
            slug: "test-agent".into(),
            name: "Test Agent".into(),
            description: "Does testing".into(),
            category: "testing".into(),
        }];
        let prompt = build_meta_prompt("intent", &agents);
        assert!(prompt.contains("@test-agent: Does testing"));
    }

    // ── Frontmatter Parsing ─────────────────────────────

    #[test]
    fn parse_valid_prompt_file() {
        let content = r#"---
name: test-task
description: A test task description
tags: [testing, ci]
agents: [security-auditor]
---

This is the prompt body with enough characters to pass validation.
It contains multiple lines and detailed instructions for Claude to follow."#;

        let parsed = parse_prompt_file(content).unwrap();
        assert_eq!(parsed.name, "test-task");
        assert_eq!(parsed.description, "A test task description");
        assert_eq!(parsed.tags, vec!["testing", "ci"]);
        assert_eq!(parsed.agents, vec!["security-auditor"]);
        assert!(parsed.body.starts_with("This is the prompt body"));
    }

    #[test]
    fn parse_no_frontmatter_fails() {
        let content = "Just some text without frontmatter";
        assert!(parse_prompt_file(content).is_err());
    }

    #[test]
    fn parse_missing_closing_delimiter_fails() {
        let content = "---\nname: test\nNo closing delimiter";
        assert!(parse_prompt_file(content).is_err());
    }

    #[test]
    fn parse_empty_agents_and_tags() {
        let content = r#"---
name: minimal
description: Minimal task
tags: []
agents: []
---

Body text here."#;

        let parsed = parse_prompt_file(content).unwrap();
        assert!(parsed.tags.is_empty());
        assert!(parsed.agents.is_empty());
    }

    // ── Validation ──────────────────────────────────────

    #[test]
    fn validate_valid_prompt() {
        let content = r#"---
name: valid-task
description: A valid test task
tags: [testing]
agents: []
---

This is a valid prompt body with enough content to pass the minimum length check.
It includes multiple lines of detailed instructions for autonomous execution by Claude."#;

        let (validation, parsed) = validate_generated_prompt(content, &[]);
        assert!(validation.is_valid, "Errors: {:?}", validation.errors);
        assert!(parsed.is_some());
    }

    #[test]
    fn validate_rejects_empty_name() {
        let content = r#"---
name: ""
description: A task
tags: [test]
agents: []
---

This is a prompt body with enough content to pass the minimum length check for validation purposes and more."#;

        let (validation, _) = validate_generated_prompt(content, &[]);
        assert!(!validation.is_valid);
        assert!(validation.errors.iter().any(|e| e.contains("name")));
    }

    #[test]
    fn validate_rejects_placeholder_text() {
        let content = r#"---
name: test-task
description: A task
tags: [test]
agents: []
---

This is a prompt body with TODO placeholders that should be filled in.
It has enough content to pass the length check but contains forbidden patterns."#;

        let (validation, _) = validate_generated_prompt(content, &[]);
        assert!(!validation.is_valid);
        assert!(validation.errors.iter().any(|e| e.contains("placeholder")));
    }

    #[test]
    fn validate_rejects_interactive_instructions() {
        let content = r#"---
name: test-task
description: A task
tags: [test]
agents: []
---

This prompt tells Claude to ask the user for confirmation before proceeding.
It has enough content to pass the length check but contains interactive patterns."#;

        let (validation, _) = validate_generated_prompt(content, &[]);
        assert!(!validation.is_valid);
        assert!(validation.errors.iter().any(|e| e.contains("interactive")));
    }

    #[test]
    fn validate_auto_fixes_missing_tags() {
        let content = r#"---
name: test-task
description: A task
tags: []
agents: []
---

This is a prompt body with enough content to pass the minimum length check for validation.
It includes sufficient detail for autonomous execution."#;

        let (validation, parsed) = validate_generated_prompt(content, &[]);
        assert!(validation.is_valid);
        assert!(!validation.auto_fixes_applied.is_empty());
        assert_eq!(parsed.unwrap().tags, vec!["generated"]);
    }

    #[test]
    fn validate_auto_fixes_missing_agents() {
        let content = r#"---
name: test-task
description: A task
tags: [test]
agents: []
---

Acting as @security-auditor, this prompt reviews code for security issues.
It has enough content to pass the minimum length check for validation purposes."#;

        let expected_agents = vec!["security-auditor".to_string()];
        let (validation, parsed) = validate_generated_prompt(content, &expected_agents);
        assert!(validation.is_valid);
        assert_eq!(parsed.unwrap().agents, vec!["security-auditor"]);
    }

    #[test]
    fn validate_warns_about_missing_agent_references() {
        let content = r#"---
name: test-task
description: A task
tags: [test]
agents: [security-auditor]
---

This prompt does not reference the security auditor agent anywhere in its body.
It has enough content to pass the minimum length check for validation purposes."#;

        let expected_agents = vec!["security-auditor".to_string()];
        let (validation, _) = validate_generated_prompt(content, &expected_agents);
        assert!(validation.is_valid); // warnings don't cause failure
        assert!(!validation.warnings.is_empty());
    }

    #[test]
    fn validate_rejects_short_body() {
        let content = r#"---
name: test-task
description: A task
tags: [test]
agents: []
---

Too short."#;

        let (validation, _) = validate_generated_prompt(content, &[]);
        assert!(!validation.is_valid);
        assert!(validation.errors.iter().any(|e| e.contains("too short")));
    }

    // ── Round-trip ──────────────────────────────────────

    #[test]
    fn roundtrip_parse_and_validate() {
        let content = r#"---
name: roundtrip-test
description: Tests that a parsed prompt can be validated
tags: [test, roundtrip]
agents: [code-reviewer]
---

Acting as @code-reviewer, you are responsible for reviewing code quality.

**Objective:** Analyze the codebase for quality issues and produce a report.

**Scope and Constraints:**
- Review all source files
- Do NOT modify any code

**Steps:**
1. List all source files
2. Analyze each file for quality issues
3. Produce a summary report

**Output Format:**
A Markdown report listing all findings.

**Edge Cases:**
- If no source files found, report and exit cleanly."#;

        let parsed = parse_prompt_file(content).unwrap();
        assert_eq!(parsed.name, "roundtrip-test");
        assert_eq!(parsed.tags, vec!["test", "roundtrip"]);
        assert_eq!(parsed.agents, vec!["code-reviewer"]);

        let (validation, _) = validate_generated_prompt(content, &["code-reviewer".to_string()]);
        assert!(validation.is_valid, "Errors: {:?}", validation.errors);
    }

    // ── AgentRegistry ───────────────────────────────────

    #[test]
    fn registry_staleness_check() {
        let mut registry = AgentRegistry::empty();
        assert!(!registry.is_stale(24));

        // Make it old
        registry.fetched_at = Utc::now() - chrono::Duration::hours(25);
        assert!(registry.is_stale(24));
    }

    #[test]
    fn registry_lookup_agents() {
        let registry = AgentRegistry {
            fetched_at: Utc::now(),
            agents: vec![
                AgentEntry {
                    slug: "security-auditor".into(),
                    name: "Security Auditor".into(),
                    description: "Audits security".into(),
                    category: "security".into(),
                },
                AgentEntry {
                    slug: "code-reviewer".into(),
                    name: "Code Reviewer".into(),
                    description: "Reviews code".into(),
                    category: "quality".into(),
                },
            ],
        };

        let found =
            registry.lookup_agents(&["security-auditor".to_string(), "nonexistent".to_string()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].slug, "security-auditor");
    }

    // ── Optimization Helpers ─────────────────────────────

    fn make_log(run_index: u32, exit_code: i32, status: &str, cost: Option<f64>) -> LogSummary {
        LogSummary {
            run_index,
            started_at: "2026-03-18T10:00:00Z".into(),
            duration_secs: 12.0,
            exit_code,
            status: status.into(),
            stdout_excerpt: "some output text".into(),
            stderr_excerpt: if exit_code != 0 {
                "error: something went wrong".into()
            } else {
                String::new()
            },
            tokens_used: Some(500),
            cost_usd: cost,
        }
    }

    #[test]
    fn truncate_short_text_unchanged() {
        let text = "hello world";
        assert_eq!(truncate_for_prompt(text, 20, 20), "hello world");
    }

    #[test]
    fn truncate_long_text_inserts_marker() {
        // 10 chars total; head=3, tail=3 → should truncate 4 chars in the middle
        let text = "abcdefghij";
        let result = truncate_for_prompt(text, 3, 3);
        assert!(result.starts_with("abc"), "got: {result}");
        assert!(result.ends_with("hij"), "got: {result}");
        assert!(result.contains("truncated 4 chars"), "got: {result}");
    }

    #[test]
    fn truncate_exact_budget_unchanged() {
        let text = "abcdef";
        // head=3 + tail=3 = 6 == len → no truncation needed
        assert_eq!(truncate_for_prompt(text, 3, 3), "abcdef");
    }

    // ── truncate_log_for_prompt ──────────────────────────

    #[test]
    fn truncate_log_for_prompt_short_stdout_unchanged() {
        let text = "short output";
        // Well under the stdout budget of 1500+1000 = 2500 chars.
        assert_eq!(truncate_log_for_prompt(text, false), "short output");
    }

    #[test]
    fn truncate_log_for_prompt_short_stderr_unchanged() {
        let text = "error: file not found";
        // Well under the stderr budget of 1000+1500 = 2500 chars.
        assert_eq!(truncate_log_for_prompt(text, true), "error: file not found");
    }

    #[test]
    fn truncate_log_for_prompt_long_stdout_uses_head_bias() {
        // Stdout should use STDOUT_HEAD_CHARS (1500) + STDOUT_TAIL_CHARS (1000).
        // Create text that exceeds the total budget.
        let text: String = "A".repeat(STDOUT_HEAD_CHARS + STDOUT_TAIL_CHARS + 200);
        let result = truncate_log_for_prompt(&text, false);
        // Head must be exactly STDOUT_HEAD_CHARS 'A's.
        let head: String = result.chars().take(STDOUT_HEAD_CHARS).collect();
        assert_eq!(head, "A".repeat(STDOUT_HEAD_CHARS), "stdout head size mismatch");
        assert!(result.contains("truncated 200 chars"), "marker missing; got: {result}");
    }

    #[test]
    fn truncate_log_for_prompt_long_stderr_uses_tail_bias() {
        // Stderr should use STDERR_HEAD_CHARS (1000) + STDERR_TAIL_CHARS (1500).
        let text: String = "B".repeat(STDERR_HEAD_CHARS + STDERR_TAIL_CHARS + 300);
        let result = truncate_log_for_prompt(&text, true);
        // Head must be exactly STDERR_HEAD_CHARS 'B's.
        let head: String = result.chars().take(STDERR_HEAD_CHARS).collect();
        assert_eq!(head, "B".repeat(STDERR_HEAD_CHARS), "stderr head size mismatch");
        assert!(result.contains("truncated 300 chars"), "marker missing; got: {result}");
    }

    #[test]
    fn format_log_success_contains_header_fields() {
        let log = make_log(0, 0, "success", Some(0.0031));
        let formatted = format_log_for_prompt(&log);
        assert!(formatted.contains("SUCCESS"), "got: {formatted}");
        assert!(formatted.contains("12"), "got: {formatted}");
        assert!(formatted.contains("$0.0031"), "got: {formatted}");
        assert!(!formatted.contains("exit"), "success run should not show exit code");
    }

    #[test]
    fn format_log_failure_shows_exit_code_and_stderr() {
        let log = make_log(1, 1, "failure", None);
        let formatted = format_log_for_prompt(&log);
        assert!(formatted.contains("FAILURE"), "got: {formatted}");
        assert!(formatted.contains("exit 1"), "got: {formatted}");
        assert!(formatted.contains("error: something went wrong"), "got: {formatted}");
    }

    #[test]
    fn format_log_timeout_adds_note() {
        let mut log = make_log(2, 1, "timeout", None);
        log.exit_code = 1;
        log.status = "timeout".into();
        let formatted = format_log_for_prompt(&log);
        assert!(formatted.contains("TIMEOUT"), "got: {formatted}");
        assert!(
            formatted.contains("scope reduction"),
            "got: {formatted}"
        );
    }

    #[test]
    fn compute_pattern_annotations_empty() {
        let annotations = compute_pattern_annotations(&[]);
        assert!(annotations.contains("no logs available"));
    }

    #[test]
    fn compute_pattern_annotations_all_success() {
        let logs = vec![
            make_log(0, 0, "success", Some(0.001)),
            make_log(1, 0, "success", Some(0.002)),
        ];
        let annotations = compute_pattern_annotations(&logs);
        assert!(annotations.contains("2/2 runs"), "got: {annotations}");
        assert!(annotations.contains("EXCELLENT"), "got: {annotations}");
    }

    #[test]
    fn compute_pattern_annotations_mixed() {
        let logs = vec![
            make_log(0, 0, "success", Some(0.001)),
            make_log(1, 1, "failure", Some(0.002)),
            make_log(2, 1, "failure", Some(0.003)),
        ];
        let annotations = compute_pattern_annotations(&logs);
        assert!(annotations.contains("1/3 runs"), "got: {annotations}");
        assert!(annotations.contains("Average cost"), "got: {annotations}");
    }

    #[test]
    fn build_optimization_prompt_substitutes_all_variables() {
        let logs = vec![make_log(0, 0, "success", Some(0.001))];
        let prompt = build_optimization_prompt(
            "test-task",
            "Runs stuff efficiently",
            "claude -p 'do stuff'",
            &OptimizationFocus::Efficiency,
            &logs,
            &[],
        );
        assert!(!prompt.contains("{{TASK_NAME}}"), "unresolved TASK_NAME");
        assert!(!prompt.contains("{{TASK_DESCRIPTION}}"), "unresolved TASK_DESCRIPTION");
        assert!(!prompt.contains("{{CURRENT_COMMAND}}"), "unresolved CURRENT_COMMAND");
        assert!(!prompt.contains("{{EXECUTION_HISTORY}}"), "unresolved EXECUTION_HISTORY");
        assert!(!prompt.contains("{{AGENT_BLOCK}}"), "unresolved AGENT_BLOCK");
        assert!(prompt.contains("test-task"), "task name missing");
        assert!(prompt.contains("Runs stuff efficiently"), "task description missing");
        assert!(prompt.contains("claude -p 'do stuff'"), "command missing");
    }

    #[test]
    fn build_optimization_prompt_description_fallback() {
        // When task_description is empty, task_name is used as the description.
        let logs = vec![make_log(0, 0, "success", None)];
        let prompt = build_optimization_prompt(
            "my-task",
            "",
            "run it",
            &OptimizationFocus::All,
            &logs,
            &[],
        );
        // The task name should appear twice: once in TASK_NAME and once as TASK_DESCRIPTION.
        let occurrences = prompt.matches("my-task").count();
        assert!(occurrences >= 2, "expected task name used as description fallback; got occurrences={occurrences}");
        assert!(!prompt.contains("{{TASK_DESCRIPTION}}"), "unresolved TASK_DESCRIPTION");
    }

    #[test]
    fn build_optimization_prompt_includes_agents() {
        let logs = vec![make_log(0, 0, "success", None)];
        let prompt = build_optimization_prompt(
            "review-task",
            "Reviews code quality",
            "review code",
            &OptimizationFocus::Quality,
            &logs,
            &["security-auditor", "code-reviewer"],
        );
        assert!(prompt.contains("@security-auditor"), "got: {prompt}");
        assert!(prompt.contains("@code-reviewer"), "got: {prompt}");
    }

    #[test]
    fn validate_optimization_result_valid_json() {
        let raw = r#"{
          "optimized_command": "claude -p 'do the thing efficiently'",
          "changes_summary": "Added efficiency constraints to reduce token usage significantly.",
          "confidence_score": 75,
          "optimization_categories": ["efficiency"]
        }"#;
        let outcome = validate_optimization_result(raw, "claude -p 'do the thing'");
        assert!(outcome.is_ok(), "got error: {:?}", outcome.err());
        let o = outcome.unwrap();
        assert_eq!(o.result.confidence_score, 75);
        assert_eq!(o.result.optimization_categories, vec!["efficiency"]);
    }

    #[test]
    fn validate_optimization_result_strips_markdown_fence() {
        let raw = "```json\n{\
            \"optimized_command\": \"run it better\",\
            \"changes_summary\": \"This is a sufficient summary of changes made to the prompt.\",\
            \"confidence_score\": 80,\
            \"optimization_categories\": [\"quality\"]\
        }\n```";
        let outcome = validate_optimization_result(raw, "run it");
        assert!(outcome.is_ok(), "got error: {:?}", outcome.err());
    }

    #[test]
    fn validate_optimization_result_rejects_empty_command() {
        let raw = r#"{
          "optimized_command": "",
          "changes_summary": "This has enough characters to pass the summary length check.",
          "confidence_score": 50,
          "optimization_categories": ["efficiency"]
        }"#;
        let err = validate_optimization_result(raw, "original").unwrap_err();
        assert!(err.contains("optimized_command"), "got: {err}");
    }

    #[test]
    fn validate_optimization_result_rejects_short_summary() {
        let raw = r#"{
          "optimized_command": "something valid here",
          "changes_summary": "Too short.",
          "confidence_score": 60,
          "optimization_categories": ["quality"]
        }"#;
        let err = validate_optimization_result(raw, "original").unwrap_err();
        assert!(err.contains("changes_summary"), "got: {err}");
    }

    #[test]
    fn validate_optimization_result_rejects_empty_categories() {
        let raw = r#"{
          "optimized_command": "something valid here",
          "changes_summary": "This is a long enough summary to pass the minimum length check.",
          "confidence_score": 60,
          "optimization_categories": []
        }"#;
        let err = validate_optimization_result(raw, "original").unwrap_err();
        assert!(err.contains("optimization_categories"), "got: {err}");
    }

    #[test]
    fn validate_optimization_result_rejects_non_json() {
        let raw = "This is not JSON at all.";
        let err = validate_optimization_result(raw, "original").unwrap_err();
        assert!(err.contains("not valid JSON"), "got: {err}");
    }

    // ── GAP-01: confidence_score bounds check ────────────────────────────────

    #[test]
    fn validate_optimization_result_rejects_score_above_100() {
        let raw = r#"{
          "optimized_command": "claude -p 'do the thing efficiently'",
          "changes_summary": "Added efficiency constraints to reduce token usage significantly.",
          "confidence_score": 101,
          "optimization_categories": ["efficiency"]
        }"#;
        let err = validate_optimization_result(raw, "claude -p 'do the thing'").unwrap_err();
        assert!(
            err.contains("confidence_score") && err.contains("out of range"),
            "got: {err}"
        );
    }

    #[test]
    fn validate_optimization_result_accepts_score_100() {
        let raw = r#"{
          "optimized_command": "claude -p 'do the thing'",
          "changes_summary": "No changes were necessary; the command is already optimal.",
          "confidence_score": 100,
          "optimization_categories": ["efficiency"]
        }"#;
        // Same command → no "changed but score 100" warning.
        let outcome = validate_optimization_result(raw, "claude -p 'do the thing'").unwrap();
        assert_eq!(outcome.result.confidence_score, 100);
        assert!(outcome.warnings.is_empty(), "unexpected warnings: {:?}", outcome.warnings);
    }

    #[test]
    fn validate_optimization_result_accepts_score_0() {
        let raw = r#"{
          "optimized_command": "claude -p 'do the thing'",
          "changes_summary": "No changes were made due to lack of data for analysis.",
          "confidence_score": 0,
          "optimization_categories": ["efficiency"]
        }"#;
        let outcome = validate_optimization_result(raw, "claude -p 'do the thing'").unwrap();
        assert_eq!(outcome.result.confidence_score, 0);
    }

    // ── GAP-02: content-level regression warnings ────────────────────────────

    #[test]
    fn validate_optimization_result_warns_changed_command_with_score_100() {
        let raw = r#"{
          "optimized_command": "claude -p 'do something completely different'",
          "changes_summary": "Rewrote the command for better efficiency and reliability.",
          "confidence_score": 100,
          "optimization_categories": ["efficiency"]
        }"#;
        let outcome =
            validate_optimization_result(raw, "claude -p 'original command'").unwrap();
        assert!(
            outcome.warnings.iter().any(|w| w.contains("confidence_score is 100")),
            "expected 'changed but score 100' warning; got: {:?}",
            outcome.warnings
        );
    }

    #[test]
    fn validate_optimization_result_warns_scope_expansion() {
        // Build an optimized command that is more than 3× the original length.
        let original = "short";
        let expanded = "A".repeat(original.len() * 4); // 4× — exceeds threshold
        let raw = serde_json::json!({
            "optimized_command": expanded,
            "changes_summary": "Expanded the command significantly to cover all edge cases and scenarios.",
            "confidence_score": 70,
            "optimization_categories": ["quality"]
        })
        .to_string();
        let outcome = validate_optimization_result(&raw, original).unwrap();
        assert!(
            outcome.warnings.iter().any(|w| w.contains("scope expansion")),
            "expected scope expansion warning; got: {:?}",
            outcome.warnings
        );
    }

    #[test]
    fn validate_optimization_result_warns_capability_loss() {
        // Build an optimized command that is less than 25% of the original length.
        let original = "A".repeat(100);
        let shrunk = "x"; // 1% of original
        let raw = serde_json::json!({
            "optimized_command": shrunk,
            "changes_summary": "Drastically reduced the command to eliminate redundancy and verbosity.",
            "confidence_score": 60,
            "optimization_categories": ["efficiency"]
        })
        .to_string();
        let outcome = validate_optimization_result(&raw, &original).unwrap();
        assert!(
            outcome.warnings.iter().any(|w| w.contains("capability loss")),
            "expected capability loss warning; got: {:?}",
            outcome.warnings
        );
    }

    #[test]
    fn validate_optimization_result_no_warnings_for_normal_change() {
        // A modest change at confidence 75 should produce no warnings.
        let original = "claude -p 'review the code'";
        let optimized = "claude -p 'review the code carefully for security issues'";
        let raw = serde_json::json!({
            "optimized_command": optimized,
            "changes_summary": "Added specificity to focus the review on security concerns.",
            "confidence_score": 75,
            "optimization_categories": ["quality"]
        })
        .to_string();
        let outcome = validate_optimization_result(&raw, original).unwrap();
        assert!(outcome.warnings.is_empty(), "unexpected warnings: {:?}", outcome.warnings);
    }
}

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
            errors.push("name: must be kebab-case (lowercase letters, digits, hyphens only)".into());
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
        assert!(validation
            .errors
            .iter()
            .any(|e| e.contains("placeholder")));
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
        assert!(validation
            .errors
            .iter()
            .any(|e| e.contains("interactive")));
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

        let (validation, _) =
            validate_generated_prompt(content, &["code-reviewer".to_string()]);
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

        let found = registry.lookup_agents(&[
            "security-auditor".to_string(),
            "nonexistent".to_string(),
        ]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].slug, "security-auditor");
    }
}

pub mod builtin_agents;
pub mod prompt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

// ── Default helpers ─────────────────────────────────────

/// Default budget per run in USD.
pub fn default_budget() -> f64 {
    5.0
}

/// Default task timeout in seconds.
pub fn default_timeout() -> u64 {
    600
}

// ── JSON-RPC Error Codes (CC-8) ─────────────────────────

/// Standard JSON-RPC 2.0 error codes plus Intern application codes.
pub mod rpc_errors {
    /// Task not found in configuration.
    pub const TASK_NOT_FOUND: i32 = -32001;
    /// Input validation failed (details in `data` field).
    pub const VALIDATION_ERROR: i32 = -32002;
    /// launchd scheduler operation failed.
    pub const SCHEDULER_ERROR: i32 = -32003;
    /// SQLite database operation failed.
    pub const DATABASE_ERROR: i32 = -32004;
    /// Daemon is busy or a resource lock could not be acquired.
    pub const DAEMON_BUSY: i32 = -32005;
    /// Daily or per-run budget cap exceeded.
    pub const BUDGET_EXCEEDED: i32 = -32006;
}

// ── Task ID ─────────────────────────────────────────────

/// Unique task identifier with format `lc-XXXXXXXX` (8 hex chars from UUID v4).
///
/// Collision risk is approximately 1 in 4 billion, which is acceptable for a
/// single-user local scheduler.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl TaskId {
    /// Generate a new random task ID.
    pub fn new() -> Self {
        let id = uuid::Uuid::new_v4().simple().to_string();
        Self(format!("lc-{}", &id[..8]))
    }

    /// Return the ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Return the launchd label: `com.intern.task.lc-xxxxxxxx`.
    pub fn launchd_label(&self) -> String {
        format!("com.intern.task.{}", self.0)
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Schedule ────────────────────────────────────────────

/// How a task is scheduled to run.
///
/// Uses a tagged enum with `#[serde(tag = "type")]` so the JSON representation
/// includes a `"type"` discriminator field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Schedule {
    /// Standard 5-field cron expression, e.g. `"*/15 * * * *"`.
    Cron { expression: String },
    /// launchd `StartInterval` in seconds.
    Interval { seconds: u64 },
    /// launchd `StartCalendarInterval` fields.
    Calendar {
        minute: Option<u8>,
        hour: Option<u8>,
        day: Option<u8>,
        weekday: Option<u8>,
        month: Option<u8>,
    },
}

impl Schedule {
    /// Return a human-readable description of the schedule.
    pub fn to_human(&self) -> String {
        match self {
            Schedule::Cron { expression } => format!("Cron: {expression}"),
            Schedule::Interval { seconds } => {
                if *seconds < 60 {
                    format!("Every {seconds}s")
                } else if *seconds < 3600 {
                    format!("Every {}m", seconds / 60)
                } else {
                    format!("Every {}h", seconds / 3600)
                }
            }
            Schedule::Calendar {
                minute,
                hour,
                weekday,
                ..
            } => {
                let time = match (hour, minute) {
                    (Some(h), Some(m)) => format!("{h:02}:{m:02}"),
                    (Some(h), None) => format!("{h:02}:00"),
                    _ => "every interval".to_string(),
                };
                match weekday {
                    Some(d) => {
                        let day_name = match d {
                            0 | 7 => "Sun",
                            1 => "Mon",
                            2 => "Tue",
                            3 => "Wed",
                            4 => "Thu",
                            5 => "Fri",
                            6 => "Sat",
                            _ => "?",
                        };
                        format!("{day_name}s at {time}")
                    }
                    None => format!("Daily at {time}"),
                }
            }
        }
    }
}

// ── Task Status ─────────────────────────────────────────

/// The operational status of a task.
///
/// `Running` was added per requirement R9 to distinguish actively-executing
/// tasks from merely `Active` (scheduled) ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Active,
    Paused,
    Error,
    Disabled,
    Running,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Active => write!(f, "active"),
            TaskStatus::Paused => write!(f, "paused"),
            TaskStatus::Error => write!(f, "error"),
            TaskStatus::Disabled => write!(f, "disabled"),
            TaskStatus::Running => write!(f, "running"),
        }
    }
}

// ── Task ────────────────────────────────────────────────

/// A scheduled Claude Code task with all configuration fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub name: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    pub schedule: Schedule,
    pub schedule_human: String,
    pub working_dir: PathBuf,
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
    #[serde(default = "default_budget")]
    pub max_budget_per_run: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<u32>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    pub status: TaskStatus,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Agent slugs used to generate this task's prompt.
    #[serde(default)]
    pub agents: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Execution Log ───────────────────────────────────────

/// The outcome of a single task execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecStatus {
    Success,
    Failed,
    Timeout,
    Killed,
    Skipped,
}

impl std::fmt::Display for ExecStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecStatus::Success => write!(f, "success"),
            ExecStatus::Failed => write!(f, "failed"),
            ExecStatus::Timeout => write!(f, "timeout"),
            ExecStatus::Killed => write!(f, "killed"),
            ExecStatus::Skipped => write!(f, "skipped"),
        }
    }
}

impl std::str::FromStr for ExecStatus {
    type Err = InternError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "success" => Ok(ExecStatus::Success),
            "failed" => Ok(ExecStatus::Failed),
            "timeout" => Ok(ExecStatus::Timeout),
            "killed" => Ok(ExecStatus::Killed),
            "skipped" => Ok(ExecStatus::Skipped),
            _ => Err(InternError::InvalidStatus(s.to_string())),
        }
    }
}

/// A single execution log entry persisted in SQLite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionLog {
    pub id: i64,
    pub task_id: String,
    pub task_name: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub duration_secs: u64,
    pub exit_code: i32,
    pub status: ExecStatus,
    pub stdout: String,
    pub stderr: String,
    pub tokens_used: Option<u64>,
    pub cost_usd: Option<f64>,
    /// Whether the cost figure was estimated from duration rather than extracted
    /// from Claude Code output.
    #[serde(default)]
    pub cost_is_estimate: bool,
    pub summary: String,
}

// ── Dashboard Metrics ───────────────────────────────────

/// Aggregated metrics for a single task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMetrics {
    pub task_id: String,
    pub total_runs: u64,
    pub success_count: u64,
    pub fail_count: u64,
    pub total_cost: f64,
    pub total_tokens: u64,
    pub avg_duration_secs: f64,
    pub last_run: Option<DateTime<Utc>>,
}

/// Per-day cost aggregate for sparkline charts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyCost {
    /// Date string in `YYYY-MM-DD` format.
    pub date: String,
    pub total_cost: f64,
    pub run_count: u64,
}

/// Dashboard-level aggregated metrics returned by `metrics.dashboard`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardMetrics {
    pub total_tasks: u64,
    pub active_tasks: u64,
    pub total_runs: u64,
    pub overall_success_rate: f64,
    pub total_spend: f64,
    pub tasks: Vec<TaskMetrics>,
    /// Last 7 days of cost data, ordered oldest to newest.
    #[serde(default)]
    pub cost_trend: Vec<DailyCost>,
}

// ── IPC Messages (JSON-RPC 2.0) ─────────────────────────

/// A JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
    pub id: serde_json::Value,
}

/// A JSON-RPC 2.0 response (either success or error, never both).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: serde_json::Value,
}

/// The error object inside a JSON-RPC 2.0 error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    /// Build a successful response wrapping the given result value.
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Build an error response with the given code and message.
    pub fn error(id: serde_json::Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
            id,
        }
    }
}

// ── Create/Update DTOs ──────────────────────────────────

/// Input for creating a new task via `task.create`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskInput {
    pub name: String,
    pub command: String,
    pub skill: Option<String>,
    pub schedule: Schedule,
    pub schedule_human: Option<String>,
    pub working_dir: String,
    pub env_vars: Option<HashMap<String, String>>,
    pub max_budget_per_run: Option<f64>,
    pub max_turns: Option<u32>,
    pub timeout_secs: Option<u64>,
    pub tags: Option<Vec<String>>,
    pub agents: Option<Vec<String>>,
}

impl CreateTaskInput {
    /// Validate all fields according to CC-3 rules.
    ///
    /// Returns `Ok(())` when valid, or `Err(errors)` with a list of human-readable
    /// validation failure descriptions.
    ///
    /// # Rules
    ///
    /// - `name`: non-empty, max 200 chars, no control characters
    /// - `command`: non-empty
    /// - `max_budget_per_run`: > 0 and <= 100.0
    /// - `timeout_secs`: > 0 and <= 86400
    /// - `tags`: each max 50 chars, max 20 tags
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // name
        if self.name.is_empty() {
            errors.push("name: must not be empty".to_string());
        }
        if self.name.len() > 200 {
            errors.push("name: must be 200 characters or fewer".to_string());
        }
        if self.name.chars().any(|c| c.is_control()) {
            errors.push("name: must not contain control characters".to_string());
        }

        // command
        if self.command.is_empty() {
            errors.push("command: must not be empty".to_string());
        }

        // budget
        if let Some(budget) = self.max_budget_per_run {
            if budget <= 0.0 {
                errors.push("max_budget_per_run: must be greater than 0".to_string());
            }
            if budget > 100.0 {
                errors.push("max_budget_per_run: must be 100.0 or less".to_string());
            }
        }

        // timeout
        if let Some(timeout) = self.timeout_secs {
            if timeout == 0 {
                errors.push("timeout_secs: must be greater than 0".to_string());
            }
            if timeout > 86400 {
                errors.push("timeout_secs: must be 86400 or less".to_string());
            }
        }

        // tags
        if let Some(tags) = &self.tags {
            if tags.len() > 20 {
                errors.push("tags: must have 20 or fewer tags".to_string());
            }
            for tag in tags {
                if tag.len() > 50 {
                    errors.push(format!("tags: tag '{}' exceeds 50 characters", tag));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Input for partially updating an existing task via `task.update`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTaskInput {
    pub id: String,
    pub name: Option<String>,
    pub command: Option<String>,
    pub skill: Option<String>,
    pub schedule: Option<Schedule>,
    pub schedule_human: Option<String>,
    pub working_dir: Option<String>,
    pub env_vars: Option<HashMap<String, String>>,
    pub max_budget_per_run: Option<f64>,
    pub max_turns: Option<u32>,
    pub timeout_secs: Option<u64>,
    pub tags: Option<Vec<String>>,
    pub agents: Option<Vec<String>>,
    pub status: Option<TaskStatus>,
}

impl UpdateTaskInput {
    /// Validate all provided fields according to CC-3 rules.
    ///
    /// Only validates fields that are `Some`. Returns `Ok(())` when valid, or
    /// `Err(errors)` with a list of human-readable validation failure descriptions.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // name
        if let Some(name) = &self.name {
            if name.is_empty() {
                errors.push("name: must not be empty".to_string());
            }
            if name.len() > 200 {
                errors.push("name: must be 200 characters or fewer".to_string());
            }
            if name.chars().any(|c| c.is_control()) {
                errors.push("name: must not contain control characters".to_string());
            }
        }

        // command
        if let Some(command) = &self.command {
            if command.is_empty() {
                errors.push("command: must not be empty".to_string());
            }
        }

        // budget
        if let Some(budget) = self.max_budget_per_run {
            if budget <= 0.0 {
                errors.push("max_budget_per_run: must be greater than 0".to_string());
            }
            if budget > 100.0 {
                errors.push("max_budget_per_run: must be 100.0 or less".to_string());
            }
        }

        // timeout
        if let Some(timeout) = self.timeout_secs {
            if timeout == 0 {
                errors.push("timeout_secs: must be greater than 0".to_string());
            }
            if timeout > 86400 {
                errors.push("timeout_secs: must be 86400 or less".to_string());
            }
        }

        // tags
        if let Some(tags) = &self.tags {
            if tags.len() > 20 {
                errors.push("tags: must have 20 or fewer tags".to_string());
            }
            for tag in tags {
                if tag.len() > 50 {
                    errors.push(format!("tags: tag '{}' exceeds 50 characters", tag));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Query parameters for `logs.query`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogQuery {
    pub task_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub search: Option<String>,
}

// ── Errors ──────────────────────────────────────────────

/// All error variants used across the Intern crate ecosystem.
#[derive(Debug, Error)]
pub enum InternError {
    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Scheduler error: {0}")]
    Scheduler(String),

    #[error("Runner error: {0}")]
    Runner(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid status: {0}")]
    InvalidStatus(String),

    #[error("Budget exceeded: task {task_id} spent ${spent:.2}, limit ${limit:.2}")]
    BudgetExceeded {
        task_id: String,
        spent: f64,
        limit: f64,
    },

    #[error("Daemon not running")]
    DaemonNotRunning,

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Validation error: {0}")]
    Validation(String),
}

// ── Paths ───────────────────────────────────────────────

/// All filesystem paths used by Intern, rooted at `~/.intern/`.
///
/// The socket is kept under the user's home directory (not `/tmp/`) to prevent
/// symlink attacks on shared machines (CC-6).
pub struct InternPaths {
    pub root: PathBuf,
    pub config_file: PathBuf,
    pub tasks_dir: PathBuf,
    pub plists_dir: PathBuf,
    pub output_dir: PathBuf,
    pub db_file: PathBuf,
    /// Generated prompt files: `~/.intern/prompts/`.
    pub prompts_dir: PathBuf,
    /// Unix domain socket: `~/.intern/daemon.sock` (NOT `/tmp/`).
    pub socket_path: PathBuf,
    pub pid_file: PathBuf,
    pub launch_agents_dir: PathBuf,
}

impl InternPaths {
    /// Create a new `InternPaths` anchored at the current user's home directory.
    ///
    /// # Panics
    ///
    /// Panics if the home directory cannot be determined.
    pub fn new() -> Self {
        let home = dirs::home_dir().expect("No home directory");
        let root = home.join(".intern");
        Self {
            config_file: root.join("config.yaml"),
            tasks_dir: root.join("tasks"),
            plists_dir: root.join("plists"),
            output_dir: root.join("output"),
            prompts_dir: root.join("prompts"),
            db_file: root.join("logs.db"),
            // SECURITY: Using /tmp is vulnerable to symlink attacks and other users
            // on shared machines. Use a user-scoped path under the data dir instead.
            socket_path: root.join("daemon.sock"),
            pid_file: root.join("daemon.pid"),
            launch_agents_dir: home.join("Library/LaunchAgents"),
            root,
        }
    }

    /// Create paths relative to a custom root directory (useful in tests).
    pub fn with_root(root: PathBuf) -> Self {
        Self {
            config_file: root.join("config.yaml"),
            tasks_dir: root.join("tasks"),
            plists_dir: root.join("plists"),
            output_dir: root.join("output"),
            prompts_dir: root.join("prompts"),
            db_file: root.join("logs.db"),
            socket_path: root.join("daemon.sock"),
            pid_file: root.join("daemon.pid"),
            launch_agents_dir: root.join("LaunchAgents"),
            root,
        }
    }

    /// Create all required directories if they do not exist.
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(&self.tasks_dir)?;
        std::fs::create_dir_all(&self.plists_dir)?;
        std::fs::create_dir_all(&self.output_dir)?;
        std::fs::create_dir_all(&self.prompts_dir)?;
        Ok(())
    }
}

impl Default for InternPaths {
    fn default() -> Self {
        Self::new()
    }
}

// ── Utility ─────────────────────────────────────────────

/// Expand a leading `~` to the user's home directory.
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix('~') {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest.strip_prefix('/').unwrap_or(rest));
        }
    }
    PathBuf::from(path)
}

// ── Task Templates (N1) ────────────────────────────────

/// A built-in task template that pre-fills common configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTemplate {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub command: String,
    pub schedule: Schedule,
    pub schedule_human: String,
    pub max_budget_per_run: f64,
    pub tags: Vec<String>,
}

/// Return the five built-in task templates.
///
/// Defined as a function rather than a `const` because `Schedule` and `String`
/// require heap allocation.
pub fn builtin_templates() -> Vec<TaskTemplate> {
    vec![
        TaskTemplate {
            slug: "pr-review".into(),
            name: "PR Review Sweep".into(),
            description: "Review all open PRs for logic errors, missing tests, and style issues"
                .into(),
            command: "claude -p 'Review all open PRs in this repo. Check for logic errors, \
                      missing tests, and style violations. Auto-fix what you can, leave \
                      comments on what you cannot.'"
                .into(),
            schedule: Schedule::Interval { seconds: 7200 },
            schedule_human: "Every 2 hours".into(),
            max_budget_per_run: 5.0,
            tags: vec!["code-review".into(), "automation".into()],
        },
        TaskTemplate {
            slug: "error-monitor".into(),
            name: "Error Log Monitor".into(),
            description: "Scan application logs for new errors and create issues".into(),
            command: "claude -p 'Check the application logs for new errors since the last \
                      run. Summarize each unique error, suggest fixes, and create GitHub \
                      issues for critical ones.'"
                .into(),
            schedule: Schedule::Interval { seconds: 3600 },
            schedule_human: "Every hour".into(),
            max_budget_per_run: 3.0,
            tags: vec!["monitoring".into(), "errors".into()],
        },
        TaskTemplate {
            slug: "morning-briefing".into(),
            name: "Morning Briefing".into(),
            description: "Generate a daily summary of repo activity, open PRs, and failing CI"
                .into(),
            command: "claude -p 'Generate a morning briefing: summarize commits from the \
                      last 24h, list open PRs needing review, and report any failing CI \
                      pipelines.'"
                .into(),
            schedule: Schedule::Calendar {
                minute: Some(0),
                hour: Some(7),
                day: None,
                weekday: None,
                month: None,
            },
            schedule_human: "Daily at 07:00".into(),
            max_budget_per_run: 2.0,
            tags: vec!["reporting".into(), "daily".into()],
        },
        TaskTemplate {
            slug: "dependency-audit".into(),
            name: "Dependency Audit".into(),
            description: "Scan for outdated or vulnerable dependencies".into(),
            command: "claude -p 'Audit all project dependencies. Identify outdated packages, \
                      known CVEs, and suggest safe upgrade paths. Create a summary report.'"
                .into(),
            schedule: Schedule::Calendar {
                minute: Some(0),
                hour: Some(0),
                day: None,
                weekday: None,
                month: None,
            },
            schedule_human: "Daily at midnight".into(),
            max_budget_per_run: 4.0,
            tags: vec!["security".into(), "dependencies".into()],
        },
        TaskTemplate {
            slug: "test-health".into(),
            name: "Test Health Check".into(),
            description: "Track flaky tests and test coverage trends nightly".into(),
            command: "claude -p 'Run the test suite, identify flaky tests that passed/failed \
                      inconsistently, analyze coverage trends, and suggest tests for uncovered \
                      code paths.'"
                .into(),
            schedule: Schedule::Calendar {
                minute: Some(0),
                hour: Some(2),
                day: None,
                weekday: None,
                month: None,
            },
            schedule_human: "Daily at 02:00".into(),
            max_budget_per_run: 5.0,
            tags: vec!["testing".into(), "quality".into()],
        },
    ]
}

// ── Dry Run Result (N5) ────────────────────────────────

/// Result of a task dry run: shows what would happen without executing anything.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunResult {
    pub task_id: String,
    pub task_name: String,
    /// The argv array that would be passed to `tokio::process::Command`.
    pub resolved_command: Vec<String>,
    pub working_dir: String,
    pub env_vars: HashMap<String, String>,
    pub timeout_secs: u64,
    pub max_budget_per_run: f64,
    pub daily_spend_so_far: f64,
    pub would_be_skipped: bool,
    pub skip_reason: Option<String>,
    pub schedule_human: String,
}

// ── Task Export (N4) ────────────────────────────────────

/// Portable task definition for import/export, omitting runtime-only fields
/// (`id`, `status`, `created_at`, `updated_at`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExport {
    pub version: u32,
    pub name: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    pub schedule: Schedule,
    pub schedule_human: String,
    /// Kept as `String` (not `PathBuf`) for cross-machine portability.
    pub working_dir: String,
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
    pub max_budget_per_run: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<u32>,
    pub timeout_secs: u64,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub agents: Vec<String>,
}

impl From<&Task> for TaskExport {
    fn from(task: &Task) -> Self {
        Self {
            version: 1,
            name: task.name.clone(),
            command: task.command.clone(),
            skill: task.skill.clone(),
            schedule: task.schedule.clone(),
            schedule_human: task.schedule_human.clone(),
            working_dir: task.working_dir.to_string_lossy().into_owned(),
            env_vars: task.env_vars.clone(),
            max_budget_per_run: task.max_budget_per_run,
            max_turns: task.max_turns,
            timeout_secs: task.timeout_secs,
            tags: task.tags.clone(),
            agents: task.agents.clone(),
        }
    }
}

impl From<TaskExport> for CreateTaskInput {
    fn from(export: TaskExport) -> Self {
        Self {
            name: export.name,
            command: export.command,
            skill: export.skill,
            schedule: export.schedule,
            schedule_human: Some(export.schedule_human),
            working_dir: export.working_dir,
            env_vars: Some(export.env_vars),
            max_budget_per_run: Some(export.max_budget_per_run),
            max_turns: export.max_turns,
            timeout_secs: Some(export.timeout_secs),
            tags: Some(export.tags),
            agents: Some(export.agents),
        }
    }
}

// ── Daemon Events ───────────────────────────────────────

/// Events that can be pushed to subscribed clients (Swift app, CLI).
///
/// Serialized as a tagged enum with `"type"` discriminator and `"data"` content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DaemonEvent {
    /// A task execution started (intern-runner spawned).
    TaskStarted { task_id: String, task_name: String },
    /// A task execution completed successfully.
    TaskCompleted {
        task_id: String,
        task_name: String,
        duration_secs: u64,
        cost_usd: Option<f64>,
    },
    /// A task execution failed.
    TaskFailed {
        task_id: String,
        task_name: String,
        exit_code: i32,
        summary: String,
    },
    /// A task's status changed (paused, resumed, error, etc.).
    TaskStatusChanged {
        task_id: String,
        old_status: String,
        new_status: String,
    },
    /// Health check repaired a launchd discrepancy.
    HealthRepair { task_id: String, action: String },
    /// A task was skipped due to budget cap.
    BudgetExceeded {
        task_id: String,
        task_name: String,
        daily_spend: f64,
        cap: f64,
    },
}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- TaskId -----------------------------------------------------------

    #[test]
    fn task_id_format() {
        let id = TaskId::new();
        let s = id.as_str();
        assert!(s.starts_with("lc-"), "must start with 'lc-': {s}");
        assert_eq!(s.len(), 11, "lc- + 8 hex = 11: {s}");
        assert!(
            s[3..].chars().all(|c| c.is_ascii_hexdigit()),
            "suffix must be hex: {}",
            &s[3..]
        );
    }

    #[test]
    fn task_id_display() {
        let id = TaskId("lc-abcd1234".into());
        assert_eq!(format!("{id}"), "lc-abcd1234");
    }

    #[test]
    fn task_id_launchd_label() {
        let id = TaskId("lc-abcdef01".into());
        assert_eq!(id.launchd_label(), "com.intern.task.lc-abcdef01");
    }

    // -- Schedule ---------------------------------------------------------

    #[test]
    fn schedule_to_human_cron() {
        let s = Schedule::Cron {
            expression: "*/15 * * * *".into(),
        };
        assert_eq!(s.to_human(), "Cron: */15 * * * *");
    }

    #[test]
    fn schedule_to_human_interval_seconds() {
        assert_eq!(Schedule::Interval { seconds: 30 }.to_human(), "Every 30s");
    }

    #[test]
    fn schedule_to_human_interval_minutes() {
        assert_eq!(Schedule::Interval { seconds: 900 }.to_human(), "Every 15m");
    }

    #[test]
    fn schedule_to_human_interval_hours() {
        assert_eq!(Schedule::Interval { seconds: 7200 }.to_human(), "Every 2h");
    }

    #[test]
    fn schedule_to_human_calendar_daily() {
        let s = Schedule::Calendar {
            minute: Some(0),
            hour: Some(7),
            day: None,
            weekday: None,
            month: None,
        };
        assert_eq!(s.to_human(), "Daily at 07:00");
    }

    #[test]
    fn schedule_to_human_calendar_weekday() {
        let s = Schedule::Calendar {
            minute: Some(30),
            hour: Some(9),
            day: None,
            weekday: Some(1),
            month: None,
        };
        assert_eq!(s.to_human(), "Mons at 09:30");
    }

    #[test]
    fn schedule_serde_tagged() {
        let cron = Schedule::Cron {
            expression: "0 * * * *".into(),
        };
        let json = serde_json::to_string(&cron).unwrap();
        assert!(json.contains(r#""type":"cron""#));
        let roundtrip: Schedule = serde_json::from_str(&json).unwrap();
        assert_eq!(cron, roundtrip);
    }

    // -- TaskStatus -------------------------------------------------------

    #[test]
    fn task_status_display() {
        assert_eq!(format!("{}", TaskStatus::Active), "active");
        assert_eq!(format!("{}", TaskStatus::Paused), "paused");
        assert_eq!(format!("{}", TaskStatus::Error), "error");
        assert_eq!(format!("{}", TaskStatus::Disabled), "disabled");
        assert_eq!(format!("{}", TaskStatus::Running), "running");
    }

    // -- ExecStatus -------------------------------------------------------

    #[test]
    fn exec_status_display() {
        assert_eq!(format!("{}", ExecStatus::Success), "success");
        assert_eq!(format!("{}", ExecStatus::Failed), "failed");
        assert_eq!(format!("{}", ExecStatus::Timeout), "timeout");
        assert_eq!(format!("{}", ExecStatus::Killed), "killed");
        assert_eq!(format!("{}", ExecStatus::Skipped), "skipped");
    }

    #[test]
    fn exec_status_from_str() {
        for (text, expected) in [
            ("success", ExecStatus::Success),
            ("failed", ExecStatus::Failed),
            ("timeout", ExecStatus::Timeout),
            ("killed", ExecStatus::Killed),
            ("skipped", ExecStatus::Skipped),
        ] {
            let parsed: ExecStatus = text.parse().unwrap();
            assert_eq!(parsed, expected);
        }
        assert!("invalid".parse::<ExecStatus>().is_err());
    }

    // -- Task serde -------------------------------------------------------

    #[test]
    fn task_serde_roundtrip() {
        let task = Task {
            id: TaskId("lc-12345678".into()),
            name: "Test Task".into(),
            command: "echo hello".into(),
            skill: None,
            schedule: Schedule::Interval { seconds: 300 },
            schedule_human: "Every 5m".into(),
            working_dir: PathBuf::from("/tmp"),
            env_vars: HashMap::new(),
            max_budget_per_run: 5.0,
            max_turns: Some(50),
            timeout_secs: 600,
            status: TaskStatus::Active,
            tags: vec!["test".into()],
            agents: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&task).unwrap();
        let roundtrip: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(task, roundtrip);
    }

    // -- ExecutionLog serde -----------------------------------------------

    #[test]
    fn execution_log_serde_roundtrip() {
        let log = ExecutionLog {
            id: 1,
            task_id: "lc-12345678".into(),
            task_name: "Test".into(),
            started_at: Utc::now(),
            finished_at: Utc::now(),
            duration_secs: 42,
            exit_code: 0,
            status: ExecStatus::Success,
            stdout: "output".into(),
            stderr: String::new(),
            tokens_used: Some(1000),
            cost_usd: Some(0.05),
            cost_is_estimate: false,
            summary: "done".into(),
        };
        let json = serde_json::to_string(&log).unwrap();
        let roundtrip: ExecutionLog = serde_json::from_str(&json).unwrap();
        assert_eq!(log, roundtrip);
    }

    // -- JsonRpcResponse --------------------------------------------------

    #[test]
    fn json_rpc_response_success() {
        let resp = JsonRpcResponse::success(serde_json::json!(1), serde_json::json!({"ok": true}));
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
        assert_eq!(resp.id, serde_json::json!(1));
        assert_eq!(resp.result.unwrap(), serde_json::json!({"ok": true}));
    }

    #[test]
    fn json_rpc_response_error() {
        let resp = JsonRpcResponse::error(
            serde_json::json!(42),
            rpc_errors::TASK_NOT_FOUND,
            "Task not found: lc-deadbeef".into(),
        );
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32001);
        assert_eq!(err.message, "Task not found: lc-deadbeef");
    }

    #[test]
    fn json_rpc_response_serde_roundtrip() {
        let resp =
            JsonRpcResponse::success(serde_json::json!("req-1"), serde_json::json!({"tasks": []}));
        let json = serde_json::to_string(&resp).unwrap();
        let roundtrip: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.jsonrpc, "2.0");
        assert!(roundtrip.result.is_some());
        assert!(roundtrip.error.is_none());
    }

    // -- Validation -------------------------------------------------------

    #[test]
    fn validation_valid_input_passes() {
        let input = CreateTaskInput {
            name: "Test Task".into(),
            command: "echo hello".into(),
            skill: None,
            schedule: Schedule::Interval { seconds: 300 },
            schedule_human: None,
            working_dir: "/tmp".into(),
            env_vars: None,
            max_budget_per_run: Some(5.0),
            max_turns: None,
            timeout_secs: Some(600),
            tags: Some(vec!["test".into()]),
            agents: None,
        };
        assert!(input.validate().is_ok());
    }

    #[test]
    fn validation_empty_name_fails() {
        let input = CreateTaskInput {
            name: String::new(),
            command: "echo hello".into(),
            skill: None,
            schedule: Schedule::Interval { seconds: 300 },
            schedule_human: None,
            working_dir: "/tmp".into(),
            env_vars: None,
            max_budget_per_run: None,
            max_turns: None,
            timeout_secs: None,
            tags: None,
            agents: None,
        };
        let errors = input.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("name")));
    }

    #[test]
    fn validation_name_too_long_fails() {
        let input = CreateTaskInput {
            name: "x".repeat(201),
            command: "echo hello".into(),
            skill: None,
            schedule: Schedule::Interval { seconds: 300 },
            schedule_human: None,
            working_dir: "/tmp".into(),
            env_vars: None,
            max_budget_per_run: None,
            max_turns: None,
            timeout_secs: None,
            tags: None,
            agents: None,
        };
        let errors = input.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("200")));
    }

    #[test]
    fn validation_control_chars_in_name_fails() {
        let input = CreateTaskInput {
            name: "bad\x00name".into(),
            command: "echo hello".into(),
            skill: None,
            schedule: Schedule::Interval { seconds: 300 },
            schedule_human: None,
            working_dir: "/tmp".into(),
            env_vars: None,
            max_budget_per_run: None,
            max_turns: None,
            timeout_secs: None,
            tags: None,
            agents: None,
        };
        let errors = input.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("control")));
    }

    #[test]
    fn validation_empty_command_fails() {
        let input = CreateTaskInput {
            name: "Test".into(),
            command: String::new(),
            skill: None,
            schedule: Schedule::Interval { seconds: 300 },
            schedule_human: None,
            working_dir: "/tmp".into(),
            env_vars: None,
            max_budget_per_run: None,
            max_turns: None,
            timeout_secs: None,
            tags: None,
            agents: None,
        };
        let errors = input.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("command")));
    }

    #[test]
    fn validation_budget_zero_fails() {
        let input = CreateTaskInput {
            name: "Test".into(),
            command: "echo hello".into(),
            skill: None,
            schedule: Schedule::Interval { seconds: 300 },
            schedule_human: None,
            working_dir: "/tmp".into(),
            env_vars: None,
            max_budget_per_run: Some(0.0),
            max_turns: None,
            timeout_secs: None,
            tags: None,
            agents: None,
        };
        let errors = input.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("budget")));
    }

    #[test]
    fn validation_budget_too_high_fails() {
        let input = CreateTaskInput {
            name: "Test".into(),
            command: "echo hello".into(),
            skill: None,
            schedule: Schedule::Interval { seconds: 300 },
            schedule_human: None,
            working_dir: "/tmp".into(),
            env_vars: None,
            max_budget_per_run: Some(200.0),
            max_turns: None,
            timeout_secs: None,
            tags: None,
            agents: None,
        };
        let errors = input.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("100")));
    }

    #[test]
    fn validation_timeout_zero_fails() {
        let input = CreateTaskInput {
            name: "Test".into(),
            command: "echo hello".into(),
            skill: None,
            schedule: Schedule::Interval { seconds: 300 },
            schedule_human: None,
            working_dir: "/tmp".into(),
            env_vars: None,
            max_budget_per_run: None,
            max_turns: None,
            timeout_secs: Some(0),
            tags: None,
            agents: None,
        };
        let errors = input.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("timeout")));
    }

    #[test]
    fn validation_timeout_too_high_fails() {
        let input = CreateTaskInput {
            name: "Test".into(),
            command: "echo hello".into(),
            skill: None,
            schedule: Schedule::Interval { seconds: 300 },
            schedule_human: None,
            working_dir: "/tmp".into(),
            env_vars: None,
            max_budget_per_run: None,
            max_turns: None,
            timeout_secs: Some(100_000),
            tags: None,
            agents: None,
        };
        let errors = input.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("86400")));
    }

    #[test]
    fn validation_too_many_tags_fails() {
        let tags: Vec<String> = (0..25).map(|i| format!("tag-{i}")).collect();
        let input = CreateTaskInput {
            name: "Test".into(),
            command: "echo hello".into(),
            skill: None,
            schedule: Schedule::Interval { seconds: 300 },
            schedule_human: None,
            working_dir: "/tmp".into(),
            env_vars: None,
            max_budget_per_run: None,
            max_turns: None,
            timeout_secs: None,
            tags: Some(tags),
            agents: None,
        };
        let errors = input.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("20")));
    }

    #[test]
    fn validation_tag_too_long_fails() {
        let input = CreateTaskInput {
            name: "Test".into(),
            command: "echo hello".into(),
            skill: None,
            schedule: Schedule::Interval { seconds: 300 },
            schedule_human: None,
            working_dir: "/tmp".into(),
            env_vars: None,
            max_budget_per_run: None,
            max_turns: None,
            timeout_secs: None,
            tags: Some(vec!["x".repeat(51)]),
            agents: None,
        };
        let errors = input.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("50")));
    }

    #[test]
    fn validation_multiple_errors_collected() {
        let input = CreateTaskInput {
            name: String::new(),
            command: String::new(),
            skill: None,
            schedule: Schedule::Interval { seconds: 300 },
            schedule_human: None,
            working_dir: "/tmp".into(),
            env_vars: None,
            max_budget_per_run: Some(0.0),
            max_turns: None,
            timeout_secs: Some(0),
            tags: None,
            agents: None,
        };
        let errors = input.validate().unwrap_err();
        assert!(
            errors.len() >= 4,
            "should collect all errors, got: {errors:?}"
        );
    }

    #[test]
    fn update_validation_empty_is_ok() {
        let input = UpdateTaskInput {
            id: "lc-aabbccdd".into(),
            name: None,
            command: None,
            skill: None,
            schedule: None,
            schedule_human: None,
            working_dir: None,
            env_vars: None,
            max_budget_per_run: None,
            max_turns: None,
            timeout_secs: None,
            tags: None,
            agents: None,
            status: None,
        };
        assert!(input.validate().is_ok());
    }

    #[test]
    fn update_validation_empty_name_fails() {
        let input = UpdateTaskInput {
            id: "lc-aabbccdd".into(),
            name: Some(String::new()),
            command: None,
            skill: None,
            schedule: None,
            schedule_human: None,
            working_dir: None,
            env_vars: None,
            max_budget_per_run: None,
            max_turns: None,
            timeout_secs: None,
            tags: None,
            agents: None,
            status: None,
        };
        let errors = input.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("name")));
    }

    // -- Templates --------------------------------------------------------

    #[test]
    fn builtin_templates_count_and_slugs() {
        let templates = builtin_templates();
        assert_eq!(templates.len(), 5);
        let slugs: Vec<&str> = templates.iter().map(|t| t.slug.as_str()).collect();
        assert!(slugs.contains(&"pr-review"));
        assert!(slugs.contains(&"error-monitor"));
        assert!(slugs.contains(&"morning-briefing"));
        assert!(slugs.contains(&"dependency-audit"));
        assert!(slugs.contains(&"test-health"));
    }

    // -- TaskExport -------------------------------------------------------

    #[test]
    fn task_export_from_task() {
        let task = Task {
            id: TaskId("lc-aabbccdd".into()),
            name: "Export Me".into(),
            command: "echo export".into(),
            skill: None,
            schedule: Schedule::Interval { seconds: 60 },
            schedule_human: "Every 1m".into(),
            working_dir: PathBuf::from("/home/user"),
            env_vars: HashMap::new(),
            max_budget_per_run: 3.0,
            max_turns: None,
            timeout_secs: 300,
            status: TaskStatus::Active,
            tags: vec!["export".into()],
            agents: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let export = TaskExport::from(&task);
        assert_eq!(export.version, 1);
        assert_eq!(export.name, "Export Me");
        assert_eq!(export.working_dir, "/home/user");
    }

    #[test]
    fn task_export_to_create_input() {
        let export = TaskExport {
            version: 1,
            name: "Imported".into(),
            command: "echo import".into(),
            skill: None,
            schedule: Schedule::Interval { seconds: 120 },
            schedule_human: "Every 2m".into(),
            working_dir: "/tmp".into(),
            env_vars: HashMap::new(),
            max_budget_per_run: 2.0,
            max_turns: Some(5),
            timeout_secs: 100,
            tags: vec!["imported".into()],
            agents: vec![],
        };
        let input = CreateTaskInput::from(export);
        assert_eq!(input.name, "Imported");
        assert_eq!(input.command, "echo import");
        assert_eq!(input.max_budget_per_run, Some(2.0));
        assert_eq!(input.max_turns, Some(5));
    }

    // -- DaemonEvent serde ------------------------------------------------

    #[test]
    fn daemon_event_serde_roundtrip() {
        let event = DaemonEvent::TaskStarted {
            task_id: "lc-aabbccdd".into(),
            task_name: "Test".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("TaskStarted"));
        let roundtrip: DaemonEvent = serde_json::from_str(&json).unwrap();
        match roundtrip {
            DaemonEvent::TaskStarted { task_id, task_name } => {
                assert_eq!(task_id, "lc-aabbccdd");
                assert_eq!(task_name, "Test");
            }
            _ => panic!("expected TaskStarted variant"),
        }
    }

    // -- RPC error codes --------------------------------------------------

    #[test]
    fn rpc_error_constants() {
        assert_eq!(rpc_errors::TASK_NOT_FOUND, -32001);
        assert_eq!(rpc_errors::VALIDATION_ERROR, -32002);
        assert_eq!(rpc_errors::SCHEDULER_ERROR, -32003);
        assert_eq!(rpc_errors::DATABASE_ERROR, -32004);
        assert_eq!(rpc_errors::DAEMON_BUSY, -32005);
        assert_eq!(rpc_errors::BUDGET_EXCEEDED, -32006);
    }

    // -- InternError ----------------------------------------------------------

    #[test]
    fn lc_error_display() {
        let err = InternError::TaskNotFound("lc-deadbeef".into());
        assert_eq!(format!("{err}"), "Task not found: lc-deadbeef");

        let err = InternError::BudgetExceeded {
            task_id: "lc-aabb".into(),
            spent: 10.50,
            limit: 5.00,
        };
        assert_eq!(
            format!("{err}"),
            "Budget exceeded: task lc-aabb spent $10.50, limit $5.00"
        );

        let err = InternError::DaemonNotRunning;
        assert_eq!(format!("{err}"), "Daemon not running");
    }

    // -- Defaults ---------------------------------------------------------

    #[test]
    fn default_budget_value() {
        assert!((default_budget() - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn default_timeout_value() {
        assert_eq!(default_timeout(), 600);
    }

    // -- DailyCost --------------------------------------------------------

    #[test]
    fn daily_cost_serde() {
        let dc = DailyCost {
            date: "2026-03-15".into(),
            total_cost: 12.34,
            run_count: 5,
        };
        let json = serde_json::to_string(&dc).unwrap();
        let roundtrip: DailyCost = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.date, "2026-03-15");
        assert!((roundtrip.total_cost - 12.34).abs() < f64::EPSILON);
        assert_eq!(roundtrip.run_count, 5);
    }

    // -- DaemonEvent additional serde tests -----------------------------------

    #[test]
    fn daemon_event_budget_exceeded_serde_roundtrip() {
        let event = DaemonEvent::BudgetExceeded {
            task_id: "lc-aabbccdd".into(),
            task_name: "Budget Task".into(),
            daily_spend: 4.75,
            cap: 5.0,
        };
        let json = serde_json::to_string(&event).unwrap();
        let roundtrip: DaemonEvent = serde_json::from_str(&json).unwrap();
        match roundtrip {
            DaemonEvent::BudgetExceeded {
                task_id,
                task_name,
                daily_spend,
                cap,
            } => {
                assert_eq!(task_id, "lc-aabbccdd");
                assert_eq!(task_name, "Budget Task");
                assert!((daily_spend - 4.75).abs() < f64::EPSILON);
                assert!((cap - 5.0).abs() < f64::EPSILON);
            }
            _ => panic!("expected BudgetExceeded variant"),
        }
    }

    #[test]
    fn daemon_event_health_repair_serde_roundtrip() {
        let event = DaemonEvent::HealthRepair {
            task_id: "lc-11223344".into(),
            action: "re-registered launchd agent".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let roundtrip: DaemonEvent = serde_json::from_str(&json).unwrap();
        match roundtrip {
            DaemonEvent::HealthRepair { task_id, action } => {
                assert_eq!(task_id, "lc-11223344");
                assert_eq!(action, "re-registered launchd agent");
            }
            _ => panic!("expected HealthRepair variant"),
        }
    }

    // -- InternError additional display tests ---------------------------------

    #[test]
    fn intern_error_budget_exceeded_display() {
        let task_id = "lc-cafebabe";
        let err = InternError::BudgetExceeded {
            task_id: task_id.into(),
            spent: 7.25,
            limit: 5.0,
        };
        let msg = format!("{err}");
        assert!(
            msg.contains(task_id),
            "display should contain task_id; got: {msg}"
        );
        assert!(
            msg.contains("7.25"),
            "display should contain spent amount; got: {msg}"
        );
        assert!(
            msg.contains("5.00"),
            "display should contain limit amount; got: {msg}"
        );
    }

    // -- DryRunResult -----------------------------------------------------

    #[test]
    fn dry_run_result_serde() {
        let result = DryRunResult {
            task_id: "lc-aabb".into(),
            task_name: "Test".into(),
            resolved_command: vec!["claude".into(), "-p".into(), "hello".into()],
            working_dir: "/tmp".into(),
            env_vars: HashMap::new(),
            timeout_secs: 600,
            max_budget_per_run: 5.0,
            daily_spend_so_far: 1.23,
            would_be_skipped: false,
            skip_reason: None,
            schedule_human: "Every 5m".into(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let roundtrip: DryRunResult = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.task_id, "lc-aabb");
        assert_eq!(roundtrip.resolved_command.len(), 3);
        assert!(!roundtrip.would_be_skipped);
    }
}

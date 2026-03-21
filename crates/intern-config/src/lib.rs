pub mod registry;

use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use intern_core::{
    CreateTaskInput, InternError, InternPaths, Task, TaskId, TaskStatus, UpdateTaskInput,
};
// Re-export Schedule for consumers of this crate.
pub use intern_core::Schedule;
pub use registry::RegistryManager;
use serde::{Deserialize, Serialize};

// ── Global Config ────────────────────────────────────────

/// Application-wide configuration persisted at `~/.intern/config.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Schema version for forward compatibility.
    #[serde(default = "default_version")]
    pub version: u32,

    /// Path or name of the Claude CLI binary.
    #[serde(default = "default_claude_binary")]
    pub claude_binary: String,

    /// Default budget per run in USD when not specified per-task.
    #[serde(default = "default_budget")]
    pub default_budget: f64,

    /// Default timeout in seconds when not specified per-task.
    #[serde(default = "default_timeout")]
    pub default_timeout: u64,

    /// Default maximum conversation turns.
    #[serde(default = "default_max_turns")]
    pub default_max_turns: u32,

    /// Number of days to retain execution logs before pruning.
    #[serde(default = "default_log_retention_days")]
    pub log_retention_days: u32,

    /// Whether macOS notifications are enabled.
    #[serde(default = "default_notifications_enabled")]
    pub notifications_enabled: bool,

    /// Maximum number of tasks that may execute concurrently.
    #[serde(default = "default_max_concurrent_tasks")]
    pub max_concurrent_tasks: u32,

    /// Optional daily budget cap in USD. When `None`, defaults to
    /// `max_budget_per_run * 20` at runtime.
    #[serde(default)]
    pub daily_budget_cap: Option<f64>,

    /// Estimated cost per second of execution for fallback budget tracking.
    #[serde(default = "default_cost_estimate_per_second")]
    pub cost_estimate_per_second: f64,

    /// UI theme name.
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_version() -> u32 {
    1
}
fn default_claude_binary() -> String {
    "claude".to_string()
}
fn default_budget() -> f64 {
    5.0
}
fn default_timeout() -> u64 {
    600
}
fn default_max_turns() -> u32 {
    50
}
fn default_log_retention_days() -> u32 {
    90
}
fn default_notifications_enabled() -> bool {
    true
}
fn default_max_concurrent_tasks() -> u32 {
    4
}
fn default_cost_estimate_per_second() -> f64 {
    0.01
}
fn default_theme() -> String {
    "dark".to_string()
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            claude_binary: default_claude_binary(),
            default_budget: default_budget(),
            default_timeout: default_timeout(),
            default_max_turns: default_max_turns(),
            log_retention_days: default_log_retention_days(),
            notifications_enabled: default_notifications_enabled(),
            max_concurrent_tasks: default_max_concurrent_tasks(),
            daily_budget_cap: None,
            cost_estimate_per_second: default_cost_estimate_per_second(),
            theme: default_theme(),
        }
    }
}

// ── Config Manager ───────────────────────────────────────

/// Manages the global configuration file and per-task YAML files.
///
/// All file writes use atomic temp-file-then-rename to prevent corruption
/// if the process crashes mid-write (CC-1).
pub struct ConfigManager {
    paths: InternPaths,
    global: GlobalConfig,
}

impl ConfigManager {
    /// Load the global config from disk, or create it with defaults if the
    /// config file does not exist. Also ensures all required directories exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file exists but cannot be read or parsed,
    /// or if directory creation fails.
    pub fn new(paths: InternPaths) -> Result<Self, InternError> {
        paths
            .ensure_dirs()
            .map_err(|e| InternError::Config(format!("Failed to create directories: {e}")))?;

        let global = if paths.config_file.exists() {
            let content = std::fs::read_to_string(&paths.config_file).map_err(|e| {
                InternError::Config(format!(
                    "Failed to read config file {}: {e}",
                    paths.config_file.display()
                ))
            })?;
            serde_yaml::from_str(&content).map_err(|e| {
                InternError::Config(format!(
                    "Failed to parse config file {}: {e}",
                    paths.config_file.display()
                ))
            })?
        } else {
            let config = GlobalConfig::default();
            let content = serde_yaml::to_string(&config).map_err(|e| {
                InternError::Config(format!("Failed to serialize default config: {e}"))
            })?;
            atomic_write(&paths.config_file, content.as_bytes())?;
            tracing::info!("Created default config at {}", paths.config_file.display());
            config
        };

        Ok(Self { paths, global })
    }

    /// Return a reference to the current global config.
    pub fn global_config(&self) -> &GlobalConfig {
        &self.global
    }

    /// Return a mutable reference to the global config for in-memory updates.
    pub fn global_config_mut(&mut self) -> &mut GlobalConfig {
        &mut self.global
    }

    /// Persist the current in-memory global config to disk using atomic writes.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or file writing fails.
    pub fn save_global_config(&self) -> Result<(), InternError> {
        let content = serde_yaml::to_string(&self.global)
            .map_err(|e| InternError::Config(format!("Failed to serialize config: {e}")))?;
        atomic_write(&self.paths.config_file, content.as_bytes())?;
        tracing::debug!(
            "Saved global config to {}",
            self.paths.config_file.display()
        );
        Ok(())
    }

    /// List all tasks by reading every `.yaml` file in the tasks directory.
    ///
    /// Each file is parsed independently. Corrupt or unparseable files produce
    /// a warning string rather than causing the entire operation to fail. This
    /// means a single bad file never takes down the task list (per R1).
    ///
    /// Returns a tuple of `(valid_tasks, warning_strings)`.
    pub fn list_tasks(&self) -> (Vec<Task>, Vec<String>) {
        let mut tasks = Vec::new();
        let mut warnings = Vec::new();

        let entries = match std::fs::read_dir(&self.paths.tasks_dir) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!("Failed to read tasks directory: {e}");
                warnings.push(format!(
                    "Failed to read tasks directory {}: {e}",
                    self.paths.tasks_dir.display()
                ));
                return (tasks, warnings);
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warnings.push(format!("Failed to read directory entry: {e}"));
                    continue;
                }
            };

            let path = entry.path();

            // Only process .yaml files, skip .tmp files and other artifacts
            match path.extension().and_then(|e| e.to_str()) {
                Some("yaml" | "yml") => {}
                _ => continue,
            }

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    warnings.push(format!("Failed to read {}: {e}", path.display()));
                    continue;
                }
            };

            match serde_yaml::from_str::<Task>(&content) {
                Ok(task) => tasks.push(task),
                Err(e) => {
                    warnings.push(format!("Failed to parse {}: {e}", path.display()));
                }
            }
        }

        // Sort by name for deterministic ordering
        tasks.sort_by(|a, b| a.name.cmp(&b.name));

        (tasks, warnings)
    }

    /// Read a single task by its ID.
    ///
    /// # Errors
    ///
    /// Returns `InternError::TaskNotFound` if the YAML file does not exist, or an
    /// error if the file cannot be read or parsed.
    pub fn get_task(&self, id: &str) -> Result<Task, InternError> {
        let path = self.task_path(id);
        if !path.exists() {
            return Err(InternError::TaskNotFound(id.to_string()));
        }

        let content = std::fs::read_to_string(&path).map_err(|e| {
            InternError::Config(format!("Failed to read task file {}: {e}", path.display()))
        })?;

        serde_yaml::from_str(&content).map_err(|e| {
            InternError::Config(format!("Failed to parse task file {}: {e}", path.display()))
        })
    }

    /// Write a task to its YAML file using atomic writes (CC-1).
    ///
    /// The write sequence is:
    /// 1. Serialize the task to YAML with block scalar style for the command field.
    /// 2. Write to `{id}.yaml.tmp` in the tasks directory.
    /// 3. `fsync` the file descriptor.
    /// 4. Rename the temp file to the final path (atomic on POSIX).
    ///
    /// # Errors
    ///
    /// Returns an error if serialization, writing, or renaming fails.
    pub fn save_task(&self, task: &Task) -> Result<(), InternError> {
        let path = self.task_path(task.id.as_str());
        let yaml = serialize_task_yaml(task)?;
        atomic_write(&path, yaml.as_bytes())?;
        tracing::debug!("Saved task {} to {}", task.id, path.display());
        Ok(())
    }

    /// Delete a task's YAML file.
    ///
    /// # Errors
    ///
    /// Returns `InternError::TaskNotFound` if the file does not exist, or an IO
    /// error if deletion fails.
    pub fn delete_task(&self, id: &str) -> Result<(), InternError> {
        let path = self.task_path(id);
        if !path.exists() {
            return Err(InternError::TaskNotFound(id.to_string()));
        }
        std::fs::remove_file(&path).map_err(|e| {
            InternError::Config(format!(
                "Failed to delete task file {}: {e}",
                path.display()
            ))
        })?;
        tracing::debug!("Deleted task file {}", path.display());
        Ok(())
    }

    /// Write a Claude slash-command context file for a task into its working
    /// directory under `.claude/commands/<sanitized-name>.md`.
    ///
    /// The file is written atomically and contains task metadata plus the
    /// full instruction text, formatted as a Claude custom slash-command.
    ///
    /// # Collision handling
    ///
    /// If a file already exists at the primary path and it does **not** belong
    /// to this task (checked via the `intern:task-id=` header comment), the
    /// filename is suffixed with the first 8 hex characters of the task ID
    /// (after the `lc-` prefix) to avoid overwriting another task's file.
    ///
    /// # Errors
    ///
    /// Returns an error if the path escapes the working directory, if the
    /// commands directory cannot be created, or if the file write fails.
    pub fn write_command_file(&self, task: &Task) -> Result<(), InternError> {
        let working_dir = if task.working_dir.to_string_lossy().starts_with('~') {
            intern_core::expand_tilde(&task.working_dir.to_string_lossy())
        } else {
            task.working_dir.clone()
        };

        let sanitized = sanitize_task_name(&task.name);
        let commands_dir = working_dir.join(".claude").join("commands");

        // Check for filename collision.
        let primary_path = commands_dir.join(format!("{sanitized}.md"));
        let filename =
            if primary_path.exists() && !file_belongs_to_task(&primary_path, task.id.as_str()) {
                format!("{}-{}.md", sanitized, &task.id.as_str()[3..11])
            } else {
                format!("{sanitized}.md")
            };

        let file_path = commands_dir.join(&filename);

        // Path traversal guard.
        if !file_path.starts_with(&working_dir) {
            return Err(InternError::Config(
                "context file path escapes working directory".into(),
            ));
        }

        // Create directory.
        std::fs::create_dir_all(&commands_dir).map_err(|e| {
            InternError::Config(format!(
                "Failed to create commands directory {}: {e}",
                commands_dir.display()
            ))
        })?;

        // Generate and write.
        let content = generate_context_file_content(task);
        atomic_write_md(&file_path, content.as_bytes())?;

        tracing::debug!(
            "Wrote context file for task {} to {}",
            task.id,
            file_path.display()
        );
        Ok(())
    }

    /// Delete the Claude slash-command context file for a task.
    ///
    /// Checks both the primary filename (`<sanitized>.md`) and the
    /// collision-suffixed filename (`<sanitized>-<short-id>.md`). Only deletes
    /// the file if it belongs to this task (verified via the header comment).
    ///
    /// This operation is idempotent — if neither candidate file exists, or
    /// neither belongs to this task, `Ok(())` is returned.
    pub fn delete_command_file(
        &self,
        working_dir: &Path,
        task_name: &str,
        task_id: &str,
    ) -> Result<(), InternError> {
        let sanitized = sanitize_task_name(task_name);
        let commands_dir = working_dir.join(".claude").join("commands");

        let primary = commands_dir.join(format!("{sanitized}.md"));
        let short_id = if task_id.len() > 3 {
            &task_id[3..task_id.len().min(11)]
        } else {
            task_id
        };
        let suffixed = commands_dir.join(format!("{sanitized}-{short_id}.md"));

        for candidate in [&primary, &suffixed] {
            if candidate.exists() && file_belongs_to_task(candidate, task_id) {
                if let Err(e) = std::fs::remove_file(candidate) {
                    tracing::warn!("Failed to delete context file {}: {e}", candidate.display());
                } else {
                    tracing::debug!("Deleted context file {}", candidate.display());
                }
                return Ok(());
            }
        }

        Ok(()) // Idempotent — not an error if file doesn't exist
    }

    /// Create a new `Task` from `CreateTaskInput`, filling in defaults from
    /// the global config.
    ///
    /// This does not persist the task; call `save_task` afterward.
    pub fn create_task_from_input(&self, input: CreateTaskInput) -> Task {
        let now = Utc::now();
        let schedule_human = input
            .schedule_human
            .unwrap_or_else(|| input.schedule.to_human());

        Task {
            id: TaskId::new(),
            name: input.name,
            command: input.command,
            skill: input.skill,
            schedule: input.schedule,
            schedule_human,
            working_dir: expand_path(&input.working_dir),
            env_vars: input.env_vars.unwrap_or_default(),
            max_budget_per_run: input
                .max_budget_per_run
                .unwrap_or(self.global.default_budget),
            max_turns: input.max_turns.or(Some(self.global.default_max_turns)),
            timeout_secs: input.timeout_secs.unwrap_or(self.global.default_timeout),
            status: TaskStatus::Active,
            tags: input.tags.unwrap_or_default(),
            agents: input.agents.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Apply a partial update to an existing task. Only fields that are `Some`
    /// in the `UpdateTaskInput` are overwritten.
    ///
    /// This does not persist the task; call `save_task` afterward.
    pub fn apply_update(&self, task: &mut Task, update: UpdateTaskInput) {
        if let Some(name) = update.name {
            task.name = name;
        }
        if let Some(command) = update.command {
            task.command = command;
        }
        if update.skill.is_some() {
            task.skill = update.skill;
        }
        if let Some(schedule) = update.schedule {
            task.schedule_human = update.schedule_human.unwrap_or_else(|| schedule.to_human());
            task.schedule = schedule;
        } else if let Some(schedule_human) = update.schedule_human {
            task.schedule_human = schedule_human;
        }
        if let Some(working_dir) = update.working_dir {
            task.working_dir = expand_path(&working_dir);
        }
        if let Some(env_vars) = update.env_vars {
            task.env_vars = env_vars;
        }
        if let Some(budget) = update.max_budget_per_run {
            task.max_budget_per_run = budget;
        }
        if update.max_turns.is_some() {
            task.max_turns = update.max_turns;
        }
        if let Some(timeout) = update.timeout_secs {
            task.timeout_secs = timeout;
        }
        if let Some(tags) = update.tags {
            task.tags = tags;
        }
        if let Some(agents) = update.agents {
            task.agents = agents;
        }
        if let Some(status) = update.status {
            task.status = status;
        }
        task.updated_at = Utc::now();
    }

    /// Return the `InternPaths` reference.
    pub fn paths(&self) -> &InternPaths {
        &self.paths
    }

    /// Return the filesystem path for a task's YAML file.
    fn task_path(&self, id: &str) -> PathBuf {
        self.paths.tasks_dir.join(format!("{id}.yaml"))
    }
}

// ── Path Expansion ───────────────────────────────────────

/// Expand a leading `~` in a path string to the user's home directory.
///
/// If the path does not start with `~`, or if the home directory cannot be
/// determined, the path is returned as-is.
pub fn expand_path(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix('~') {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest.strip_prefix('/').unwrap_or(rest));
        }
    }
    PathBuf::from(path)
}

// ── Task Name Sanitization ───────────────────────────────

/// Sanitize a task name for use as a filename.
///
/// Rules:
/// 1. Lowercase the full string
/// 2. Replace spaces, `/`, `\`, and NUL with `-`
/// 3. Strip leading dots
/// 4. Truncate to 64 characters
/// 5. If empty after transforms, fall back to `"task"`
///
/// # Examples
///
/// ```
/// use intern_config::sanitize_task_name;
///
/// assert_eq!(sanitize_task_name("Daily PR Review"), "daily-pr-review");
/// assert_eq!(sanitize_task_name("Dep/Audit"), "dep-audit");
/// assert_eq!(sanitize_task_name("..hidden"), "hidden");
/// ```
pub fn sanitize_task_name(name: &str) -> String {
    let mut s: String = name
        .to_lowercase()
        .chars()
        .map(|c| match c {
            ' ' | '/' | '\\' | '\0' => '-',
            other => other,
        })
        .collect();
    s = s.trim_start_matches('.').to_string();
    s = s.chars().take(64).collect();
    if s.is_empty() {
        "task".to_string()
    } else {
        s
    }
}

// ── Atomic Write (CC-1) ─────────────────────────────────

/// Write content to a file atomically by writing to a temporary file first,
/// calling `fsync`, then renaming to the final path. Rename is atomic on POSIX
/// filesystems, so readers never see a partially-written file.
fn atomic_write(path: &Path, content: &[u8]) -> Result<(), InternError> {
    let tmp_path = path.with_extension("yaml.tmp");
    let mut file = std::fs::File::create(&tmp_path).map_err(|e| {
        InternError::Config(format!(
            "Failed to create temp file {}: {e}",
            tmp_path.display()
        ))
    })?;
    file.write_all(content).map_err(|e| {
        InternError::Config(format!(
            "Failed to write temp file {}: {e}",
            tmp_path.display()
        ))
    })?;
    file.sync_all().map_err(|e| {
        InternError::Config(format!(
            "Failed to fsync temp file {}: {e}",
            tmp_path.display()
        ))
    })?;
    std::fs::rename(&tmp_path, path).map_err(|e| {
        InternError::Config(format!(
            "Failed to rename {} to {}: {e}",
            tmp_path.display(),
            path.display()
        ))
    })?;
    Ok(())
}

/// Write content to a `.md` file atomically using a `.md.tmp` temporary file.
///
/// Identical semantics to [`atomic_write`] but uses an `.md.tmp` extension so
/// that the temp file is distinguishable from the final Markdown output.
fn atomic_write_md(path: &Path, content: &[u8]) -> Result<(), InternError> {
    let tmp_path = path.with_extension("md.tmp");
    let mut file = std::fs::File::create(&tmp_path).map_err(|e| {
        InternError::Config(format!(
            "Failed to create temp file {}: {e}",
            tmp_path.display()
        ))
    })?;
    file.write_all(content).map_err(|e| {
        InternError::Config(format!(
            "Failed to write temp file {}: {e}",
            tmp_path.display()
        ))
    })?;
    file.sync_all().map_err(|e| {
        InternError::Config(format!(
            "Failed to fsync temp file {}: {e}",
            tmp_path.display()
        ))
    })?;
    std::fs::rename(&tmp_path, path).map_err(|e| {
        InternError::Config(format!(
            "Failed to rename {} to {}: {e}",
            tmp_path.display(),
            path.display()
        ))
    })?;
    Ok(())
}

// ── Context File Generation ──────────────────────────────

/// Generate the Markdown content for a Claude slash-command context file.
///
/// The output includes an HTML comment header that embeds the task ID so files
/// can be identified and owned by a specific task without relying on path alone.
fn generate_context_file_content(task: &Task) -> String {
    let sanitized = sanitize_task_name(&task.name);
    let mut s = String::new();

    // Header comment
    s.push_str(&format!(
        "<!-- intern:task-id={} intern:version=1 -->\n",
        task.id
    ));
    s.push('\n');

    // Title
    s.push_str(&format!("# {sanitized}\n\n"));

    // Blockquote notice
    s.push_str(
        "> **Scheduled Task** — Managed by [Intern](https://github.com/cNameHitch/intern).\n",
    );
    s.push_str("> Do not edit manually. Changes will be overwritten on task save.\n");
    s.push_str(&format!(
        "> To modify: `intern task edit {}` or use the Intern app.\n",
        task.id
    ));
    s.push('\n');

    // Automated Execution Notice
    s.push_str("## Automated Execution Notice\n\n");
    s.push_str("You are running as a **scheduled unattended task** via Intern. This means:\n\n");
    s.push_str("- No human is present. Do not ask for clarification — use your best judgment.\n");
    s.push_str("- Do not wait for confirmation before taking actions within the stated scope.\n");
    s.push_str("- Produce structured output as specified and exit cleanly.\n");
    s.push_str("- If you encounter an ambiguous situation, take the most conservative action and document what you did.\n");
    s.push('\n');

    // Execution Constraints table
    s.push_str("## Execution Constraints\n\n");
    s.push_str("| Constraint | Value |\n");
    s.push_str("|------------|-------|\n");
    s.push_str(&format!("| Schedule | {} |\n", task.schedule_human));
    if let Some(n) = task.max_turns {
        s.push_str(&format!("| Turn budget | {n} turns maximum |\n"));
    }
    s.push_str(&format!(
        "| Cost budget | ${:.2} per run |\n",
        task.max_budget_per_run
    ));
    s.push_str(&format!("| Timeout | {} seconds |\n", task.timeout_secs));
    if !task.agents.is_empty() {
        let agents_str: Vec<String> = task.agents.iter().map(|a| format!("@{a}")).collect();
        s.push_str(&format!("| Agents | {} |\n", agents_str.join(", ")));
    }
    if !task.env_vars.is_empty() {
        s.push_str(&format!(
            "| Env vars | {} variable(s) configured |\n",
            task.env_vars.len()
        ));
    }
    s.push('\n');

    // Instructions
    s.push_str("## Instructions\n\n");
    s.push_str(&task.command);
    s.push('\n');

    s
}

/// Return `true` if the first line of `path` contains `intern:task-id=<task_id>`.
///
/// Used to determine file ownership before overwriting or deleting, so that
/// two tasks with identical sanitized names do not clobber each other's files.
fn file_belongs_to_task(path: &Path, task_id: &str) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    content
        .lines()
        .next()
        .map(|line| line.contains(&format!("intern:task-id={task_id}")))
        .unwrap_or(false)
}

// ── YAML Serialization ──────────────────────────────────

/// Serialize a `Task` to YAML, using block scalar style for the `command` field
/// so that multiline prompts remain human-readable.
///
/// We first serialize via serde_yaml to get a base YAML document, then
/// post-process the `command` field to use literal block scalar style (`|-`)
/// when the command contains newlines, preserving line breaks exactly.
fn serialize_task_yaml(task: &Task) -> Result<String, InternError> {
    // Serialize the task normally first
    let yaml = serde_yaml::to_string(task)
        .map_err(|e| InternError::Yaml(format!("Failed to serialize task: {e}")))?;

    // If the command contains newlines, rewrite the command field to use
    // block scalar style for readability.
    if task.command.contains('\n') {
        Ok(rewrite_command_block_scalar(&yaml, &task.command))
    } else {
        Ok(yaml)
    }
}

/// Rewrite the `command` field in a YAML string to use literal block scalar
/// style (`|-`). This preserves line breaks exactly and makes multiline
/// prompts significantly more readable.
///
/// Detection logic: a top-level YAML key is any line that starts at column 0
/// (no leading whitespace) and contains a colon. The original serde_yaml
/// serialization of the command value (possibly spanning multiple continuation
/// lines) is replaced with a `|-` block scalar.
fn rewrite_command_block_scalar(yaml: &str, command: &str) -> String {
    let mut result = String::with_capacity(yaml.len() + command.len());
    let mut skip_until_next_field = false;

    for line in yaml.lines() {
        if skip_until_next_field {
            // We are skipping the original serialized value of the command field.
            // A top-level field is any non-empty line that starts at column 0
            // (no leading whitespace) and contains a colon somewhere.
            let is_top_level_key = !line.is_empty() && !line.starts_with(' ') && line.contains(':');

            if is_top_level_key {
                // This is the next field — stop skipping.
                skip_until_next_field = false;
                result.push_str(line);
                result.push('\n');
            }
            // Otherwise keep skipping continuation/indented lines.
            continue;
        }

        if line.starts_with("command:") {
            // Replace with block scalar
            result.push_str("command: |-\n");
            for cmd_line in command.lines() {
                result.push_str("  ");
                result.push_str(cmd_line);
                result.push('\n');
            }
            skip_until_next_field = true;
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

// ── Tests ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    /// Create a `ConfigManager` backed by a temporary directory.
    fn setup() -> (TempDir, ConfigManager) {
        let tmp = TempDir::new().unwrap();
        let paths = InternPaths::with_root(tmp.path().to_path_buf());
        let mgr = ConfigManager::new(paths).unwrap();
        (tmp, mgr)
    }

    /// Create a minimal valid `CreateTaskInput` whose working_dir points at
    /// the given directory (so validation passes).
    fn sample_input(working_dir: &Path) -> CreateTaskInput {
        CreateTaskInput {
            name: "Test Task".to_string(),
            command: "echo hello".to_string(),
            skill: None,
            schedule: Schedule::Interval { seconds: 300 },
            schedule_human: None,
            working_dir: working_dir.to_string_lossy().to_string(),
            env_vars: None,
            max_budget_per_run: None,
            max_turns: None,
            timeout_secs: None,
            tags: Some(vec!["test".to_string()]),
            agents: None,
        }
    }

    // ── CRUD Round-Trip ──────────────────────────────────

    #[test]
    fn create_and_read_task_roundtrip() {
        let (tmp, mgr) = setup();
        let input = sample_input(tmp.path());
        let task = mgr.create_task_from_input(input);
        let task_id = task.id.as_str().to_string();

        mgr.save_task(&task).unwrap();
        let loaded = mgr.get_task(&task_id).unwrap();

        assert_eq!(loaded.id, task.id);
        assert_eq!(loaded.name, "Test Task");
        assert_eq!(loaded.command, "echo hello");
        assert_eq!(loaded.status, TaskStatus::Active);
        assert_eq!(loaded.max_budget_per_run, 5.0);
        assert_eq!(loaded.timeout_secs, 600);
        assert_eq!(loaded.tags, vec!["test"]);
    }

    #[test]
    fn update_task_partial() {
        let (tmp, mgr) = setup();
        let input = sample_input(tmp.path());
        let mut task = mgr.create_task_from_input(input);
        let original_created_at = task.created_at;

        let update = UpdateTaskInput {
            id: task.id.as_str().to_string(),
            name: Some("Updated Name".to_string()),
            command: None,
            skill: None,
            schedule: None,
            schedule_human: None,
            working_dir: None,
            env_vars: None,
            max_budget_per_run: Some(10.0),
            max_turns: None,
            timeout_secs: None,
            tags: None,
            agents: None,
            status: Some(TaskStatus::Paused),
        };

        mgr.apply_update(&mut task, update);

        assert_eq!(task.name, "Updated Name");
        assert_eq!(task.command, "echo hello"); // unchanged
        assert_eq!(task.max_budget_per_run, 10.0);
        assert_eq!(task.status, TaskStatus::Paused);
        assert_eq!(task.created_at, original_created_at); // unchanged
        assert!(task.updated_at > original_created_at);
    }

    #[test]
    fn delete_task_removes_file() {
        let (tmp, mgr) = setup();
        let input = sample_input(tmp.path());
        let task = mgr.create_task_from_input(input);
        let task_id = task.id.as_str().to_string();

        mgr.save_task(&task).unwrap();
        assert!(mgr.get_task(&task_id).is_ok());

        mgr.delete_task(&task_id).unwrap();
        assert!(mgr.get_task(&task_id).is_err());
    }

    #[test]
    fn delete_nonexistent_task_returns_not_found() {
        let (_tmp, mgr) = setup();
        let result = mgr.delete_task("lc-00000000");
        assert!(matches!(result, Err(InternError::TaskNotFound(_))));
    }

    // ── list_tasks ───────────────────────────────────────

    #[test]
    fn list_tasks_returns_valid_and_warns_on_corrupt() {
        let (tmp, mgr) = setup();

        // Create two valid tasks
        let input1 = sample_input(tmp.path());
        let task1 = mgr.create_task_from_input(input1);
        mgr.save_task(&task1).unwrap();

        let mut input2 = sample_input(tmp.path());
        input2.name = "Second Task".to_string();
        let task2 = mgr.create_task_from_input(input2);
        mgr.save_task(&task2).unwrap();

        // Write a corrupt YAML file
        let corrupt_path = mgr.paths.tasks_dir.join("lc-corrupt0.yaml");
        std::fs::write(&corrupt_path, "this is not valid yaml: [[[").unwrap();

        let (tasks, warnings) = mgr.list_tasks();

        assert_eq!(tasks.len(), 2);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("lc-corrupt0.yaml"));
    }

    #[test]
    fn list_tasks_skips_non_yaml_files() {
        let (tmp, mgr) = setup();

        let input = sample_input(tmp.path());
        let task = mgr.create_task_from_input(input);
        mgr.save_task(&task).unwrap();

        // Write a non-YAML file that should be ignored
        let txt_path = mgr.paths.tasks_dir.join("notes.txt");
        std::fs::write(&txt_path, "just a note").unwrap();

        let (tasks, warnings) = mgr.list_tasks();
        assert_eq!(tasks.len(), 1);
        assert!(warnings.is_empty());
    }

    // ── GlobalConfig defaults ────────────────────────────

    #[test]
    fn global_config_defaults_when_file_missing() {
        let (_tmp, mgr) = setup();
        let config = mgr.global_config();

        assert_eq!(config.version, 1);
        assert_eq!(config.claude_binary, "claude");
        assert_eq!(config.default_budget, 5.0);
        assert_eq!(config.default_timeout, 600);
        assert_eq!(config.default_max_turns, 50);
        assert_eq!(config.log_retention_days, 90);
        assert!(config.notifications_enabled);
        assert_eq!(config.max_concurrent_tasks, 4);
        assert!(config.daily_budget_cap.is_none());
        assert!((config.cost_estimate_per_second - 0.01).abs() < f64::EPSILON);
        assert_eq!(config.theme, "dark");
    }

    #[test]
    fn global_config_persists_and_reloads() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();

        // Create and modify config
        {
            let paths = InternPaths::with_root(root.clone());
            let mut mgr = ConfigManager::new(paths).unwrap();
            mgr.global_config_mut().default_budget = 10.0;
            mgr.global_config_mut().theme = "light".to_string();
            mgr.save_global_config().unwrap();
        }

        // Reload from disk
        {
            let paths = InternPaths::with_root(root);
            let mgr = ConfigManager::new(paths).unwrap();
            assert_eq!(mgr.global_config().default_budget, 10.0);
            assert_eq!(mgr.global_config().theme, "light");
        }
    }

    // ── Tilde expansion ──────────────────────────────────

    #[test]
    fn expand_tilde_replaces_home() {
        let expanded = expand_path("~/projects");
        let home = dirs::home_dir().unwrap();
        assert_eq!(expanded, home.join("projects"));
    }

    #[test]
    fn expand_tilde_alone() {
        let expanded = expand_path("~");
        let home = dirs::home_dir().unwrap();
        assert_eq!(expanded, home);
    }

    #[test]
    fn expand_no_tilde_unchanged() {
        let expanded = expand_path("/usr/local/bin");
        assert_eq!(expanded, PathBuf::from("/usr/local/bin"));
    }

    // ── CreateTaskInput -> Task with defaults ────────────

    #[test]
    fn create_task_fills_defaults_from_global_config() {
        let (tmp, mgr) = setup();
        let input = CreateTaskInput {
            name: "My Task".to_string(),
            command: "echo hi".to_string(),
            skill: None,
            schedule: Schedule::Cron {
                expression: "*/5 * * * *".to_string(),
            },
            schedule_human: None,
            working_dir: tmp.path().to_string_lossy().to_string(),
            env_vars: None,
            max_budget_per_run: None,
            max_turns: None,
            timeout_secs: None,
            tags: None,
            agents: None,
        };

        let task = mgr.create_task_from_input(input);

        assert!(task.id.as_str().starts_with("lc-"));
        assert_eq!(task.name, "My Task");
        assert_eq!(task.command, "echo hi");
        assert_eq!(task.max_budget_per_run, 5.0); // from GlobalConfig default
        assert_eq!(task.timeout_secs, 600); // from GlobalConfig default
        assert_eq!(task.max_turns, Some(50)); // from GlobalConfig default
        assert_eq!(task.status, TaskStatus::Active);
        assert!(task.tags.is_empty());
        assert_eq!(task.schedule_human, "Cron: */5 * * * *"); // auto-generated
    }

    #[test]
    fn create_task_respects_explicit_values() {
        let (tmp, mgr) = setup();
        let input = CreateTaskInput {
            name: "Custom Task".to_string(),
            command: "run test".to_string(),
            skill: Some("testing".to_string()),
            schedule: Schedule::Interval { seconds: 60 },
            schedule_human: Some("Every minute".to_string()),
            working_dir: tmp.path().to_string_lossy().to_string(),
            env_vars: Some(HashMap::from([("KEY".to_string(), "val".to_string())])),
            max_budget_per_run: Some(20.0),
            max_turns: Some(100),
            timeout_secs: Some(1200),
            tags: Some(vec!["custom".to_string(), "important".to_string()]),
            agents: None,
        };

        let task = mgr.create_task_from_input(input);

        assert_eq!(task.max_budget_per_run, 20.0);
        assert_eq!(task.timeout_secs, 1200);
        assert_eq!(task.max_turns, Some(100));
        assert_eq!(task.schedule_human, "Every minute");
        assert_eq!(task.skill, Some("testing".to_string()));
        assert_eq!(task.env_vars.get("KEY"), Some(&"val".to_string()));
        assert_eq!(task.tags.len(), 2);
    }

    // ── Atomic write: temp file cleanup ──────────────────

    #[test]
    fn atomic_write_no_temp_file_on_success() {
        let (tmp, mgr) = setup();
        let input = sample_input(tmp.path());
        let task = mgr.create_task_from_input(input);
        let task_id = task.id.as_str().to_string();

        mgr.save_task(&task).unwrap();

        let yaml_path = mgr.paths.tasks_dir.join(format!("{task_id}.yaml"));
        let tmp_path = mgr.paths.tasks_dir.join(format!("{task_id}.yaml.tmp"));

        assert!(yaml_path.exists(), "Final YAML file should exist");
        assert!(
            !tmp_path.exists(),
            "Temp file should not linger after success"
        );
    }

    // ── Multiline command block scalar ───────────────────

    #[test]
    fn multiline_command_uses_block_scalar() {
        let (tmp, mgr) = setup();
        let mut input = sample_input(tmp.path());
        input.command =
            "claude -p 'Review all open PRs.\nCheck for logic errors.\nFix style issues.'"
                .to_string();
        let task = mgr.create_task_from_input(input);

        mgr.save_task(&task).unwrap();

        let path = mgr.paths.tasks_dir.join(format!("{}.yaml", task.id));
        let content = std::fs::read_to_string(&path).unwrap();

        // The YAML should contain a block scalar indicator
        assert!(
            content.contains("command: |-"),
            "Multiline command should use block scalar style. Got:\n{content}"
        );

        // Round-trip: the task should deserialize correctly
        let loaded = mgr.get_task(task.id.as_str()).unwrap();
        assert_eq!(loaded.command, task.command);
    }

    // ── Update with schedule change ──────────────────────

    #[test]
    fn update_schedule_updates_human_readable() {
        let (tmp, mgr) = setup();
        let input = sample_input(tmp.path());
        let mut task = mgr.create_task_from_input(input);

        let update = UpdateTaskInput {
            id: task.id.as_str().to_string(),
            name: None,
            command: None,
            skill: None,
            schedule: Some(Schedule::Interval { seconds: 7200 }),
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

        mgr.apply_update(&mut task, update);

        assert_eq!(task.schedule_human, "Every 2h");
    }

    // ── Edge case: empty tasks directory ─────────────────

    #[test]
    fn list_tasks_empty_directory() {
        let (_tmp, mgr) = setup();
        let (tasks, warnings) = mgr.list_tasks();
        assert!(tasks.is_empty());
        assert!(warnings.is_empty());
    }

    // ── Save and reload global config ────────────────────

    #[test]
    fn save_global_config_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let paths = InternPaths::with_root(tmp.path().to_path_buf());
        let mut mgr = ConfigManager::new(paths).unwrap();

        mgr.global_config_mut().max_concurrent_tasks = 8;
        mgr.global_config_mut().daily_budget_cap = Some(50.0);
        mgr.save_global_config().unwrap();

        let paths2 = InternPaths::with_root(tmp.path().to_path_buf());
        let mgr2 = ConfigManager::new(paths2).unwrap();
        assert_eq!(mgr2.global_config().max_concurrent_tasks, 8);
        assert_eq!(mgr2.global_config().daily_budget_cap, Some(50.0));
    }

    // ── sanitize_task_name ──────────────────────────────

    #[test]
    fn sanitize_name_spaces_to_dashes() {
        assert_eq!(sanitize_task_name("Daily PR Review"), "daily-pr-review");
    }

    #[test]
    fn sanitize_name_slashes_to_dashes() {
        assert_eq!(sanitize_task_name("Dep/Audit"), "dep-audit");
    }

    #[test]
    fn sanitize_name_leading_dots_stripped() {
        assert_eq!(sanitize_task_name("..hidden task"), "hidden-task");
    }

    #[test]
    fn sanitize_name_truncates_to_64() {
        let long_name = "a".repeat(100);
        assert_eq!(sanitize_task_name(&long_name), "a".repeat(64));
    }

    #[test]
    fn sanitize_name_empty_fallback() {
        assert_eq!(sanitize_task_name("   "), "---");
    }

    #[test]
    fn sanitize_name_uppercase() {
        assert_eq!(sanitize_task_name("UPPER CASE"), "upper-case");
    }

    // ── write/delete command file ─────────────────────

    #[test]
    fn write_command_file_creates_file() {
        let (tmp, mgr) = setup();
        let working_dir = tmp.path().join("project");
        std::fs::create_dir_all(&working_dir).unwrap();

        let mut input = sample_input(&working_dir);
        input.name = "My Test Task".to_string();
        input.command = "Do something useful".to_string();
        let task = mgr.create_task_from_input(input);

        mgr.write_command_file(&task).unwrap();

        let file_path = working_dir
            .join(".claude")
            .join("commands")
            .join("my-test-task.md");
        assert!(file_path.exists(), "Context file should exist");

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.starts_with("<!-- intern:task-id="));
        assert!(content.contains("## Instructions"));
        assert!(content.contains("Do something useful"));
        assert!(content.contains("## Automated Execution Notice"));
    }

    #[test]
    fn delete_command_file_removes_file() {
        let (tmp, mgr) = setup();
        let working_dir = tmp.path().join("project");
        std::fs::create_dir_all(&working_dir).unwrap();

        let mut input = sample_input(&working_dir);
        input.name = "Delete Me".to_string();
        let task = mgr.create_task_from_input(input);

        mgr.write_command_file(&task).unwrap();

        let file_path = working_dir
            .join(".claude")
            .join("commands")
            .join("delete-me.md");
        assert!(file_path.exists());

        mgr.delete_command_file(&working_dir, &task.name, task.id.as_str())
            .unwrap();
        assert!(!file_path.exists());
    }

    #[test]
    fn delete_command_file_idempotent() {
        let (tmp, mgr) = setup();
        let working_dir = tmp.path().join("project");
        // No file exists, should not error
        let result = mgr.delete_command_file(&working_dir, "nonexistent", "lc-00000000");
        assert!(result.is_ok());
    }

    #[test]
    fn write_command_file_collision_appends_suffix() {
        let (tmp, mgr) = setup();
        let working_dir = tmp.path().join("project");
        std::fs::create_dir_all(&working_dir).unwrap();

        // Create task A with name "review"
        let mut input_a = sample_input(&working_dir);
        input_a.name = "Review".to_string();
        let task_a = mgr.create_task_from_input(input_a);
        mgr.write_command_file(&task_a).unwrap();

        // Create task B with same sanitized name "review" but different ID
        let mut input_b = sample_input(&working_dir);
        input_b.name = "review".to_string();
        let task_b = mgr.create_task_from_input(input_b);
        mgr.write_command_file(&task_b).unwrap();

        // Task A should have review.md
        let primary = working_dir
            .join(".claude")
            .join("commands")
            .join("review.md");
        assert!(primary.exists());
        let content_a = std::fs::read_to_string(&primary).unwrap();
        assert!(content_a.contains(&format!("intern:task-id={}", task_a.id)));

        // Task B should have review-<short-id>.md
        let short_id = &task_b.id.as_str()[3..11];
        let suffixed = working_dir
            .join(".claude")
            .join("commands")
            .join(format!("review-{short_id}.md"));
        assert!(
            suffixed.exists(),
            "Suffixed file should exist for collision case"
        );
    }
}

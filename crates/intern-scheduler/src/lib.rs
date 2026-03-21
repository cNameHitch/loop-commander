use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use intern_core::{InternPaths, Schedule, Task};
use tracing::{debug, info, warn};

/// Validate a standard 5-field cron expression using the `cron` crate parser.
///
/// Accepts the traditional POSIX/Vixie cron format with exactly 5 whitespace-
/// separated fields: `<minute> <hour> <dom> <month> <dow>`.
///
/// Internally the expression is prefixed with a seconds field (`0`) before
/// being handed to the `cron` crate, which requires 6 fields. This is
/// transparent to callers — the API surface remains 5-field.
///
/// Returns `Ok(())` when the expression is syntactically valid, or
/// `Err(message)` with a human-readable explanation of what is wrong.
///
/// # Examples
///
/// ```
/// assert!(intern_scheduler::validate_cron("0 9 * * 1-5").is_ok());
/// assert!(intern_scheduler::validate_cron("*/15 * * * *").is_ok());
/// assert!(intern_scheduler::validate_cron("not a cron").is_err());
/// ```
pub fn validate_cron(expr: &str) -> Result<(), String> {
    // The `cron` crate (v0.12) requires a 6-field expression with a leading
    // seconds field. Standard 5-field cron omits seconds, so we prepend `0 `
    // (fire at second 0 of the matching minute) before parsing.
    let six_field = format!("0 {expr}");
    six_field
        .parse::<cron::Schedule>()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Intermediate representation for cron-to-plist conversion.
///
/// A cron expression may expand to a single interval, a single calendar entry,
/// or multiple calendar entries (e.g. weekday ranges like `1-5`). The `Schedule`
/// enum in `intern-core` only holds a single calendar entry, so this type captures
/// the full expansion before it is flattened into plist XML.
#[derive(Debug, Clone)]
enum CronExpansion {
    /// Maps to `StartInterval` in launchd.
    Interval { seconds: u64 },
    /// One or more `StartCalendarInterval` dicts.
    Calendars(Vec<CalendarEntry>),
}

/// A single `StartCalendarInterval` dict's fields.
#[derive(Debug, Clone)]
struct CalendarEntry {
    minute: Option<u8>,
    hour: Option<u8>,
    day: Option<u8>,
    weekday: Option<u8>,
    month: Option<u8>,
}

impl CalendarEntry {
    /// Convert this entry to a plist dictionary.
    fn to_plist_dict(&self) -> plist::Dictionary {
        let mut dict = plist::Dictionary::new();
        if let Some(m) = self.minute {
            dict.insert("Minute".into(), plist::Value::Integer(i64::from(m).into()));
        }
        if let Some(h) = self.hour {
            dict.insert("Hour".into(), plist::Value::Integer(i64::from(h).into()));
        }
        if let Some(d) = self.day {
            dict.insert("Day".into(), plist::Value::Integer(i64::from(d).into()));
        }
        if let Some(w) = self.weekday {
            dict.insert("Weekday".into(), plist::Value::Integer(i64::from(w).into()));
        }
        if let Some(mo) = self.month {
            dict.insert("Month".into(), plist::Value::Integer(i64::from(mo).into()));
        }
        dict
    }
}

/// Manages launchd plists for Intern tasks.
///
/// Each active task gets a plist at `~/.intern/plists/<label>.plist`
/// symlinked into `~/Library/LaunchAgents/`. The scheduler handles the full
/// lifecycle: register (write plist + symlink), activate (bootstrap into
/// launchd), deactivate (bootout), and unregister (remove files).
pub struct Scheduler {
    paths: InternPaths,
    runner_path: PathBuf,
}

impl Scheduler {
    /// Create a new scheduler, discovering the `intern-runner` binary on disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the `intern-runner` binary cannot be found in any of the
    /// searched locations.
    pub fn new(paths: InternPaths) -> Result<Self> {
        let runner_path = find_runner().context("Failed to locate intern-runner binary")?;
        info!(?runner_path, "Discovered intern-runner binary");
        Ok(Self { paths, runner_path })
    }

    /// Return the path to the `intern-runner` binary that was discovered at
    /// construction time.
    #[must_use]
    pub fn runner_path(&self) -> &PathBuf {
        &self.runner_path
    }

    /// Generate plist XML for a task, write it to `plists_dir`, and create a
    /// symlink in `~/Library/LaunchAgents/`.
    ///
    /// # Errors
    ///
    /// Returns an error if the plist cannot be written or the symlink cannot be
    /// created.
    pub fn register(&self, task: &Task) -> Result<()> {
        let label = task.id.launchd_label();
        let plist_filename = format!("{label}.plist");
        let plist_path = self.paths.plists_dir.join(&plist_filename);
        let symlink_path = self.paths.launch_agents_dir.join(&plist_filename);

        // Ensure directories exist.
        std::fs::create_dir_all(&self.paths.plists_dir)
            .context("Failed to create plists directory")?;
        std::fs::create_dir_all(&self.paths.launch_agents_dir)
            .context("Failed to create LaunchAgents directory")?;
        std::fs::create_dir_all(&self.paths.output_dir)
            .context("Failed to create output directory")?;

        // Build the plist dictionary.
        let plist_dict = self.build_plist(task);

        // Write plist to file.
        let file = std::fs::File::create(&plist_path)
            .with_context(|| format!("Failed to create plist file at {}", plist_path.display()))?;
        plist::to_writer_xml(file, &plist_dict)
            .with_context(|| format!("Failed to write plist XML to {}", plist_path.display()))?;

        info!(?plist_path, "Wrote plist file");

        // Create symlink (remove stale one first).
        if symlink_path.exists() || symlink_path.symlink_metadata().is_ok() {
            std::fs::remove_file(&symlink_path).ok();
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(&plist_path, &symlink_path).with_context(|| {
            format!(
                "Failed to symlink {} -> {}",
                symlink_path.display(),
                plist_path.display()
            )
        })?;

        info!(?symlink_path, "Created symlink in LaunchAgents");
        Ok(())
    }

    /// Activate a task's launchd job using the modern `bootstrap` API.
    ///
    /// Falls back to `launchctl load -w` if `bootstrap` fails with an error
    /// other than errno 37 (already loaded).
    ///
    /// # Errors
    ///
    /// Returns an error if both the modern and legacy APIs fail.
    pub fn activate(&self, task: &Task) -> Result<()> {
        let label = task.id.launchd_label();
        let plist_filename = format!("{label}.plist");
        let symlink_path = self.paths.launch_agents_dir.join(&plist_filename);
        let uid = get_uid()?;
        let domain_target = format!("gui/{uid}");

        debug!(%label, %domain_target, "Bootstrapping launchd job");

        let output = Command::new("launchctl")
            .arg("bootstrap")
            .arg(&domain_target)
            .arg(&symlink_path)
            .output()
            .context("Failed to execute launchctl bootstrap")?;

        if output.status.success() {
            info!(%label, "Successfully bootstrapped launchd job");
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);

        // errno 37 = "Operation already in progress" means already loaded.
        // Also check for error code 37 in the stderr text.
        if stderr.contains("37") || stderr.contains("already loaded") {
            info!(%label, "Job already loaded (errno 37), treating as success");
            return Ok(());
        }

        warn!(
            %label,
            %stderr,
            "launchctl bootstrap failed, falling back to load -w"
        );

        // Fallback: legacy API.
        let fallback = Command::new("launchctl")
            .arg("load")
            .arg("-w")
            .arg(&symlink_path)
            .output()
            .context("Failed to execute launchctl load -w")?;

        if fallback.status.success() {
            info!(%label, "Successfully loaded via legacy launchctl load -w");
            return Ok(());
        }

        let fallback_stderr = String::from_utf8_lossy(&fallback.stderr);
        anyhow::bail!(
            "Failed to activate launchd job {label}: bootstrap error: {stderr}, \
             load -w error: {fallback_stderr}"
        );
    }

    /// Deactivate a task's launchd job using the modern `bootout` API.
    ///
    /// Falls back to `launchctl unload` if `bootout` fails.
    ///
    /// # Errors
    ///
    /// Returns an error if both the modern and legacy APIs fail. Deactivating a
    /// job that is not loaded is treated as success.
    pub fn deactivate(&self, task_id: &str) -> Result<()> {
        let label = format!("com.intern.task.{task_id}");
        let uid = get_uid()?;
        let service_target = format!("gui/{uid}/{label}");

        debug!(%label, "Booting out launchd job");

        let output = Command::new("launchctl")
            .arg("bootout")
            .arg(&service_target)
            .output()
            .context("Failed to execute launchctl bootout")?;

        if output.status.success() {
            info!(%label, "Successfully booted out launchd job");
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);

        // "No such process" or "Could not find service" means not loaded, which is fine.
        if stderr.contains("No such process")
            || stderr.contains("Could not find service")
            || stderr.contains('3')
        {
            debug!(%label, "Job was not loaded, treating deactivate as success");
            return Ok(());
        }

        warn!(%label, %stderr, "launchctl bootout failed, falling back to unload");

        // Fallback: legacy API.
        let plist_filename = format!("{label}.plist");
        let symlink_path = self.paths.launch_agents_dir.join(&plist_filename);

        let fallback = Command::new("launchctl")
            .arg("unload")
            .arg(&symlink_path)
            .output()
            .context("Failed to execute launchctl unload")?;

        if fallback.status.success() {
            info!(%label, "Successfully unloaded via legacy launchctl unload");
            return Ok(());
        }

        let fallback_stderr = String::from_utf8_lossy(&fallback.stderr);

        // Not loaded is still fine for deactivate.
        if fallback_stderr.contains("Could not find specified service") {
            debug!(%label, "Job was not loaded (legacy), treating as success");
            return Ok(());
        }

        anyhow::bail!(
            "Failed to deactivate launchd job {label}: bootout error: {stderr}, \
             unload error: {fallback_stderr}"
        );
    }

    /// Remove the plist file and its symlink from disk.
    ///
    /// # Errors
    ///
    /// Returns an error if removal fails for reasons other than the files not
    /// existing.
    pub fn unregister(&self, task_id: &str) -> Result<()> {
        let label = format!("com.intern.task.{task_id}");
        let plist_filename = format!("{label}.plist");
        let plist_path = self.paths.plists_dir.join(&plist_filename);
        let symlink_path = self.paths.launch_agents_dir.join(&plist_filename);

        // Remove symlink.
        if symlink_path.symlink_metadata().is_ok() {
            std::fs::remove_file(&symlink_path).with_context(|| {
                format!("Failed to remove symlink at {}", symlink_path.display())
            })?;
            debug!(?symlink_path, "Removed LaunchAgents symlink");
        }

        // Remove plist file.
        if plist_path.exists() {
            std::fs::remove_file(&plist_path)
                .with_context(|| format!("Failed to remove plist at {}", plist_path.display()))?;
            debug!(?plist_path, "Removed plist file");
        }

        info!(%label, "Unregistered task plist");
        Ok(())
    }

    /// Register and activate a task in one call.
    ///
    /// # Errors
    ///
    /// Returns an error if either registration or activation fails.
    pub fn install(&self, task: &Task) -> Result<()> {
        self.register(task)?;
        self.activate(task)?;
        Ok(())
    }

    /// Deactivate and unregister a task in one call.
    ///
    /// # Errors
    ///
    /// Returns an error if deactivation or unregistration fails.
    pub fn uninstall(&self, task_id: &str) -> Result<()> {
        self.deactivate(task_id)?;
        self.unregister(task_id)?;
        Ok(())
    }

    /// Deactivate, re-register, and re-activate a task.
    ///
    /// Used when a task's schedule or configuration changes and the plist needs
    /// to be regenerated.
    ///
    /// # Errors
    ///
    /// Returns an error if any step fails.
    pub fn reinstall(&self, task: &Task) -> Result<()> {
        let task_id = task.id.as_str();
        // Deactivate first (ignore errors if not loaded).
        if let Err(e) = self.deactivate(task_id) {
            warn!(%task_id, error = %e, "Deactivate during reinstall failed (continuing)");
        }
        self.register(task)?;
        self.activate(task)?;
        Ok(())
    }

    /// Check whether a task's launchd job is currently loaded.
    ///
    /// Uses `launchctl print gui/<uid>/<label>` — exit code 0 means loaded.
    ///
    /// # Errors
    ///
    /// Returns an error if `launchctl print` cannot be executed.
    pub fn is_loaded(&self, task_id: &str) -> Result<bool> {
        let label = format!("com.intern.task.{task_id}");
        let uid = get_uid()?;
        let service_target = format!("gui/{uid}/{label}");

        let output = Command::new("launchctl")
            .arg("print")
            .arg(&service_target)
            .output()
            .context("Failed to execute launchctl print")?;

        Ok(output.status.success())
    }

    /// Convert a `Schedule` enum value to a plist `Value` suitable for
    /// inclusion in a launchd plist.
    ///
    /// - `Schedule::Interval` produces a `StartInterval` integer.
    /// - `Schedule::Calendar` produces a single `StartCalendarInterval` dict.
    /// - `Schedule::Cron` parses the expression and produces either a
    ///   `StartInterval` integer or one-or-more `StartCalendarInterval` dicts.
    ///
    /// The return is a plist `Value` that should be inserted into the plist
    /// dictionary under the appropriate key. Use `schedule_to_plist_entries` to
    /// get key-value pairs ready for insertion.
    pub fn schedule_to_plist(&self, schedule: &Schedule) -> plist::Value {
        match schedule {
            #[allow(clippy::cast_possible_wrap)] // launchd intervals are always small enough
            Schedule::Interval { seconds } => plist::Value::Integer((*seconds as i64).into()),
            Schedule::Calendar {
                minute,
                hour,
                day,
                weekday,
                month,
            } => {
                let entry = CalendarEntry {
                    minute: *minute,
                    hour: *hour,
                    day: *day,
                    weekday: *weekday,
                    month: *month,
                };
                plist::Value::Dictionary(entry.to_plist_dict())
            }
            Schedule::Cron { expression } => {
                match parse_cron_to_expansion(expression) {
                    Ok(expansion) => match expansion {
                        CronExpansion::Interval { seconds } =>
                        {
                            #[allow(clippy::cast_possible_wrap)]
                            plist::Value::Integer((seconds as i64).into())
                        }
                        CronExpansion::Calendars(entries) => {
                            if entries.len() == 1 {
                                plist::Value::Dictionary(entries[0].to_plist_dict())
                            } else {
                                let arr: Vec<plist::Value> = entries
                                    .iter()
                                    .map(|e| plist::Value::Dictionary(e.to_plist_dict()))
                                    .collect();
                                plist::Value::Array(arr)
                            }
                        }
                    },
                    Err(e) => {
                        // Best-effort: log warning and return an empty dict.
                        warn!(
                            expression,
                            error = %e,
                            "Failed to parse cron expression, using empty calendar interval"
                        );
                        plist::Value::Dictionary(plist::Dictionary::new())
                    }
                }
            }
        }
    }

    /// Parse a 5-field cron expression into a `Schedule` enum.
    ///
    /// Handles common patterns:
    /// - `*/N * * * *` -> `Schedule::Interval { seconds: N * 60 }`
    /// - `0 */N * * *` -> `Schedule::Interval { seconds: N * 3600 }`
    /// - `M H * * *` -> `Schedule::Calendar { minute: M, hour: H }`
    /// - `M H * * D` -> `Schedule::Calendar { weekday: D, ... }`
    /// - `M H D * *` -> `Schedule::Calendar { day: D, ... }`
    ///
    /// For weekday ranges (e.g. `0 7 * * 1-5`), this returns the first
    /// weekday's calendar entry as a `Schedule::Calendar`. To get the full
    /// expansion with all weekdays, use the plist generation path which handles
    /// ranges internally via `schedule_to_plist` on the `Cron` variant.
    ///
    /// # Errors
    ///
    /// Returns an error if the cron expression has an invalid format.
    pub fn cron_to_schedule(&self, cron: &str) -> Result<Schedule> {
        let expansion = parse_cron_to_expansion(cron)?;
        match expansion {
            CronExpansion::Interval { seconds } => Ok(Schedule::Interval { seconds }),
            CronExpansion::Calendars(entries) => {
                let e = &entries[0];
                Ok(Schedule::Calendar {
                    minute: e.minute,
                    hour: e.hour,
                    day: e.day,
                    weekday: e.weekday,
                    month: e.month,
                })
            }
        }
    }

    /// Build the complete plist dictionary for a task.
    fn build_plist(&self, task: &Task) -> plist::Dictionary {
        let label = task.id.launchd_label();
        let mut dict = plist::Dictionary::new();

        // Label
        dict.insert("Label".into(), plist::Value::String(label));

        // ProgramArguments
        let runner_str = self.runner_path.to_string_lossy().into_owned();
        let program_args = vec![
            plist::Value::String(runner_str),
            plist::Value::String("--task-id".into()),
            plist::Value::String(task.id.as_str().to_string()),
        ];
        dict.insert("ProgramArguments".into(), plist::Value::Array(program_args));

        // Schedule
        let schedule_value = self.schedule_to_plist(&task.schedule);
        match &task.schedule {
            Schedule::Interval { .. } => {
                dict.insert("StartInterval".into(), schedule_value);
            }
            Schedule::Calendar { .. } => {
                dict.insert("StartCalendarInterval".into(), schedule_value);
            }
            Schedule::Cron { expression } => {
                // Determine the correct key based on parsing.
                match parse_cron_to_expansion(expression) {
                    Ok(CronExpansion::Interval { .. }) => {
                        dict.insert("StartInterval".into(), schedule_value);
                    }
                    _ => {
                        dict.insert("StartCalendarInterval".into(), schedule_value);
                    }
                }
            }
        }

        // WorkingDirectory
        let working_dir = task.working_dir.to_string_lossy().into_owned();
        dict.insert("WorkingDirectory".into(), plist::Value::String(working_dir));

        // StandardOutPath
        let stdout_path = self
            .paths
            .output_dir
            .join(format!("{}.stdout.log", task.id.as_str()));
        dict.insert(
            "StandardOutPath".into(),
            plist::Value::String(stdout_path.to_string_lossy().into_owned()),
        );

        // StandardErrorPath
        let stderr_path = self
            .paths
            .output_dir
            .join(format!("{}.stderr.log", task.id.as_str()));
        dict.insert(
            "StandardErrorPath".into(),
            plist::Value::String(stderr_path.to_string_lossy().into_owned()),
        );

        // RunAtLoad
        dict.insert("RunAtLoad".into(), plist::Value::Boolean(false));

        // KeepAlive
        dict.insert("KeepAlive".into(), plist::Value::Boolean(false));

        // EnvironmentVariables -- always include PATH so that intern-runner's child
        // processes (e.g. `claude`) can be found. launchd provides only a minimal
        // PATH (/usr/bin:/bin:/usr/sbin:/sbin) which excludes user-local dirs.
        {
            let mut env_dict = plist::Dictionary::new();

            // Capture the current PATH at registration time so launchd jobs
            // inherit the user's full PATH (includes ~/.local/bin, Homebrew, etc.).
            if !task.env_vars.contains_key("PATH") {
                if let Ok(path) = std::env::var("PATH") {
                    env_dict.insert("PATH".into(), plist::Value::String(path));
                }
            }

            // Also ensure HOME is set -- launchd doesn't always provide it.
            if !task.env_vars.contains_key("HOME") {
                if let Ok(home) = std::env::var("HOME") {
                    env_dict.insert("HOME".into(), plist::Value::String(home));
                }
            }

            for (key, value) in &task.env_vars {
                env_dict.insert(key.clone(), plist::Value::String(value.clone()));
            }

            if !env_dict.is_empty() {
                dict.insert(
                    "EnvironmentVariables".into(),
                    plist::Value::Dictionary(env_dict),
                );
            }
        }

        dict
    }
}

/// Search for the `intern-runner` binary in standard locations.
///
/// Search order:
/// 1. Same directory as the current executable
/// 2. `~/.cargo/bin/intern-runner`
/// 3. `/usr/local/bin/intern-runner`
/// 4. `which intern-runner` (via `std::process::Command`)
fn find_runner() -> Result<PathBuf> {
    // 1. Same directory as current executable.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("intern-runner");
            if candidate.is_file() {
                debug!(
                    ?candidate,
                    "Found intern-runner alongside current executable"
                );
                return Ok(candidate);
            }
        }
    }

    // 2. ~/.cargo/bin/intern-runner
    if let Some(home) = dirs::home_dir() {
        let candidate = home.join(".cargo/bin/intern-runner");
        if candidate.is_file() {
            debug!(?candidate, "Found intern-runner in ~/.cargo/bin/");
            return Ok(candidate);
        }
    }

    // 3. /usr/local/bin/intern-runner
    let candidate = PathBuf::from("/usr/local/bin/intern-runner");
    if candidate.is_file() {
        debug!(?candidate, "Found intern-runner in /usr/local/bin/");
        return Ok(candidate);
    }

    // 4. `which intern-runner`
    if let Ok(output) = Command::new("which").arg("intern-runner").output() {
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path_str.is_empty() {
                let candidate = PathBuf::from(&path_str);
                if candidate.is_file() {
                    debug!(?candidate, "Found intern-runner via `which`");
                    return Ok(candidate);
                }
            }
        }
    }

    anyhow::bail!(
        "Could not find intern-runner binary. Searched:\n  \
         1. Same directory as current executable\n  \
         2. ~/.cargo/bin/intern-runner\n  \
         3. /usr/local/bin/intern-runner\n  \
         4. `which intern-runner`\n\
         Please ensure intern-runner is built and available in one of these locations."
    )
}

/// Get the current user's UID by shelling out to `id -u`.
fn get_uid() -> Result<u32> {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .context("Failed to execute `id -u`")?;

    if !output.status.success() {
        anyhow::bail!(
            "`id -u` failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let uid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    uid_str
        .parse::<u32>()
        .with_context(|| format!("Failed to parse UID from `id -u` output: {uid_str}"))
}

/// Parse a 5-field cron expression into a `CronExpansion`.
///
/// Supports the following patterns:
/// - `*/N * * * *` -> interval of N minutes
/// - `0 */N * * *` -> interval of N hours
/// - `M H * * *` -> daily at H:M
/// - `M H * * D` -> specific weekday at H:M
/// - `M H * * D1-D2` -> range of weekdays at H:M (multiple calendar entries)
/// - `M H D * *` -> specific day of month at H:M
/// - `M H * MO *` -> specific month at H:M
fn parse_cron_to_expansion(cron: &str) -> Result<CronExpansion> {
    let fields: Vec<&str> = cron.split_whitespace().collect();
    if fields.len() != 5 {
        anyhow::bail!(
            "Invalid cron expression: expected 5 fields, got {}: '{cron}'",
            fields.len()
        );
    }

    let (minute_f, hour_f, day_f, month_f, weekday_f) =
        (fields[0], fields[1], fields[2], fields[3], fields[4]);

    // Pattern: */N * * * * -> interval of N minutes
    if let Some(n_str) = minute_f.strip_prefix("*/") {
        if hour_f == "*" && day_f == "*" && month_f == "*" && weekday_f == "*" {
            let n: u64 = n_str
                .parse()
                .with_context(|| format!("Invalid minute interval: {n_str}"))?;
            if n == 0 {
                anyhow::bail!("Invalid cron expression: */0 is not valid");
            }
            return Ok(CronExpansion::Interval { seconds: n * 60 });
        }
    }

    // Pattern: 0 */N * * * -> interval of N hours
    if minute_f == "0" {
        if let Some(n_str) = hour_f.strip_prefix("*/") {
            if day_f == "*" && month_f == "*" && weekday_f == "*" {
                let n: u64 = n_str
                    .parse()
                    .with_context(|| format!("Invalid hour interval: {n_str}"))?;
                if n == 0 {
                    anyhow::bail!("Invalid cron expression: 0 */0 is not valid");
                }
                return Ok(CronExpansion::Interval { seconds: n * 3600 });
            }
        }
    }

    // Parse minute and hour (required for calendar patterns).
    let minute = parse_cron_field(minute_f, "minute", 0, 59)?;
    let hour = parse_cron_field(hour_f, "hour", 0, 23)?;

    // Parse day of month.
    let day = if day_f == "*" {
        None
    } else {
        Some(
            day_f
                .parse::<u8>()
                .with_context(|| format!("Invalid day of month: {day_f}"))?,
        )
    };

    // Parse month.
    let month = if month_f == "*" {
        None
    } else {
        Some(
            month_f
                .parse::<u8>()
                .with_context(|| format!("Invalid month: {month_f}"))?,
        )
    };

    // Parse weekday field (possibly a range) and build calendar entries.
    parse_weekday_field(weekday_f, minute, hour, day, month)
}

/// Parse the weekday field of a cron expression and produce one or more
/// `CalendarEntry` values. Handles wildcards, single values, and ranges.
fn parse_weekday_field(
    weekday_f: &str,
    minute: Option<u8>,
    hour: Option<u8>,
    day: Option<u8>,
    month: Option<u8>,
) -> Result<CronExpansion> {
    if weekday_f == "*" {
        return Ok(CronExpansion::Calendars(vec![CalendarEntry {
            minute,
            hour,
            day,
            weekday: None,
            month,
        }]));
    }

    // Range: D1-D2
    if let Some((start_str, end_str)) = weekday_f.split_once('-') {
        let start: u8 = start_str
            .parse()
            .with_context(|| format!("Invalid weekday range start: {start_str}"))?;
        let end: u8 = end_str
            .parse()
            .with_context(|| format!("Invalid weekday range end: {end_str}"))?;

        if start > 7 || end > 7 {
            anyhow::bail!("Invalid weekday range: {weekday_f} (weekdays must be 0-7)");
        }

        let entries = expand_weekday_range(start, end, minute, hour, day, month);
        if entries.is_empty() {
            anyhow::bail!("Invalid weekday range produced no entries: {weekday_f}");
        }

        return Ok(CronExpansion::Calendars(entries));
    }

    // Single weekday.
    let weekday: u8 = weekday_f
        .parse()
        .with_context(|| format!("Invalid weekday: {weekday_f}"))?;

    if weekday > 7 {
        anyhow::bail!("Invalid weekday: {weekday} (must be 0-7)");
    }

    Ok(CronExpansion::Calendars(vec![CalendarEntry {
        minute,
        hour,
        day,
        weekday: Some(weekday),
        month,
    }]))
}

/// Expand a weekday range `start..=end` (with wrap-around support) into a
/// vector of `CalendarEntry` values.
fn expand_weekday_range(
    start: u8,
    end: u8,
    minute: Option<u8>,
    hour: Option<u8>,
    day: Option<u8>,
    month: Option<u8>,
) -> Vec<CalendarEntry> {
    let mut entries = Vec::new();
    if start <= end {
        for wd in start..=end {
            entries.push(CalendarEntry {
                minute,
                hour,
                day,
                weekday: Some(wd),
                month,
            });
        }
    } else {
        // Wrap-around range like 5-2 means Fri,Sat,Sun,Mon,Tue
        for wd in start..=7 {
            let normalized = if wd == 7 { 0 } else { wd };
            entries.push(CalendarEntry {
                minute,
                hour,
                day,
                weekday: Some(normalized),
                month,
            });
        }
        for wd in 0..=end {
            entries.push(CalendarEntry {
                minute,
                hour,
                day,
                weekday: Some(wd),
                month,
            });
        }
    }
    entries
}

/// Parse a single cron field that should be either `*` (wildcard) or a numeric
/// value within `[min, max]`.
fn parse_cron_field(field: &str, name: &str, min: u8, max: u8) -> Result<Option<u8>> {
    if field == "*" {
        return Ok(None);
    }
    let val: u8 = field
        .parse()
        .with_context(|| format!("Invalid {name} field: {field}"))?;
    if val < min || val > max {
        anyhow::bail!("{name} field out of range [{min}-{max}]: {val}");
    }
    Ok(Some(val))
}

#[cfg(test)]
mod tests {
    use super::*;
    use intern_core::{Schedule, Task, TaskId, TaskStatus};
    use std::collections::HashMap;

    /// Helper: build a minimal Task for testing.
    fn test_task(schedule: Schedule) -> Task {
        Task {
            id: TaskId("lc-testtest".into()),
            name: "Test Task".into(),
            command: "echo hello".into(),
            skill: None,
            schedule,
            schedule_human: "test".into(),
            working_dir: "/tmp/lc-test".into(),
            env_vars: HashMap::new(),
            max_budget_per_run: 5.0,
            max_turns: None,
            timeout_secs: 600,
            status: TaskStatus::Active,
            tags: vec![],
            agents: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    /// Helper: build a Task with env vars for testing.
    fn test_task_with_env(schedule: Schedule) -> Task {
        let mut env = HashMap::new();
        env.insert("FOO".into(), "bar".into());
        env.insert("BAZ".into(), "qux".into());
        Task {
            id: TaskId("lc-envvtest".into()),
            name: "Env Test Task".into(),
            command: "echo hello".into(),
            skill: None,
            schedule,
            schedule_human: "test".into(),
            working_dir: "/tmp/lc-test".into(),
            env_vars: env,
            max_budget_per_run: 5.0,
            max_turns: None,
            timeout_secs: 600,
            status: TaskStatus::Active,
            tags: vec![],
            agents: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    /// Helper: create a Scheduler using a temporary directory, with a dummy
    /// runner path to bypass find_runner() discovery.
    fn test_scheduler(dir: &std::path::Path) -> Scheduler {
        let paths = InternPaths::with_root(dir.to_path_buf());
        paths.ensure_dirs().expect("ensure_dirs");
        Scheduler {
            paths,
            runner_path: PathBuf::from("/usr/local/bin/intern-runner"),
        }
    }

    // ── Cron parsing tests ──────────────────────────────────

    #[test]
    fn cron_every_15_minutes() {
        let sched = test_scheduler(std::path::Path::new("/tmp/lc-sched-test-1"));
        let result = sched.cron_to_schedule("*/15 * * * *").unwrap();
        match result {
            Schedule::Interval { seconds } => assert_eq!(seconds, 900),
            other => panic!("Expected Interval, got {other:?}"),
        }
    }

    #[test]
    fn cron_every_2_hours() {
        let sched = test_scheduler(std::path::Path::new("/tmp/lc-sched-test-2"));
        let result = sched.cron_to_schedule("0 */2 * * *").unwrap();
        match result {
            Schedule::Interval { seconds } => assert_eq!(seconds, 7200),
            other => panic!("Expected Interval, got {other:?}"),
        }
    }

    #[test]
    fn cron_weekdays_7am_produces_5_entries() {
        let expansion = parse_cron_to_expansion("0 7 * * 1-5").unwrap();
        match expansion {
            CronExpansion::Calendars(entries) => {
                assert_eq!(entries.len(), 5, "Expected 5 calendar entries for Mon-Fri");
                for (i, entry) in entries.iter().enumerate() {
                    let expected_wd = (i as u8) + 1;
                    assert_eq!(entry.weekday, Some(expected_wd));
                    assert_eq!(entry.hour, Some(7));
                    assert_eq!(entry.minute, Some(0));
                }
            }
            other => panic!("Expected Calendars, got {other:?}"),
        }
    }

    #[test]
    fn cron_midnight_daily() {
        let sched = test_scheduler(std::path::Path::new("/tmp/lc-sched-test-4"));
        let result = sched.cron_to_schedule("0 0 * * *").unwrap();
        match result {
            Schedule::Calendar {
                minute,
                hour,
                day,
                weekday,
                month,
            } => {
                assert_eq!(minute, Some(0));
                assert_eq!(hour, Some(0));
                assert_eq!(day, None);
                assert_eq!(weekday, None);
                assert_eq!(month, None);
            }
            other => panic!("Expected Calendar, got {other:?}"),
        }
    }

    #[test]
    fn cron_monday_930am() {
        let sched = test_scheduler(std::path::Path::new("/tmp/lc-sched-test-5"));
        let result = sched.cron_to_schedule("30 9 * * 1").unwrap();
        match result {
            Schedule::Calendar {
                minute,
                hour,
                weekday,
                ..
            } => {
                assert_eq!(minute, Some(30));
                assert_eq!(hour, Some(9));
                assert_eq!(weekday, Some(1));
            }
            other => panic!("Expected Calendar, got {other:?}"),
        }
    }

    #[test]
    fn cron_invalid_expression_returns_error() {
        let sched = test_scheduler(std::path::Path::new("/tmp/lc-sched-test-6"));
        let result = sched.cron_to_schedule("not a cron");
        assert!(result.is_err(), "Expected error for invalid cron");
    }

    #[test]
    fn cron_too_few_fields_returns_error() {
        let sched = test_scheduler(std::path::Path::new("/tmp/lc-sched-test-7"));
        let result = sched.cron_to_schedule("*/5 *");
        assert!(result.is_err(), "Expected error for too few fields");
    }

    #[test]
    fn cron_too_many_fields_returns_error() {
        let sched = test_scheduler(std::path::Path::new("/tmp/lc-sched-test-8"));
        let result = sched.cron_to_schedule("* * * * * *");
        assert!(result.is_err(), "Expected error for too many fields");
    }

    #[test]
    fn cron_with_day_of_month() {
        let sched = test_scheduler(std::path::Path::new("/tmp/lc-sched-test-9"));
        let result = sched.cron_to_schedule("0 12 15 * *").unwrap();
        match result {
            Schedule::Calendar {
                minute,
                hour,
                day,
                weekday,
                ..
            } => {
                assert_eq!(minute, Some(0));
                assert_eq!(hour, Some(12));
                assert_eq!(day, Some(15));
                assert_eq!(weekday, None);
            }
            other => panic!("Expected Calendar, got {other:?}"),
        }
    }

    #[test]
    fn cron_with_month() {
        let sched = test_scheduler(std::path::Path::new("/tmp/lc-sched-test-10"));
        let result = sched.cron_to_schedule("0 0 * 6 *").unwrap();
        match result {
            Schedule::Calendar { month, .. } => {
                assert_eq!(month, Some(6));
            }
            other => panic!("Expected Calendar, got {other:?}"),
        }
    }

    // ── schedule_to_plist tests ─────────────────────────────

    #[test]
    fn plist_interval_schedule() {
        let sched = test_scheduler(std::path::Path::new("/tmp/lc-sched-test-11"));
        let schedule = Schedule::Interval { seconds: 900 };
        let value = sched.schedule_to_plist(&schedule);
        assert_eq!(value.as_signed_integer(), Some(900));
    }

    #[test]
    fn plist_calendar_schedule() {
        let sched = test_scheduler(std::path::Path::new("/tmp/lc-sched-test-12"));
        let schedule = Schedule::Calendar {
            minute: Some(30),
            hour: Some(9),
            day: None,
            weekday: Some(1),
            month: None,
        };
        let value = sched.schedule_to_plist(&schedule);
        match value {
            plist::Value::Dictionary(dict) => {
                assert_eq!(
                    dict.get("Minute").and_then(|v| v.as_signed_integer()),
                    Some(30)
                );
                assert_eq!(
                    dict.get("Hour").and_then(|v| v.as_signed_integer()),
                    Some(9)
                );
                assert_eq!(
                    dict.get("Weekday").and_then(|v| v.as_signed_integer()),
                    Some(1)
                );
                assert!(dict.get("Day").is_none());
                assert!(dict.get("Month").is_none());
            }
            other => panic!("Expected Dictionary, got {other:?}"),
        }
    }

    #[test]
    fn plist_cron_interval_schedule() {
        let sched = test_scheduler(std::path::Path::new("/tmp/lc-sched-test-13"));
        let schedule = Schedule::Cron {
            expression: "*/15 * * * *".into(),
        };
        let value = sched.schedule_to_plist(&schedule);
        assert_eq!(value.as_signed_integer(), Some(900));
    }

    #[test]
    fn plist_cron_weekday_range_produces_array() {
        let sched = test_scheduler(std::path::Path::new("/tmp/lc-sched-test-14"));
        let schedule = Schedule::Cron {
            expression: "0 7 * * 1-5".into(),
        };
        let value = sched.schedule_to_plist(&schedule);
        match value {
            plist::Value::Array(arr) => {
                assert_eq!(arr.len(), 5, "Expected 5 calendar interval dicts");
                for (i, item) in arr.iter().enumerate() {
                    let dict = item.as_dictionary().expect("Expected dict");
                    assert_eq!(
                        dict.get("Hour").and_then(|v| v.as_signed_integer()),
                        Some(7)
                    );
                    assert_eq!(
                        dict.get("Minute").and_then(|v| v.as_signed_integer()),
                        Some(0)
                    );
                    assert_eq!(
                        dict.get("Weekday").and_then(|v| v.as_signed_integer()),
                        Some((i as i64) + 1)
                    );
                }
            }
            other => panic!("Expected Array for weekday range, got {other:?}"),
        }
    }

    // ── Plist generation tests ──────────────────────────────

    #[test]
    fn plist_generation_interval_contains_start_interval() {
        let dir = tempfile::tempdir().unwrap();
        let sched = test_scheduler(dir.path());
        let task = test_task(Schedule::Interval { seconds: 600 });

        let dict = sched.build_plist(&task);

        assert!(
            dict.contains_key("StartInterval"),
            "Plist must contain StartInterval"
        );
        assert!(
            !dict.contains_key("StartCalendarInterval"),
            "Plist must NOT contain StartCalendarInterval for interval schedule"
        );

        let si = dict.get("StartInterval").unwrap();
        assert_eq!(si.as_signed_integer(), Some(600));

        // Verify other required keys.
        assert_eq!(
            dict.get("Label").and_then(|v| v.as_string()),
            Some("com.intern.task.lc-testtest")
        );
        assert_eq!(
            dict.get("RunAtLoad").and_then(|v| v.as_boolean()),
            Some(false)
        );
        assert_eq!(
            dict.get("KeepAlive").and_then(|v| v.as_boolean()),
            Some(false)
        );

        // ProgramArguments
        let args = dict
            .get("ProgramArguments")
            .and_then(|v| v.as_array())
            .unwrap();
        assert_eq!(args.len(), 3);
        assert_eq!(args[1].as_string(), Some("--task-id"));
        assert_eq!(args[2].as_string(), Some("lc-testtest"));
    }

    #[test]
    fn plist_generation_calendar_contains_start_calendar_interval() {
        let dir = tempfile::tempdir().unwrap();
        let sched = test_scheduler(dir.path());
        let task = test_task(Schedule::Calendar {
            minute: Some(0),
            hour: Some(7),
            day: None,
            weekday: None,
            month: None,
        });

        let dict = sched.build_plist(&task);

        assert!(
            dict.contains_key("StartCalendarInterval"),
            "Plist must contain StartCalendarInterval"
        );
        assert!(
            !dict.contains_key("StartInterval"),
            "Plist must NOT contain StartInterval for calendar schedule"
        );

        let sci = dict
            .get("StartCalendarInterval")
            .unwrap()
            .as_dictionary()
            .unwrap();
        assert_eq!(sci.get("Hour").and_then(|v| v.as_signed_integer()), Some(7));
        assert_eq!(
            sci.get("Minute").and_then(|v| v.as_signed_integer()),
            Some(0)
        );
    }

    #[test]
    fn plist_generation_includes_env_vars() {
        let dir = tempfile::tempdir().unwrap();
        let sched = test_scheduler(dir.path());
        let task = test_task_with_env(Schedule::Interval { seconds: 300 });

        let dict = sched.build_plist(&task);

        let env = dict
            .get("EnvironmentVariables")
            .and_then(|v| v.as_dictionary())
            .expect("EnvironmentVariables should be present");
        assert_eq!(env.get("FOO").and_then(|v| v.as_string()), Some("bar"));
        assert_eq!(env.get("BAZ").and_then(|v| v.as_string()), Some("qux"));
    }

    #[test]
    fn plist_generation_includes_path_and_home() {
        let dir = tempfile::tempdir().unwrap();
        let sched = test_scheduler(dir.path());
        let task = test_task(Schedule::Interval { seconds: 300 });

        let dict = sched.build_plist(&task);

        // Even with no task-level env_vars, the plist should include PATH and
        // HOME so that intern-runner's child processes can find user-local binaries.
        let env = dict
            .get("EnvironmentVariables")
            .and_then(|v| v.as_dictionary())
            .expect("EnvironmentVariables should be present");

        assert!(
            env.contains_key("PATH"),
            "PATH should be injected into EnvironmentVariables"
        );
        assert!(
            env.contains_key("HOME"),
            "HOME should be injected into EnvironmentVariables"
        );
    }

    #[test]
    fn plist_round_trip_write_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let sched = test_scheduler(dir.path());
        let task = test_task(Schedule::Interval { seconds: 1800 });

        // Register writes the plist and creates symlink.
        sched.register(&task).expect("register should succeed");

        let label = task.id.launchd_label();
        let plist_filename = format!("{label}.plist");
        let plist_path = sched.paths.plists_dir.join(&plist_filename);

        // Verify file exists.
        assert!(
            plist_path.exists(),
            "Plist file should exist after register"
        );

        // Read it back.
        let file = std::fs::File::open(&plist_path).unwrap();
        let read_back: plist::Value = plist::from_reader(file).unwrap();
        let dict = read_back.as_dictionary().unwrap();

        // Verify key fields.
        assert_eq!(
            dict.get("Label").and_then(|v| v.as_string()),
            Some("com.intern.task.lc-testtest")
        );
        assert_eq!(
            dict.get("StartInterval")
                .and_then(|v| v.as_signed_integer()),
            Some(1800)
        );
        assert_eq!(
            dict.get("RunAtLoad").and_then(|v| v.as_boolean()),
            Some(false)
        );
        assert_eq!(
            dict.get("KeepAlive").and_then(|v| v.as_boolean()),
            Some(false)
        );
        assert!(dict.get("WorkingDirectory").is_some());
        assert!(dict.get("StandardOutPath").is_some());
        assert!(dict.get("StandardErrorPath").is_some());

        // Verify symlink exists.
        let symlink_path = sched.paths.launch_agents_dir.join(&plist_filename);
        assert!(
            symlink_path.symlink_metadata().is_ok(),
            "Symlink should exist after register"
        );
    }

    #[test]
    fn plist_cron_schedule_produces_correct_key() {
        let dir = tempfile::tempdir().unwrap();
        let sched = test_scheduler(dir.path());

        // Cron that maps to Interval.
        let task_interval = test_task(Schedule::Cron {
            expression: "*/10 * * * *".into(),
        });
        let dict_interval = sched.build_plist(&task_interval);
        assert!(
            dict_interval.contains_key("StartInterval"),
            "Cron */10 should produce StartInterval"
        );
        assert_eq!(
            dict_interval
                .get("StartInterval")
                .and_then(|v| v.as_signed_integer()),
            Some(600)
        );

        // Cron that maps to Calendar.
        let mut task_calendar = test_task(Schedule::Cron {
            expression: "30 9 * * 1".into(),
        });
        task_calendar.id = TaskId("lc-testcal1".into());
        let dict_calendar = sched.build_plist(&task_calendar);
        assert!(
            dict_calendar.contains_key("StartCalendarInterval"),
            "Cron 30 9 * * 1 should produce StartCalendarInterval"
        );
    }

    #[test]
    fn unregister_removes_files() {
        let dir = tempfile::tempdir().unwrap();
        let sched = test_scheduler(dir.path());
        let task = test_task(Schedule::Interval { seconds: 300 });

        sched.register(&task).expect("register");

        let label = task.id.launchd_label();
        let plist_filename = format!("{label}.plist");
        let plist_path = sched.paths.plists_dir.join(&plist_filename);
        let symlink_path = sched.paths.launch_agents_dir.join(&plist_filename);

        assert!(plist_path.exists());
        assert!(symlink_path.symlink_metadata().is_ok());

        sched
            .unregister(task.id.as_str())
            .expect("unregister should succeed");

        assert!(!plist_path.exists(), "Plist file should be removed");
        assert!(
            symlink_path.symlink_metadata().is_err(),
            "Symlink should be removed"
        );
    }

    #[test]
    fn register_replaces_stale_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let sched = test_scheduler(dir.path());
        let task = test_task(Schedule::Interval { seconds: 300 });

        // First register.
        sched.register(&task).expect("first register");

        // Second register should succeed (replaces stale symlink).
        sched
            .register(&task)
            .expect("second register should succeed");

        let label = task.id.launchd_label();
        let plist_filename = format!("{label}.plist");
        let symlink_path = sched.paths.launch_agents_dir.join(&plist_filename);
        assert!(
            symlink_path.symlink_metadata().is_ok(),
            "Symlink should exist"
        );
    }

    // ── validate_cron tests ─────────────────────────────────────

    #[test]
    fn validate_cron_accepts_weekday_range() {
        assert!(
            super::validate_cron("0 9 * * 1-5").is_ok(),
            "Mon-Fri at 09:00 should be valid"
        );
    }

    #[test]
    fn validate_cron_accepts_every_15_minutes() {
        assert!(
            super::validate_cron("*/15 * * * *").is_ok(),
            "Every 15 minutes should be valid"
        );
    }

    #[test]
    fn validate_cron_accepts_midnight_daily() {
        assert!(
            super::validate_cron("0 0 * * *").is_ok(),
            "Daily at midnight should be valid"
        );
    }

    #[test]
    fn validate_cron_rejects_plain_text() {
        assert!(
            super::validate_cron("not a cron expression").is_err(),
            "Plain text must be rejected"
        );
    }

    #[test]
    fn validate_cron_rejects_too_few_fields() {
        assert!(
            super::validate_cron("*/5 *").is_err(),
            "Two-field expression must be rejected"
        );
    }

    #[test]
    fn validate_cron_error_message_is_non_empty() {
        let err = super::validate_cron("bad").unwrap_err();
        assert!(!err.is_empty(), "Error message must be non-empty");
    }

    #[test]
    fn cron_zero_interval_is_error() {
        assert!(parse_cron_to_expansion("*/0 * * * *").is_err());
    }

    #[test]
    fn cron_zero_hour_interval_is_error() {
        assert!(parse_cron_to_expansion("0 */0 * * *").is_err());
    }

    #[test]
    fn cron_every_1_minute() {
        let expansion = parse_cron_to_expansion("*/1 * * * *").unwrap();
        match expansion {
            CronExpansion::Interval { seconds } => assert_eq!(seconds, 60),
            other => panic!("Expected Interval, got {other:?}"),
        }
    }

    #[test]
    fn cron_every_1_hour() {
        let expansion = parse_cron_to_expansion("0 */1 * * *").unwrap();
        match expansion {
            CronExpansion::Interval { seconds } => assert_eq!(seconds, 3600),
            other => panic!("Expected Interval, got {other:?}"),
        }
    }

    #[test]
    fn plist_generation_cron_weekday_range_full() {
        let dir = tempfile::tempdir().unwrap();
        let sched = test_scheduler(dir.path());
        let mut task = test_task(Schedule::Cron {
            expression: "0 7 * * 1-5".into(),
        });
        task.id = TaskId("lc-wdaytest".into());

        let dict = sched.build_plist(&task);
        assert!(
            dict.contains_key("StartCalendarInterval"),
            "Must have StartCalendarInterval for weekday range"
        );

        let sci = dict.get("StartCalendarInterval").unwrap();
        let arr = sci.as_array().expect("Weekday range must produce an array");
        assert_eq!(arr.len(), 5);
    }

    #[test]
    fn plist_working_dir_and_log_paths() {
        let dir = tempfile::tempdir().unwrap();
        let sched = test_scheduler(dir.path());
        let task = test_task(Schedule::Interval { seconds: 60 });

        let dict = sched.build_plist(&task);

        assert_eq!(
            dict.get("WorkingDirectory").and_then(|v| v.as_string()),
            Some("/tmp/lc-test")
        );

        let stdout = dict
            .get("StandardOutPath")
            .and_then(|v| v.as_string())
            .unwrap();
        assert!(
            stdout.contains("lc-testtest.stdout.log"),
            "stdout path: {stdout}"
        );

        let stderr = dict
            .get("StandardErrorPath")
            .and_then(|v| v.as_string())
            .unwrap();
        assert!(
            stderr.contains("lc-testtest.stderr.log"),
            "stderr path: {stderr}"
        );
    }
}

//! SQLite persistence layer for Intern execution logs.
//!
//! `Logger` manages the SQLite database at `~/.intern/logs.db`.
//! It uses WAL mode for concurrent reads (CC-5) and provides methods for
//! inserting, querying, aggregating, and pruning execution log records.

use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use intern_core::{DailyCost, DashboardMetrics, ExecStatus, ExecutionLog, LogQuery, Task, TaskMetrics};
use rusqlite::{params, Connection};
use tracing::info;

/// The current schema version understood by this binary.
const CURRENT_SCHEMA_VERSION: i64 = 1;

/// SQLite-backed logger for execution records.
///
/// Holds a single `rusqlite::Connection`. The daemon is the primary writer
/// (via `intern-runner`, which opens its own connection). The Swift app and CLI
/// are readers (via JSON-RPC queries to the daemon).
pub struct Logger {
    conn: Connection,
}

impl Logger {
    /// Open or create the SQLite database at `db_path`.
    ///
    /// Enables WAL journal mode and sets `busy_timeout` to 5000 ms (CC-5),
    /// then runs migrations to ensure the schema is up to date.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened, pragmas fail,
    /// or migrations encounter an incompatible schema version.
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("failed to open database at {}", db_path.display()))?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .context("failed to set WAL journal mode")?;
        conn.execute_batch("PRAGMA busy_timeout=5000;")
            .context("failed to set busy_timeout")?;

        let logger = Self { conn };
        logger.migrate().context("database migration failed")?;

        info!("Logger initialized at {}", db_path.display());
        Ok(logger)
    }

    /// Run schema migrations.
    ///
    /// If the `schema_version` table does not exist, creates all tables from
    /// scratch and inserts version 1. If it exists, reads the current version
    /// and applies any pending sequential migrations. Returns an error if the
    /// database version is newer than this binary understands.
    fn migrate(&self) -> Result<()> {
        let has_version_table: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_version'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .context("failed to check for schema_version table")?
            > 0;

        if !has_version_table {
            self.conn
                .execute_batch(
                    "
                    CREATE TABLE IF NOT EXISTS schema_version (
                        version INTEGER NOT NULL
                    );

                    CREATE TABLE IF NOT EXISTS execution_logs (
                        id              INTEGER PRIMARY KEY AUTOINCREMENT,
                        task_id         TEXT NOT NULL,
                        task_name       TEXT NOT NULL,
                        started_at      TEXT NOT NULL,
                        finished_at     TEXT NOT NULL,
                        duration_secs   INTEGER NOT NULL,
                        exit_code       INTEGER NOT NULL,
                        status          TEXT NOT NULL,
                        stdout          TEXT NOT NULL DEFAULT '',
                        stderr          TEXT NOT NULL DEFAULT '',
                        tokens_used     INTEGER,
                        cost_usd        REAL,
                        cost_is_estimate INTEGER NOT NULL DEFAULT 0,
                        summary         TEXT NOT NULL DEFAULT '',
                        created_at      TEXT NOT NULL DEFAULT (datetime('now'))
                    );

                    CREATE INDEX IF NOT EXISTS idx_logs_task_id ON execution_logs(task_id);
                    CREATE INDEX IF NOT EXISTS idx_logs_started ON execution_logs(started_at DESC);
                    CREATE INDEX IF NOT EXISTS idx_logs_status ON execution_logs(status);

                    INSERT INTO schema_version (version) VALUES (1);
                    ",
                )
                .context("failed to create initial schema")?;

            info!("Created database schema version 1");
            return Ok(());
        }

        // Schema version table exists — check version.
        let db_version: i64 = self
            .conn
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
                row.get(0)
            })
            .context("failed to read schema version")?;

        if db_version > CURRENT_SCHEMA_VERSION {
            anyhow::bail!(
                "database schema version {} is newer than this binary's version {}; upgrade the binary",
                db_version,
                CURRENT_SCHEMA_VERSION
            );
        }

        // Apply sequential migrations for versions between db_version and CURRENT_SCHEMA_VERSION.
        // Currently only version 1 exists, so no additional migrations are needed.
        // Future migrations would go here:
        // if db_version < 2 { apply_v2_migration(); update version to 2; }

        Ok(())
    }

    /// Insert a new execution log row.
    ///
    /// Returns the auto-generated row ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the INSERT statement fails.
    pub fn insert_log(&self, log: &ExecutionLog) -> Result<i64> {
        self.conn
            .execute(
                "INSERT INTO execution_logs (
                    task_id, task_name, started_at, finished_at, duration_secs,
                    exit_code, status, stdout, stderr, tokens_used, cost_usd,
                    cost_is_estimate, summary
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    log.task_id,
                    log.task_name,
                    log.started_at.to_rfc3339(),
                    log.finished_at.to_rfc3339(),
                    log.duration_secs as i64,
                    log.exit_code,
                    log.status.to_string(),
                    log.stdout,
                    log.stderr,
                    log.tokens_used.map(|t| t as i64),
                    log.cost_usd,
                    i32::from(log.cost_is_estimate),
                    log.summary,
                ],
            )
            .context("failed to insert execution log")?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Query logs with optional filters.
    ///
    /// Supports filtering by `task_id`, `status`, and `search` (LIKE on
    /// `task_name`, `stdout`, `stderr`, or `summary`). Results are ordered
    /// by `started_at DESC` and can be paginated with `limit` and `offset`.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn query_logs(&self, query: &LogQuery) -> Result<Vec<ExecutionLog>> {
        let mut sql = String::from("SELECT * FROM execution_logs WHERE 1=1");
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref task_id) = query.task_id {
            sql.push_str(" AND task_id = ?");
            param_values.push(Box::new(task_id.clone()));
        }

        if let Some(ref status) = query.status {
            sql.push_str(" AND status = ?");
            param_values.push(Box::new(status.clone()));
        }

        if let Some(ref search) = query.search {
            sql.push_str(
                " AND (task_name LIKE ? OR stdout LIKE ? OR stderr LIKE ? OR summary LIKE ?)",
            );
            let pattern = format!("%{search}%");
            param_values.push(Box::new(pattern.clone()));
            param_values.push(Box::new(pattern.clone()));
            param_values.push(Box::new(pattern.clone()));
            param_values.push(Box::new(pattern));
        }

        sql.push_str(" ORDER BY started_at DESC");

        if let Some(limit) = query.limit {
            sql.push_str(" LIMIT ?");
            param_values.push(Box::new(limit as i64));
        }

        if let Some(offset) = query.offset {
            // SQLite requires LIMIT before OFFSET. If no limit was provided,
            // use -1 (unlimited) so OFFSET works.
            if query.limit.is_none() {
                sql.push_str(" LIMIT -1");
            }
            sql.push_str(" OFFSET ?");
            param_values.push(Box::new(offset as i64));
        }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql).context("failed to prepare query")?;

        let logs = stmt
            .query_map(param_refs.as_slice(), |row| Ok(row_to_execution_log(row)))
            .context("failed to execute query")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to collect query results")?;

        Ok(logs)
    }

    /// Aggregate metrics across all tasks for the dashboard.
    ///
    /// Takes a slice of current tasks to compute `total_tasks` and `active_tasks`.
    /// Queries the database for per-task metrics and overall aggregates.
    ///
    /// # Errors
    ///
    /// Returns an error if any database query fails.
    pub fn get_dashboard_metrics(&self, tasks: &[Task]) -> Result<DashboardMetrics> {
        let total_tasks = tasks.len() as u64;
        let active_tasks = tasks
            .iter()
            .filter(|t| t.status == intern_core::TaskStatus::Active)
            .count() as u64;

        // Aggregate overall metrics from the database.
        let (total_runs, success_count, total_spend): (u64, u64, f64) = self
            .conn
            .query_row(
                "SELECT
                    COUNT(*) as total_runs,
                    COALESCE(SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END), 0) as success_count,
                    COALESCE(SUM(cost_usd), 0.0) as total_spend
                 FROM execution_logs",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)? as u64,
                        row.get::<_, i64>(1)? as u64,
                        row.get::<_, f64>(2)?,
                    ))
                },
            )
            .context("failed to query overall metrics")?;

        let overall_success_rate = if total_runs > 0 {
            (success_count as f64) / (total_runs as f64) * 100.0
        } else {
            0.0
        };

        // Get per-task metrics.
        let mut task_metrics = Vec::new();
        for task in tasks {
            let metrics = self
                .get_task_metrics(task.id.as_str())
                .with_context(|| format!("failed to get metrics for task {}", task.id))?;
            task_metrics.push(metrics);
        }

        // Get cost trend for last 7 days.
        let cost_trend = self.get_cost_trend(7).context("failed to get cost trend")?;

        Ok(DashboardMetrics {
            total_tasks,
            active_tasks,
            total_runs,
            overall_success_rate,
            total_spend,
            tasks: task_metrics,
            cost_trend,
        })
    }

    /// Aggregate metrics for a single task.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_task_metrics(&self, task_id: &str) -> Result<TaskMetrics> {
        let result = self.conn.query_row(
            "SELECT
                COUNT(*) as total_runs,
                COALESCE(SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END), 0) as success_count,
                COALESCE(SUM(CASE WHEN status != 'success' AND status != 'skipped' THEN 1 ELSE 0 END), 0) as fail_count,
                COALESCE(SUM(cost_usd), 0.0) as total_cost,
                COALESCE(SUM(tokens_used), 0) as total_tokens,
                COALESCE(AVG(duration_secs), 0.0) as avg_duration_secs,
                MAX(started_at) as last_run
             FROM execution_logs
             WHERE task_id = ?1",
            params![task_id],
            |row| {
                let last_run_str: Option<String> = row.get(6)?;
                Ok((
                    row.get::<_, i64>(0)? as u64,
                    row.get::<_, i64>(1)? as u64,
                    row.get::<_, i64>(2)? as u64,
                    row.get::<_, f64>(3)?,
                    row.get::<_, i64>(4)? as u64,
                    row.get::<_, f64>(5)?,
                    last_run_str,
                ))
            },
        ).context("failed to query task metrics")?;

        let last_run = result.6.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .ok()
        });

        Ok(TaskMetrics {
            task_id: task_id.to_string(),
            total_runs: result.0,
            success_count: result.1,
            fail_count: result.2,
            total_cost: result.3,
            total_tokens: result.4,
            avg_duration_secs: result.5,
            last_run,
        })
    }

    /// Total cost for a task since a given datetime.
    ///
    /// Returns the sum of `cost_usd` for all logs matching the given `task_id`
    /// with `started_at >= since`.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn total_cost_since(&self, task_id: &str, since: DateTime<Utc>) -> Result<f64> {
        let cost: f64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(cost_usd), 0.0) FROM execution_logs
                 WHERE task_id = ?1 AND started_at >= ?2",
                params![task_id, since.to_rfc3339()],
                |row| row.get(0),
            )
            .context("failed to query total cost since")?;

        Ok(cost)
    }

    /// Delete logs older than `retention_days` days.
    ///
    /// Returns the number of rows deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if the DELETE statement fails.
    pub fn prune_logs(&self, retention_days: u32) -> Result<u64> {
        let deleted = self
            .conn
            .execute(
                &format!(
                    "DELETE FROM execution_logs WHERE started_at < datetime('now', '-{retention_days} days')"
                ),
                [],
            )
            .context("failed to prune logs")?;

        info!(
            "Pruned {} log entries older than {} days",
            deleted, retention_days
        );
        Ok(deleted as u64)
    }

    /// Count total log entries.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn count_logs(&self) -> Result<u64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM execution_logs", [], |row| row.get(0))
            .context("failed to count logs")?;

        Ok(count as u64)
    }

    /// Aggregate daily costs for the last `days` days.
    ///
    /// Returns exactly `days` data points, backfilling missing days with
    /// zero values so the frontend always gets a complete series.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_cost_trend(&self, days: u32) -> Result<Vec<DailyCost>> {
        // Query actual data from the database.
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT DATE(started_at) as date,
                            COALESCE(SUM(cost_usd), 0.0) as total_cost,
                            COUNT(*) as run_count
                     FROM execution_logs
                     WHERE started_at >= datetime('now', '-{days} days')
                     GROUP BY DATE(started_at)
                     ORDER BY date ASC"
            ))
            .context("failed to prepare cost trend query")?;

        let db_rows: Vec<DailyCost> = stmt
            .query_map([], |row| {
                Ok(DailyCost {
                    date: row.get(0)?,
                    total_cost: row.get(1)?,
                    run_count: row.get::<_, i64>(2)? as u64,
                })
            })
            .context("failed to execute cost trend query")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to collect cost trend results")?;

        // Build a lookup map of date -> DailyCost from the query results.
        let mut cost_map: std::collections::HashMap<String, DailyCost> = db_rows
            .into_iter()
            .map(|dc| (dc.date.clone(), dc))
            .collect();

        // Backfill missing days with zeros so we return exactly `days` data points.
        let today = Utc::now().date_naive();
        let mut result = Vec::with_capacity(days as usize);

        for i in (0..days).rev() {
            let date = today - Duration::days(i64::from(i));
            let date_str = date.format("%Y-%m-%d").to_string();

            let entry = cost_map.remove(&date_str).unwrap_or(DailyCost {
                date: date_str,
                total_cost: 0.0,
                run_count: 0,
            });
            result.push(entry);
        }

        Ok(result)
    }
}

/// Convert a SQLite row to an `ExecutionLog`.
fn row_to_execution_log(row: &rusqlite::Row<'_>) -> ExecutionLog {
    let started_at_str: String = row.get_unwrap(3);
    let finished_at_str: String = row.get_unwrap(4);
    let status_str: String = row.get_unwrap(7);
    let cost_is_estimate_int: i32 = row.get_unwrap(12);

    let started_at = DateTime::parse_from_rfc3339(&started_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let finished_at = DateTime::parse_from_rfc3339(&finished_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let status: ExecStatus = status_str.parse().unwrap_or(ExecStatus::Failed);

    ExecutionLog {
        id: row.get_unwrap(0),
        task_id: row.get_unwrap(1),
        task_name: row.get_unwrap(2),
        started_at,
        finished_at,
        duration_secs: row.get::<_, i64>(5).unwrap_or(0) as u64,
        exit_code: row.get_unwrap(6),
        status,
        stdout: row.get_unwrap(8),
        stderr: row.get_unwrap(9),
        tokens_used: row
            .get::<_, Option<i64>>(10)
            .unwrap_or(None)
            .map(|v| v as u64),
        cost_usd: row.get_unwrap(11),
        cost_is_estimate: cost_is_estimate_int != 0,
        summary: row.get_unwrap(13),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use intern_core::TaskStatus;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Create a Logger backed by a temp directory.
    fn test_logger() -> (Logger, TempDir) {
        let dir = TempDir::new().expect("failed to create temp dir");
        let db_path = dir.path().join("test.db");
        let logger = Logger::new(&db_path).expect("failed to create logger");
        (logger, dir)
    }

    /// Create a sample ExecutionLog for testing.
    fn sample_log(task_id: &str, status: ExecStatus, cost: Option<f64>) -> ExecutionLog {
        ExecutionLog {
            id: 0,
            task_id: task_id.to_string(),
            task_name: format!("Task {task_id}"),
            started_at: Utc::now(),
            finished_at: Utc::now(),
            duration_secs: 42,
            exit_code: if status == ExecStatus::Success { 0 } else { 1 },
            status,
            stdout: "some output".to_string(),
            stderr: String::new(),
            tokens_used: Some(100),
            cost_usd: cost,
            cost_is_estimate: false,
            summary: "test summary".to_string(),
        }
    }

    /// Create a sample Task for dashboard metrics.
    fn sample_task(id: &str) -> Task {
        Task {
            id: intern_core::TaskId(id.to_string()),
            name: format!("Task {id}"),
            command: "echo hello".to_string(),
            skill: None,
            schedule: intern_core::Schedule::Interval { seconds: 300 },
            schedule_human: "Every 5m".to_string(),
            working_dir: PathBuf::from("/tmp"),
            env_vars: HashMap::new(),
            max_budget_per_run: 5.0,
            max_turns: None,
            timeout_secs: 600,
            status: TaskStatus::Active,
            tags: vec![],
            agents: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn db_creation_and_idempotent_migration() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");

        // First creation.
        let logger1 = Logger::new(&db_path).unwrap();
        drop(logger1);

        // Second open should also succeed (idempotent migration).
        let logger2 = Logger::new(&db_path).unwrap();
        drop(logger2);

        // Third open just to be safe.
        let _logger3 = Logger::new(&db_path).unwrap();
    }

    #[test]
    fn schema_version_stored() {
        let (logger, _dir) = test_logger();

        let version: i64 = logger
            .conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();

        assert_eq!(version, 1);
    }

    #[test]
    fn insert_and_query_roundtrip() {
        let (logger, _dir) = test_logger();

        let log = sample_log("lc-abc12345", ExecStatus::Success, Some(0.50));
        let id = logger.insert_log(&log).unwrap();
        assert!(id > 0);

        let query = LogQuery {
            task_id: None,
            status: None,
            limit: None,
            offset: None,
            search: None,
        };
        let results = logger.query_logs(&query).unwrap();
        assert_eq!(results.len(), 1);

        let result = &results[0];
        assert_eq!(result.id, id);
        assert_eq!(result.task_id, "lc-abc12345");
        assert_eq!(result.task_name, "Task lc-abc12345");
        assert_eq!(result.duration_secs, 42);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.status, ExecStatus::Success);
        assert_eq!(result.stdout, "some output");
        assert_eq!(result.stderr, "");
        assert_eq!(result.tokens_used, Some(100));
        assert!((result.cost_usd.unwrap() - 0.50).abs() < f64::EPSILON);
        assert!(!result.cost_is_estimate);
        assert_eq!(result.summary, "test summary");
    }

    #[test]
    fn insert_with_cost_is_estimate() {
        let (logger, _dir) = test_logger();

        let mut log = sample_log("lc-est00001", ExecStatus::Success, Some(1.23));
        log.cost_is_estimate = true;
        let id = logger.insert_log(&log).unwrap();

        let query = LogQuery {
            task_id: Some("lc-est00001".to_string()),
            ..Default::default()
        };
        let results = logger.query_logs(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
        assert!(results[0].cost_is_estimate);
    }

    #[test]
    fn query_filter_by_task_id() {
        let (logger, _dir) = test_logger();

        logger
            .insert_log(&sample_log("lc-task0001", ExecStatus::Success, Some(0.10)))
            .unwrap();
        logger
            .insert_log(&sample_log("lc-task0002", ExecStatus::Failed, Some(0.20)))
            .unwrap();
        logger
            .insert_log(&sample_log("lc-task0001", ExecStatus::Success, Some(0.15)))
            .unwrap();

        let query = LogQuery {
            task_id: Some("lc-task0001".to_string()),
            status: None,
            limit: None,
            offset: None,
            search: None,
        };
        let results = logger.query_logs(&query).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.task_id == "lc-task0001"));
    }

    #[test]
    fn query_filter_by_status() {
        let (logger, _dir) = test_logger();

        logger
            .insert_log(&sample_log("lc-task0001", ExecStatus::Success, None))
            .unwrap();
        logger
            .insert_log(&sample_log("lc-task0001", ExecStatus::Failed, None))
            .unwrap();
        logger
            .insert_log(&sample_log("lc-task0002", ExecStatus::Success, None))
            .unwrap();

        let query = LogQuery {
            task_id: None,
            status: Some("failed".to_string()),
            limit: None,
            offset: None,
            search: None,
        };
        let results = logger.query_logs(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, ExecStatus::Failed);
    }

    #[test]
    fn query_with_limit_and_offset() {
        let (logger, _dir) = test_logger();

        for i in 0..5 {
            let mut log = sample_log("lc-task0001", ExecStatus::Success, None);
            log.started_at = Utc::now() - Duration::seconds(i64::from(5 - i));
            logger.insert_log(&log).unwrap();
        }

        // Get first 2 results.
        let query = LogQuery {
            task_id: None,
            status: None,
            limit: Some(2),
            offset: None,
            search: None,
        };
        let results = logger.query_logs(&query).unwrap();
        assert_eq!(results.len(), 2);

        // Get next 2 results via offset.
        let query = LogQuery {
            task_id: None,
            status: None,
            limit: Some(2),
            offset: Some(2),
            search: None,
        };
        let results = logger.query_logs(&query).unwrap();
        assert_eq!(results.len(), 2);

        // Offset past all results.
        let query = LogQuery {
            task_id: None,
            status: None,
            limit: Some(10),
            offset: Some(10),
            search: None,
        };
        let results = logger.query_logs(&query).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn query_with_search() {
        let (logger, _dir) = test_logger();

        let mut log1 = sample_log("lc-task0001", ExecStatus::Success, None);
        log1.summary = "deployment succeeded".to_string();
        logger.insert_log(&log1).unwrap();

        let mut log2 = sample_log("lc-task0002", ExecStatus::Failed, None);
        log2.summary = "build failed".to_string();
        logger.insert_log(&log2).unwrap();

        let query = LogQuery {
            task_id: None,
            status: None,
            limit: None,
            offset: None,
            search: Some("deployment".to_string()),
        };
        let results = logger.query_logs(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].task_id, "lc-task0001");
    }

    #[test]
    fn dashboard_metrics_aggregation() {
        let (logger, _dir) = test_logger();

        let tasks = vec![sample_task("lc-task0001"), sample_task("lc-task0002")];

        // Insert logs for both tasks.
        logger
            .insert_log(&sample_log("lc-task0001", ExecStatus::Success, Some(1.00)))
            .unwrap();
        logger
            .insert_log(&sample_log("lc-task0001", ExecStatus::Success, Some(2.00)))
            .unwrap();
        logger
            .insert_log(&sample_log("lc-task0001", ExecStatus::Failed, Some(0.50)))
            .unwrap();
        logger
            .insert_log(&sample_log("lc-task0002", ExecStatus::Success, Some(3.00)))
            .unwrap();

        let metrics = logger.get_dashboard_metrics(&tasks).unwrap();

        assert_eq!(metrics.total_tasks, 2);
        assert_eq!(metrics.active_tasks, 2);
        assert_eq!(metrics.total_runs, 4);
        // 3 successes out of 4 runs = 75%
        assert!((metrics.overall_success_rate - 75.0).abs() < f64::EPSILON);
        // total spend = 1.00 + 2.00 + 0.50 + 3.00 = 6.50
        assert!((metrics.total_spend - 6.50).abs() < f64::EPSILON);

        assert_eq!(metrics.tasks.len(), 2);

        let task1_metrics = metrics
            .tasks
            .iter()
            .find(|m| m.task_id == "lc-task0001")
            .unwrap();
        assert_eq!(task1_metrics.total_runs, 3);
        assert_eq!(task1_metrics.success_count, 2);
        assert_eq!(task1_metrics.fail_count, 1);
        assert!((task1_metrics.total_cost - 3.50).abs() < f64::EPSILON);

        let task2_metrics = metrics
            .tasks
            .iter()
            .find(|m| m.task_id == "lc-task0002")
            .unwrap();
        assert_eq!(task2_metrics.total_runs, 1);
        assert_eq!(task2_metrics.success_count, 1);
        assert_eq!(task2_metrics.fail_count, 0);
        assert!((task2_metrics.total_cost - 3.00).abs() < f64::EPSILON);
    }

    #[test]
    fn task_metrics_single_task() {
        let (logger, _dir) = test_logger();

        logger
            .insert_log(&sample_log("lc-task0001", ExecStatus::Success, Some(1.00)))
            .unwrap();
        logger
            .insert_log(&sample_log("lc-task0001", ExecStatus::Failed, Some(0.50)))
            .unwrap();

        let metrics = logger.get_task_metrics("lc-task0001").unwrap();

        assert_eq!(metrics.task_id, "lc-task0001");
        assert_eq!(metrics.total_runs, 2);
        assert_eq!(metrics.success_count, 1);
        assert_eq!(metrics.fail_count, 1);
        assert!((metrics.total_cost - 1.50).abs() < f64::EPSILON);
        assert_eq!(metrics.total_tokens, 200);
        assert!(metrics.last_run.is_some());
    }

    #[test]
    fn task_metrics_no_logs() {
        let (logger, _dir) = test_logger();

        let metrics = logger.get_task_metrics("lc-nonexistent").unwrap();

        assert_eq!(metrics.total_runs, 0);
        assert_eq!(metrics.success_count, 0);
        assert_eq!(metrics.fail_count, 0);
        assert!((metrics.total_cost - 0.0).abs() < f64::EPSILON);
        assert!(metrics.last_run.is_none());
    }

    #[test]
    fn total_cost_since_correct_sum() {
        let (logger, _dir) = test_logger();

        // Insert a log with "now" timestamp.
        let mut log_recent = sample_log("lc-task0001", ExecStatus::Success, Some(2.00));
        log_recent.started_at = Utc::now();
        logger.insert_log(&log_recent).unwrap();

        // Insert a log with yesterday timestamp.
        let mut log_old = sample_log("lc-task0001", ExecStatus::Success, Some(5.00));
        log_old.started_at = Utc::now() - Duration::hours(25);
        logger.insert_log(&log_old).unwrap();

        // Cost since 2 hours ago should only include the recent log.
        let since = Utc::now() - Duration::hours(2);
        let cost = logger.total_cost_since("lc-task0001", since).unwrap();
        assert!((cost - 2.00).abs() < f64::EPSILON);

        // Cost since 2 days ago should include both.
        let since_old = Utc::now() - Duration::days(2);
        let cost_all = logger.total_cost_since("lc-task0001", since_old).unwrap();
        assert!((cost_all - 7.00).abs() < f64::EPSILON);
    }

    #[test]
    fn total_cost_since_no_data() {
        let (logger, _dir) = test_logger();

        let cost = logger
            .total_cost_since("lc-nonexistent", Utc::now() - Duration::days(1))
            .unwrap();
        assert!((cost - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn prune_logs_deletes_old_entries() {
        let (logger, _dir) = test_logger();

        // Insert a recent log.
        let recent = sample_log("lc-task0001", ExecStatus::Success, None);
        logger.insert_log(&recent).unwrap();

        // Insert an old log by directly inserting with an old timestamp.
        logger
            .conn
            .execute(
                "INSERT INTO execution_logs (
                    task_id, task_name, started_at, finished_at, duration_secs,
                    exit_code, status, stdout, stderr, summary
                ) VALUES (
                    'lc-task0001', 'Old Task',
                    datetime('now', '-100 days'), datetime('now', '-100 days'),
                    10, 0, 'success', '', '', 'old'
                )",
                [],
            )
            .unwrap();

        assert_eq!(logger.count_logs().unwrap(), 2);

        // Prune entries older than 90 days.
        let deleted = logger.prune_logs(90).unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(logger.count_logs().unwrap(), 1);

        // The remaining entry should be the recent one.
        let remaining = logger.query_logs(&LogQuery::default()).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].task_id, "lc-task0001");
        assert_ne!(remaining[0].summary, "old");
    }

    #[test]
    fn count_logs_returns_correct_count() {
        let (logger, _dir) = test_logger();

        assert_eq!(logger.count_logs().unwrap(), 0);

        logger
            .insert_log(&sample_log("lc-task0001", ExecStatus::Success, None))
            .unwrap();
        assert_eq!(logger.count_logs().unwrap(), 1);

        logger
            .insert_log(&sample_log("lc-task0002", ExecStatus::Failed, None))
            .unwrap();
        assert_eq!(logger.count_logs().unwrap(), 2);

        logger
            .insert_log(&sample_log("lc-task0001", ExecStatus::Timeout, None))
            .unwrap();
        assert_eq!(logger.count_logs().unwrap(), 3);
    }

    #[test]
    fn get_cost_trend_daily_aggregation() {
        let (logger, _dir) = test_logger();

        // Insert two logs for today.
        let mut log1 = sample_log("lc-task0001", ExecStatus::Success, Some(1.50));
        log1.started_at = Utc::now();
        logger.insert_log(&log1).unwrap();

        let mut log2 = sample_log("lc-task0001", ExecStatus::Success, Some(2.50));
        log2.started_at = Utc::now();
        logger.insert_log(&log2).unwrap();

        let trend = logger.get_cost_trend(7).unwrap();

        // Should have exactly 7 data points.
        assert_eq!(trend.len(), 7);

        // The last entry should be today's date with the sum.
        let today = Utc::now().date_naive().format("%Y-%m-%d").to_string();
        let today_entry = trend.iter().find(|d| d.date == today).unwrap();
        assert!((today_entry.total_cost - 4.00).abs() < f64::EPSILON);
        assert_eq!(today_entry.run_count, 2);

        // All other days should have zero cost and zero run count.
        for entry in &trend {
            if entry.date != today {
                assert!((entry.total_cost - 0.0).abs() < f64::EPSILON);
                assert_eq!(entry.run_count, 0);
            }
        }
    }

    #[test]
    fn get_cost_trend_backfills_missing_days() {
        let (logger, _dir) = test_logger();

        // No data at all.
        let trend = logger.get_cost_trend(7).unwrap();
        assert_eq!(trend.len(), 7);

        // All should be zero.
        for entry in &trend {
            assert!((entry.total_cost - 0.0).abs() < f64::EPSILON);
            assert_eq!(entry.run_count, 0);
        }

        // Verify dates are consecutive and in order.
        let today = Utc::now().date_naive();
        for (i, entry) in trend.iter().enumerate() {
            let expected_date = today - Duration::days(6 - i as i64);
            let expected_str = expected_date.format("%Y-%m-%d").to_string();
            assert_eq!(entry.date, expected_str, "date mismatch at index {i}");
        }
    }

    #[test]
    fn query_logs_ordered_by_started_at_desc() {
        let (logger, _dir) = test_logger();

        for i in 0..3 {
            let mut log = sample_log("lc-task0001", ExecStatus::Success, None);
            log.started_at = Utc::now() - Duration::seconds(i64::from(i) * 10);
            log.summary = format!("log-{i}");
            logger.insert_log(&log).unwrap();
        }

        let results = logger.query_logs(&LogQuery::default()).unwrap();

        // Most recent first.
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].summary, "log-0"); // most recent
        assert_eq!(results[2].summary, "log-2"); // oldest
    }

    #[test]
    fn schema_version_prevents_downgrade() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");

        // Create DB normally.
        let logger = Logger::new(&db_path).unwrap();

        // Manually bump the schema version to simulate a newer binary.
        logger
            .conn
            .execute("UPDATE schema_version SET version = 999", [])
            .unwrap();
        drop(logger);

        // Opening with current binary should fail.
        let result = Logger::new(&db_path);
        assert!(result.is_err());
        // Use the Debug format to capture the full error chain.
        let err = result.err().unwrap();
        let err_msg = format!("{err:?}");
        assert!(
            err_msg.contains("newer"),
            "error should mention newer version, got: {err_msg}"
        );
    }

    #[test]
    fn insert_log_with_null_cost_and_tokens() {
        let (logger, _dir) = test_logger();

        let mut log = sample_log("lc-task0001", ExecStatus::Skipped, None);
        log.tokens_used = None;
        log.cost_usd = None;

        let id = logger.insert_log(&log).unwrap();
        assert!(id > 0);

        let results = logger
            .query_logs(&LogQuery {
                task_id: Some("lc-task0001".to_string()),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].tokens_used.is_none());
        assert!(results[0].cost_usd.is_none());
    }

    #[test]
    fn dashboard_metrics_empty_db() {
        let (logger, _dir) = test_logger();

        let metrics = logger.get_dashboard_metrics(&[]).unwrap();

        assert_eq!(metrics.total_tasks, 0);
        assert_eq!(metrics.active_tasks, 0);
        assert_eq!(metrics.total_runs, 0);
        assert!((metrics.overall_success_rate - 0.0).abs() < f64::EPSILON);
        assert!((metrics.total_spend - 0.0).abs() < f64::EPSILON);
        assert!(metrics.tasks.is_empty());
        assert_eq!(metrics.cost_trend.len(), 7);
    }
}

<p align="center">
  <img src="loop-logo.png" alt="Intern" width="128" height="128">
</p>

<h1 align="center">Intern</h1>

<p align="center">Schedule Claude tasks to run autonomously on macOS with persistent scheduling and a native dashboard.</p>

Intern is a persistent, local-first task automation system built on Rust and native Swift. It integrates with launchd to run Claude commands on a schedule, track execution history, and monitor spending—all from a beautiful macOS dashboard or command-line interface. Tasks survive system reboots and run reliably with per-task budgets, timeouts, and safety limits.

<!-- Screenshot: Add app screenshot here -->

> **License**: Source Available. Free to use, not free to redistribute. See [LICENSE](LICENSE).

## Why Intern

Automate repetitive code tasks that Claude excels at: PR reviews, error log analysis, documentation generation, dependency audits, and more. Define a task once, set a schedule, and let Intern run it. No cron knowledge required. Full cost tracking and execution history built in.

## Key Features

- **Persistent scheduling**: Tasks survive system reboots via launchd user agents
- **Native macOS dashboard**: Real-time task management and execution logs in SwiftUI
- **Command-line interface**: Manage tasks from the terminal with intuitive commands
- **Cost tracking**: Monitor token usage and spending per task with spending caps
- **Flexible scheduling**: Cron expressions, fixed intervals, or calendar-based schedules
- **Safe execution**: Per-task budgets, timeouts, and maximum turn limits
- **Rich logging**: SQLite-backed execution history with stdout/stderr capture
- **Health monitoring**: Automatic daemon repair and recovery
- **JSON-RPC API**: Single Unix socket interface for CLI and macOS app
- **Environment variables**: Custom env config per task for API keys and secrets
- **Portable tasks**: Export and import task definitions for backup and sharing

## Requirements

- **macOS**: 13.0 (Ventura) or later
- **Rust**: 1.70+ (for building daemon, runner, and CLI)
- **Swift**: 5.9+ (for building the macOS app)
- **Xcode**: 14.3+ (optional; use for app bundling and debugging)

## Installation

### Quick Install (recommended)

Install the latest release with a single command:

```bash
curl -fsSL https://raw.githubusercontent.com/cNameHitch/intern/main/install.sh | bash
```

This installs the CLI binaries to `~/.local/bin/` and the macOS app to `~/Applications/`. The installer verifies SHA256 checksums before installing.

Options:

```bash
# Install a specific version
curl -fsSL https://raw.githubusercontent.com/cNameHitch/intern/main/install.sh | bash -s -- --version v0.1.0

# Install CLI only (skip the macOS app)
curl -fsSL https://raw.githubusercontent.com/cNameHitch/intern/main/install.sh | bash -s -- --cli-only
```

To uninstall:

```bash
curl -fsSL https://raw.githubusercontent.com/cNameHitch/intern/main/uninstall.sh | bash
```

The uninstall script stops the daemon, removes launchd plists, deletes binaries and the app, and prompts before removing data.

### Build from Source

```bash
git clone https://github.com/cNameHitch/intern.git
cd intern

# Build Rust daemon, runner, and CLI
cargo build --release

# Install binaries
mkdir -p ~/.local/bin
cp target/release/{intern,intern-runner,intern} ~/.local/bin/

# Build the macOS app
cd macos-app && ./build-app.sh

# Launch it
open "build/Intern.app"
```

### Start the Daemon

```bash
intern &
intern daemon status
```

On first start, the daemon creates `~/.intern/` with default `config.yaml` and initializes the SQLite database.

## How It Works

1. **Define a task**: Use the CLI or macOS app to create a task with a Claude command and schedule
2. **Schedule it**: Choose cron, fixed interval, or calendar-based scheduling
3. **Let it run**: launchd automatically executes the task on your schedule
4. **Monitor**: View execution logs, metrics, and costs in real-time from the dashboard or CLI

Each execution is isolated, tracked, and capped by a spending limit. The daemon ensures tasks stay healthy and restarts them if launchd unloads them.

## Quick Start

### Start the Daemon

```bash
intern &
intern daemon status
```

### Create Your First Task

```bash
intern add \
  --name "PR Review Sweep" \
  --command "claude -p 'Review all open PRs and comment on logic errors.'" \
  --schedule "0 9 * * 1-5"
```

Tasks are scheduled with standard 5-field cron syntax. The example above runs at 9 AM, Monday through Friday.

### List Tasks

```bash
intern list
```

Output:
```
ID             NAME               SCHEDULE            STATUS   RUNS   COST
lc-a1b2c3d4   PR Review Sweep    Daily at 09:00      active    0      -
```

### View Execution Logs

```bash
intern logs lc-a1b2c3d4 --limit 10
intern logs lc-a1b2c3d4 --follow
```

### Launch the Dashboard

```bash
open "~/Applications/Intern.app"
```

The dashboard displays active tasks, real-time metrics, execution history, and a task editor with live validation.

### Trigger an Immediate Run

```bash
intern run lc-a1b2c3d4
```

### Pause and Resume

```bash
intern pause lc-a1b2c3d4
intern resume lc-a1b2c3d4
```

## Architecture

Intern is a 7-crate Cargo workspace plus a native SwiftUI macOS app:

| Component | Purpose |
|-----------|---------|
| `intern-core` | Domain types, errors, IPC messages, validation |
| `intern-config` | YAML config read/write for global and per-task settings |
| `intern-scheduler` | launchd plist generation and lifecycle management |
| `intern-runner` | Standalone binary invoked by launchd to execute tasks |
| `intern-logger` | SQLite database for execution logs (WAL mode) |
| `intern-daemon` | Unix socket JSON-RPC server, health monitoring, lifecycle |
| `intern-cli` | Command-line interface |
| `macos-app/` | Native SwiftUI dashboard application |

### Communication Model

The **daemon** is the sole writer to all persistent data. Both the CLI and macOS app communicate exclusively via **JSON-RPC 2.0 over a Unix domain socket** at `~/.intern/daemon.sock`. This single-writer architecture eliminates race conditions and concurrency issues.

```
launchd (macOS scheduler)
├─ Loads plist → spawns intern-runner
└─ Captures stdout/stderr

Intern Daemon
├─ Listens: Unix socket
├─ Writes: YAML configs, SQLite logs
├─ Health checks: Every 60 seconds
└─ Broadcasts: Real-time events to CLI/Swift app

CLI (intern)              Swift App
└─ JSON-RPC over socket ← (same protocol)
```

### Data Storage

All data lives in `~/.intern/`:

| Location | Purpose |
|----------|---------|
| `config.yaml` | Global settings (claude binary, budgets, timeouts) |
| `tasks/*.yaml` | One YAML file per task |
| `plists/*.plist` | Generated launchd plist files |
| `output/*.log` | Task stdout/stderr files |
| `logs.db` | SQLite execution history (WAL mode) |
| `daemon.pid` | Daemon process ID |
| `daemon.sock` | Unix socket for IPC |

Symlinks in `~/Library/LaunchAgents/` point to the plists. All YAML writes are atomic (temp file + fsync + rename). SQLite uses WAL mode for concurrent readers.

## CLI Reference

### Daemon

| Command | Purpose |
|---------|---------|
| `intern daemon start` | Start the daemon in background |
| `intern daemon stop` | Stop the daemon |
| `intern daemon status` | Show daemon PID and uptime |

### Task Management

| Command | Purpose |
|---------|---------|
| `intern list` | List all tasks |
| `intern add [FLAGS]` | Create a new task |
| `intern get <id>` | Show task details |
| `intern edit <id> [FLAGS]` | Edit a task (schedule, budget, command) |
| `intern rm <id> -y` | Delete a task |
| `intern pause <id>` | Pause execution (task remains in system) |
| `intern resume <id>` | Resume execution |
| `intern run <id>` | Trigger immediate execution |
| `intern stop <id>` | Kill a running task |

### Logs and Metrics

| Command | Purpose |
|---------|---------|
| `intern logs [id] [FLAGS]` | Query execution logs |
| `intern logs [id] --limit 10` | Show most recent 10 logs |
| `intern logs [id] --status success` | Filter by status: success, failed, timeout, killed, skipped |
| `intern logs [id] --search "error"` | Search stdout/stderr/summary |
| `intern logs [id] --follow` | Stream new logs in real-time |
| `intern status` | Show global summary: total tasks, runs, spend |
| `intern metrics <id>` | Per-task metrics: runs, success rate, total cost |

### Templates and Portability

| Command | Purpose |
|---------|---------|
| `intern templates` | List built-in task templates |
| `intern export <id> [--format yaml\|json]` | Export task definition |
| `intern import <file.yaml>` | Import task definition |

### Configuration

| Command | Purpose |
|---------|---------|
| `intern config get` | Show global config |
| `intern config set --key value` | Update a config key |

### Global Flags

```
-h, --help              Show command help
-v, --verbose           Verbose output
--socket PATH           Override daemon socket path
```

## Configuration

### Global Config

`~/.intern/config.yaml`:

```yaml
version: 1
claude_binary: "claude"              # Path or name of Claude CLI binary
default_budget: 5.0                  # Default max spend per run (USD)
default_timeout: 600                 # Default timeout per task (seconds)
default_max_turns: 50                # Default max turns for Claude
log_retention_days: 90               # Auto-delete logs older than this
notifications_enabled: true          # Reserved for future use
```

### Task Configuration

Each task is a YAML file at `~/.intern/tasks/{task-id}.yaml`:

```yaml
id: lc-a1b2c3d4
name: "Review Open PRs"
command: >-
  claude -p 'Review all open PRs in this repo.
  Check for logic errors, missing tests, and style violations.
  Post a comment with your findings.'
skill: null                          # Optional: skill mode for claude
schedule:
  type: cron
  expression: "0 9 * * 1-5"          # 9 AM Mon-Fri
schedule_human: "Daily at 09:00 Mon-Fri"
working_dir: "/Users/alice/projects/my-repo"
env_vars:
  GITHUB_TOKEN: "ghp_..."            # Custom env variables (secrets OK)
max_budget_per_run: 10.0             # Max spend per execution (USD)
max_turns: 50                        # Max turns for Claude
timeout_secs: 600                    # Kill task after this many seconds
status: active                       # active, paused, error, disabled
tags:
  - "ci-automation"
  - "code-review"
created_at: "2026-03-19T14:22:30Z"
updated_at: "2026-03-19T14:22:30Z"
```

### Schedule Types

#### Cron (recommended)

Standard 5-field cron syntax, automatically converted to launchd schedule:

```yaml
schedule:
  type: cron
  expression: "*/15 * * * *"         # Every 15 minutes
  # or
  expression: "0 9 * * 1-5"          # 9 AM weekdays
  # or
  expression: "0 0 1 * *"            # 1st of each month
```

#### Interval

Fixed interval in seconds:

```yaml
schedule:
  type: interval
  seconds: 3600                      # Every hour
```

#### Calendar

Calendar-based (maps to launchd StartCalendarInterval):

```yaml
schedule:
  type: calendar
  hour: 9
  minute: 0
  weekday: 1                         # 0=Sunday, 1=Monday, ..., 6=Saturday
```

## Execution Model

When a task's schedule triggers (or `intern run` is called):

1. **launchd** spawns `intern-runner --task-id <id>`
2. **intern-runner**:
   - Loads task YAML and SQLite logger
   - Checks daily budget (skips if capped at `max_budget_per_run * 20`)
   - Builds the command (wraps plain text in `claude -p '...'` if needed)
   - Spawns claude subprocess with task env vars
   - Enforces timeout; kills on overflow
   - Captures stdout and stderr
   - Parses token/cost from JSON output (best-effort)
   - Writes ExecutionLog to SQLite
   - Updates task status on failure
3. **Daemon** (via log monitoring):
   - Broadcasts events to subscribed CLI/Swift app
   - Updates metrics
   - Health checks every 60s to keep launchd jobs loaded

### Safety Limits

- **Per-run budget**: `max_budget_per_run` (default 5 USD)
- **Daily cap**: Automatically skip if daily spend >= `max_budget_per_run * 20`
- **Timeout**: `timeout_secs` (default 600s). Exceeds → task killed, marked as Timeout
- **Max turns**: `max_turns` passed to Claude CLI (default 50)

## Security Considerations

- **Local-only**: No network exposure. All communication is Unix domain socket on localhost
- **API keys in env**: Store secrets in task `env_vars`. They are persisted in YAML but NOT transmitted over network
- **File permissions**: `~/.intern/` and task YAML files are owned by the current user. Ensure proper umask
- **Socket security**: `daemon.sock` is created with 0600 permissions (user-only access)
- **No elevated privileges**: All daemon operations run as the current user; no sudo required

## Development

### Building

```bash
# Build all Rust crates (release optimized)
cargo build --release

# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p intern-core

# Build Swift app
cd macos-app && swift build

# Create production app bundle
cd macos-app && swift build -c release && ./build-app.sh
```

### Project Structure

```
crates/
├── intern-core/              # Domain types, errors, IPC messages
├── intern-config/            # YAML config read/write
├── intern-scheduler/         # launchd plist generation and control
├── intern-runner/            # Task executor binary (spawned by launchd)
├── intern-logger/            # SQLite execution logs (WAL mode)
├── intern-daemon/            # Unix socket JSON-RPC server
└── intern-cli/               # Command-line interface

macos-app/
├── Intern/                   # SwiftUI source code
│   ├── Models/               # Task, ExecutionLog, DashboardMetrics
│   ├── Services/             # DaemonClient (JSON-RPC), DaemonMonitor
│   ├── ViewModels/           # MVVM logic
│   └── Views/                # SwiftUI components
├── InternTests/              # Unit tests
└── Package.swift             # Swift package manifest
```

### Key Design Decisions

1. **Single daemon writer**: All writes go through daemon; no dual-writer race conditions
2. **JSON-RPC over Unix socket**: Simple, language-agnostic IPC
3. **launchd integration**: Tasks persist across reboots; no polling needed
4. **Atomic YAML writes**: Temp file + fsync + rename prevents corruption
5. **SQLite WAL mode**: Concurrent readers while daemon writes
6. **intern-runner as separate binary**: Process isolation; task crashes don't affect daemon

### Testing

```bash
cargo test --workspace

# Run with output
cargo test --workspace -- --nocapture

# Run a specific test
cargo test -p intern-scheduler test_cron_conversion
```

Some integration tests require launchd access and are skipped in CI with `#[cfg(not(ci))]`.

## Troubleshooting

### Daemon not responding

```bash
intern daemon status

# Start it
intern &

# Check if it's running
ps aux | grep intern
```

### Stale socket connection

Remove stale socket and restart:

```bash
rm -f ~/.intern/daemon.sock
intern &
```

### Task not executing

1. Check task status:
   ```bash
   intern get <id>
   ```

2. Verify launchd job is loaded:
   ```bash
   launchctl print gui/$(id -u)/com.intern.task.<id>
   ```

3. Check logs:
   ```bash
   intern logs <id> --limit 5
   ```

### Budget exceeded

Tasks are skipped if daily spend >= `max_budget_per_run * 20`. Check:

```bash
intern metrics <id>
intern logs <id> --status skipped
```

### launchd won't load plist

Ensure:
1. Plist path is readable: `ls -la ~/Library/LaunchAgents/com.intern.task.<id>.plist`
2. intern-runner binary exists: `which intern-runner && file $(which intern-runner)`
3. Working directory exists: check `WorkingDirectory` in plist

### Logs not appearing

Check SQLite:

```bash
sqlite3 ~/.intern/logs.db "SELECT COUNT(*) FROM execution_logs;"
```

If table doesn't exist, restart the daemon:

```bash
intern daemon stop
sleep 1
intern &
```

## JSON-RPC API

The daemon serves JSON-RPC 2.0 over the Unix socket. Both CLI and Swift app use this protocol.

### Example: Create a Task

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "task.create",
  "params": {
    "name": "Daily Standup",
    "command": "claude -p 'Write a standup summary.'",
    "schedule": {"type": "cron", "expression": "0 9 * * 1-5"},
    "working_dir": "/home/user/projects/myrepo",
    "max_budget_per_run": 2.0,
    "timeout_secs": 300
  },
  "id": 1
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "id": "lc-a1b2c3d4",
    "name": "Daily Standup",
    "schedule": {"type": "cron", "expression": "0 9 * * 1-5"},
    "status": "active",
    "created_at": "2026-03-19T14:22:30Z"
  },
  "id": 1
}
```

### Common Methods

```
task.list              → Vec<Task>
task.get               → Task
task.create            → Task
task.update            → Task
task.delete            → null
task.pause             → Task
task.resume            → Task
task.run_now           → null
task.dry_run           → DryRunResult
task.export            → TaskExport
task.import            → Task

logs.query             → Vec<ExecutionLog>
logs.prune             → u64 (deleted count)

metrics.dashboard      → DashboardMetrics
metrics.task           → TaskMetrics
metrics.cost_trend     → Vec<DailyCost>

templates.list         → Vec<TaskTemplate>

config.get             → GlobalConfig
config.update          → GlobalConfig

daemon.status          → {pid, uptime, version, connected_clients}

events.subscribe       → (streaming newline-delimited JSON)
```

See `specs.md` for full method signatures.

## CI/CD

Intern uses GitHub Actions for continuous integration and release automation.

### CI Pipeline

Runs on every push to `main` and every pull request. All three jobs run in parallel:

| Job | What it does |
|-----|-------------|
| **Test** | `cargo test --workspace` on macOS Apple Silicon |
| **Lint** | `cargo fmt --check` and `cargo clippy -- -D warnings` |
| **Swift Build** | `cd macos-app && swift build` to verify compilation |

All three must pass before a PR can merge. Cargo and Swift build caches keep CI runs under 5 minutes.

### Release Pipeline

Triggered by pushing a version tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release pipeline:

1. Runs the full test suite (gate)
2. Builds optimized Rust binaries (`cargo build --release`)
3. Builds the macOS `.app` bundle (`build-app.sh`)
4. Packages artifacts:
   - `intern-{version}-darwin-arm64.tar.gz` -- CLI binaries
   - `Intern-{version}.zip` -- macOS app bundle
   - `CHECKSUMS.txt` -- SHA256 checksums
5. Creates a GitHub Release with all artifacts and auto-generated release notes

Workflow files: [`.github/workflows/ci.yml`](.github/workflows/ci.yml), [`.github/workflows/release.yml`](.github/workflows/release.yml)

## License

Intern is **source available** under the [Intern Source Available License](LICENSE).

You are free to view, download, build, and use the software for personal or internal business purposes. Redistribution, resale, hosting as a service, and creation of derivative works for distribution are **not permitted**. See [LICENSE](LICENSE) for full terms.

This is not an open-source license. All rights are reserved by the copyright holders.

## Contributing

Contributions are welcome. Please follow this workflow:

### Branch Naming

Use one of these prefixes for your branches:

- `feature/` — New features (e.g., `feature/task-templates`)
- `fix/` — Bug fixes (e.g., `fix/daemon-socket-leak`)
- `release/` — Release branches (e.g., `release/v0.2.0`)

### Commit Messages

Use clear, concise commits with this format:

```
type: Brief summary (50 chars or less)

Optional detailed explanation if needed.
Keep lines under 72 characters.
```

Types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`

Example:
```
feat: Add task timeout validation

Validate timeout values when creating tasks to prevent
negative or excessively long durations.
```

### Pull Request Process

1. Fork the repository and create a feature branch
2. Make your changes and add tests for new functionality
3. Run `cargo test --workspace` — all tests must pass
4. Build the Swift app: `cd macos-app && swift build`
5. Verify the daemon starts and communicates correctly
6. Open a pull request with a clear description
7. Respond to review feedback

For major changes, open an issue first to discuss the approach.

### Release Process

Releases are tagged from the `main` branch using semantic versioning.

To release:

1. Update version numbers in `Cargo.toml` files
2. Create a release commit
3. Tag the commit: `git tag v0.x.x`
4. Push the tag: `git push origin v0.x.x`
5. GitHub Actions will build and publish artifacts

Use [Semantic Versioning](https://semver.org/): `v<major>.<minor>.<patch>`

Breaking changes increment the major version.

## Acknowledgments

Built with Rust, Swift/SwiftUI, SQLite, and launchd. Inspired by cron and modern task automation tools.

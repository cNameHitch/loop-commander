# Intern

System-level macOS scheduler for Claude Code tasks with a native SwiftUI dashboard.

## Architecture

7-crate Cargo workspace + native Swift macOS app:
- `intern-core`: Domain types, errors, IPC messages, validation
- `intern-config`: YAML config read/write for global config + per-task files
- `intern-scheduler`: launchd plist generation, launchctl bootstrap/bootout
- `intern-runner`: Standalone binary invoked by launchd to execute claude commands
- `intern-logger`: SQLite persistence for execution logs
- `intern-daemon`: Long-running Unix socket server, health checks, task lifecycle
- `intern-cli`: Command line interface communicating with daemon via JSON-RPC
- `macos-app/`: Native SwiftUI app communicating with daemon via JSON-RPC

All data lives in ~/.intern/

## Key constraints

- macOS only (launchd, ~/Library/LaunchAgents)
- Tasks persist across reboots (launchd user agents)
- SQLite in WAL mode (concurrent readers)
- JSON-RPC 2.0 over Unix domain socket for IPC
- Daemon is the SOLE API server — both CLI and Swift app communicate through it
- intern-runner is a separate binary for process isolation
- No FFI between Swift and Rust — pure JSON-RPC over socket
- All YAML writes are atomic (temp file + fsync + rename)
- Socket at ~/.intern/daemon.sock (not /tmp/)
- launchctl bootstrap/bootout (modern API, not deprecated load/unload)

## Testing

Run `cargo test --workspace` from root. Every crate has unit tests.
Run `swift build` from `macos-app/` for the Swift app.
Integration tests may need launchd access (skip in CI with `#[cfg(not(ci))]`).

## File locations

- ~/.intern/config.yaml — global settings
- ~/.intern/tasks/*.yaml — one file per task
- ~/.intern/plists/*.plist — generated launchd plists
- ~/.intern/output/*.log — stdout/stderr from runs
- ~/.intern/logs.db — SQLite execution log
- ~/.intern/daemon.pid — daemon PID
- ~/.intern/daemon.sock — daemon Unix socket
- ~/Library/LaunchAgents/com.intern.task.*.plist — symlinks

## Build

```bash
# Rust
cargo build --release
# Binaries: target/release/intern (daemon), target/release/intern-runner, target/release/intern (CLI)

# Swift (compile only)
cd macos-app && swift build -c debug
```

## Running the macOS App

The Swift app requires a proper `.app` bundle to run (UNUserNotificationCenter crashes without one).
Running the bare binary from `.build/debug/Intern` will fail. Always use the bundle workflow:

```bash
cd macos-app

# 1. Build
swift build -c debug

# 2. Create .app bundle (only needed once, or after clean)
mkdir -p .build/debug/Intern.app/Contents/MacOS

# 3. Copy Info.plist and icon resources from the canonical build
cp "build/Intern.app/Contents/Info.plist" .build/debug/Intern.app/Contents/Info.plist
cp -R "build/Intern.app/Contents/Resources" .build/debug/Intern.app/Contents/

# 4. Copy the fresh binary into the bundle
cp .build/debug/Intern .build/debug/Intern.app/Contents/MacOS/Intern

# 5. Launch
open .build/debug/Intern.app
```

**On rebuild**, only steps 1, 4, and 5 are needed (the bundle structure persists).

**If the dock icon shows as generic**, flush the Launch Services cache:
```bash
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister \
  -kill -r -domain local -domain system -domain user
touch .build/debug/Intern.app
```

**Kill and relaunch** (full cycle):
```bash
pkill -x Intern; sleep 1
swift build -c debug
cp .build/debug/Intern .build/debug/Intern.app/Contents/MacOS/Intern
open .build/debug/Intern.app
```

**Important**: The canonical Info.plist and AppIcon.icns live in `macos-app/build/Intern.app/Contents/`.
This directory is checked in and must not be deleted. No Xcode installation is required — only Swift toolchain via Command Line Tools.

## Running the Daemon

The macOS app connects to the daemon via Unix socket. Start it before or after the app:

```bash
# Start daemon (from repo root)
target/release/intern --foreground &

# Or build and start
cargo build --release && target/release/intern --foreground &
```

If the daemon was previously killed, clean stale files first:
```bash
rm -f ~/.intern/daemon.sock ~/.intern/daemon.pid
```

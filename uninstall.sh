#!/usr/bin/env bash
# Intern — uninstall.sh
# https://github.com/cNameHitch/intern
#
# Removes Intern CLI binaries, the macOS app bundle, and optionally
# the ~/.intern/ data directory. Also offers to clean up legacy
# ~/.loop-commander/ data left from the Loop Commander era.
#
# Usage:
#   bash uninstall.sh
#   bash uninstall.sh --data          # also remove data without prompting
#   bash uninstall.sh --keep-data     # skip data removal prompt entirely

set -euo pipefail

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/Applications"
DATA_DIR="${HOME}/.intern"
DATA_DIR_LEGACY="${HOME}/.loop-commander"
LAUNCHAGENTS_DIR="${HOME}/Library/LaunchAgents"

BINARIES=("intern" "intern-runner")

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------

REMOVE_DATA=""   # "" = prompt, "yes" = remove without prompt, "no" = skip

while [[ $# -gt 0 ]]; do
    case "$1" in
        --data)
            REMOVE_DATA="yes"
            shift
            ;;
        --keep-data)
            REMOVE_DATA="no"
            shift
            ;;
        -h|--help)
            cat <<EOF
Intern uninstaller

Usage:
  uninstall.sh [OPTIONS]

Options:
  --data        Remove the ~/.intern/ data directory without prompting.
  --keep-data   Skip the data directory removal prompt entirely.
  -h, --help    Show this help and exit.
EOF
            exit 0
            ;;
        *)
            echo "error: unknown option: $1" >&2
            echo "Run with --help for usage." >&2
            exit 1
            ;;
    esac
done

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

info()    { printf "[uninstall] %s\n" "$*"; }
success() { printf "[uninstall] %s\n" "$*"; }
warn()    { printf "[uninstall] warning: %s\n" "$*" >&2; }

removed=()

mark_removed() {
    removed+=("$1")
}

# ---------------------------------------------------------------------------
# Platform check
# ---------------------------------------------------------------------------

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "error: this uninstaller targets macOS only." >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# Stop the daemon if it is running
# ---------------------------------------------------------------------------

DAEMON_PID_FILE="${DATA_DIR}/daemon.pid"

if [[ -f "${DAEMON_PID_FILE}" ]]; then
    DAEMON_PID="$(cat "${DAEMON_PID_FILE}" 2>/dev/null || true)"
    if [[ -n "${DAEMON_PID}" ]] && kill -0 "${DAEMON_PID}" 2>/dev/null; then
        info "Stopping running daemon (PID ${DAEMON_PID})..."
        kill "${DAEMON_PID}" 2>/dev/null || true
        # Give the daemon a moment to shut down gracefully before proceeding.
        local_timeout=5
        while kill -0 "${DAEMON_PID}" 2>/dev/null && [[ ${local_timeout} -gt 0 ]]; do
            sleep 1
            (( local_timeout-- )) || true
        done
        if kill -0 "${DAEMON_PID}" 2>/dev/null; then
            warn "Daemon did not exit within 5 seconds; sending SIGKILL..."
            kill -9 "${DAEMON_PID}" 2>/dev/null || true
        else
            info "  Daemon stopped."
        fi
    fi
fi

# ---------------------------------------------------------------------------
# Remove launchd plists (symlinks in ~/Library/LaunchAgents)
# ---------------------------------------------------------------------------

PLIST_PATTERN="${LAUNCHAGENTS_DIR}/com.intern.task.*.plist"
# Use a glob expansion; if no files match, the loop body is skipped.
shopt -s nullglob
plist_files=( ${PLIST_PATTERN} )
shopt -u nullglob

if [[ ${#plist_files[@]} -gt 0 ]]; then
    info "Removing ${#plist_files[@]} launchd plist(s) from ${LAUNCHAGENTS_DIR}..."
    for plist in "${plist_files[@]}"; do
        # Attempt to unload the agent before removing the file so launchd does
        # not keep a stale entry registered.  Use bootout (modern API).
        launchctl bootout "gui/$(id -u)" "${plist}" 2>/dev/null || true
        rm -f "${plist}"
        info "  Removed ${plist}"
        mark_removed "${plist}"
    done
fi

# ---------------------------------------------------------------------------
# Remove CLI binaries
# ---------------------------------------------------------------------------

info "Removing CLI binaries from ${BIN_DIR}..."
any_binary_removed=false

for binary in "${BINARIES[@]}"; do
    target="${BIN_DIR}/${binary}"
    if [[ -f "${target}" ]]; then
        rm -f "${target}"
        info "  Removed ${target}"
        mark_removed "${target}"
        any_binary_removed=true
    else
        info "  Not found (skipping): ${target}"
    fi
done

if [[ "${any_binary_removed}" == "false" ]]; then
    info "  No binaries found in ${BIN_DIR}."
fi

# ---------------------------------------------------------------------------
# Remove macOS app bundle
# ---------------------------------------------------------------------------

APP_BUNDLE="${APP_DIR}/Intern.app"

if [[ -d "${APP_BUNDLE}" ]]; then
    info "Removing ${APP_BUNDLE}..."
    rm -rf "${APP_BUNDLE}"
    mark_removed "${APP_BUNDLE}"
    info "  Removed ${APP_BUNDLE}"
else
    info "App bundle not found (skipping): ${APP_BUNDLE}"
fi

# Also check /Applications in case the user moved it there.
SYSTEM_APP="/Applications/Intern.app"
if [[ -d "${SYSTEM_APP}" ]]; then
    info "Found app bundle at ${SYSTEM_APP}."
    info "  To remove it, run: rm -rf \"${SYSTEM_APP}\""
    info "  (Skipping system /Applications — manual removal required.)"
fi

# ---------------------------------------------------------------------------
# Remove data directory
# ---------------------------------------------------------------------------

if [[ -d "${DATA_DIR}" ]]; then
    if [[ "${REMOVE_DATA}" == "yes" ]]; then
        do_remove_data=true
    elif [[ "${REMOVE_DATA}" == "no" ]]; then
        do_remove_data=false
        info "Keeping data directory (--keep-data specified): ${DATA_DIR}"
    else
        # Interactive prompt.
        printf "\n"
        printf "Data directory found at: %s\n" "${DATA_DIR}"
        printf "This contains your task definitions, execution logs, and configuration.\n"
        printf "Remove it? This cannot be undone. [y/N] "
        read -r response
        case "${response}" in
            [yY]|[yY][eE][sS])
                do_remove_data=true
                ;;
            *)
                do_remove_data=false
                info "Keeping data directory: ${DATA_DIR}"
                ;;
        esac
    fi

    if [[ "${do_remove_data}" == "true" ]]; then
        info "Removing data directory: ${DATA_DIR}..."
        rm -rf "${DATA_DIR}"
        mark_removed "${DATA_DIR}"
        info "  Removed ${DATA_DIR}"
    fi
else
    info "Data directory not found (skipping): ${DATA_DIR}"
fi

# ---------------------------------------------------------------------------
# Legacy data cleanup (~/.loop-commander)
# ---------------------------------------------------------------------------

if [[ -d "${DATA_DIR_LEGACY}" ]]; then
    printf "\n"
    printf "Legacy data directory found at: %s\n" "${DATA_DIR_LEGACY}"
    printf "This is left over from the Loop Commander era and is no longer used.\n"

    if [[ "${REMOVE_DATA}" == "yes" ]]; then
        do_remove_legacy=true
    elif [[ "${REMOVE_DATA}" == "no" ]]; then
        do_remove_legacy=false
        info "Keeping legacy data directory (--keep-data specified): ${DATA_DIR_LEGACY}"
    else
        printf "Remove it? This cannot be undone. [y/N] "
        read -r response
        case "${response}" in
            [yY]|[yY][eE][sS])
                do_remove_legacy=true
                ;;
            *)
                do_remove_legacy=false
                info "Keeping legacy data directory: ${DATA_DIR_LEGACY}"
                ;;
        esac
    fi

    if [[ "${do_remove_legacy}" == "true" ]]; then
        info "Removing legacy data directory: ${DATA_DIR_LEGACY}..."
        rm -rf "${DATA_DIR_LEGACY}"
        mark_removed "${DATA_DIR_LEGACY}"
        info "  Removed ${DATA_DIR_LEGACY}"
    fi
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

printf "\n"

if [[ ${#removed[@]} -gt 0 ]]; then
    success "Uninstall complete. Removed:"
    for item in "${removed[@]}"; do
        printf "  - %s\n" "${item}"
    done
    printf "\n"
else
    info "Nothing was removed — no Intern files were found."
    printf "\n"
fi

# Advise the user to remove the PATH entry they may have added.
if grep -qr "\.local/bin" "${HOME}/.zshrc" "${HOME}/.bash_profile" "${HOME}/.profile" 2>/dev/null; then
    warn "You may also want to remove the PATH entry for ~/.local/bin from your shell config,"
    warn "if it was added solely for Intern."
fi

printf "Thank you for using Intern.\n"
printf "https://github.com/cNameHitch/intern\n"
printf "\n"

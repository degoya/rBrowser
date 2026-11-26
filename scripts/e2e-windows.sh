#!/usr/bin/env bash
#
# The end-to-end criteria on Windows (RB-099-02): K1-K12 of docs/roadmap.md, the scroll sync
# fixture and the milestone smokes, against the Windows package - the version's ZIP of
# scripts/package-windows.sh, or an app folder - staged in a folder of its own
# (%LOCALAPPDATA%\rbrowser-e2e) and run in the portable mode with known settings, so a run never
# touches the person's rBrowser. The cases are TypeScript in tests/e2e/ (tests/e2e/README.md),
# run by tests/e2e/run.ts with the Node of .nvmrc; the report is artifacts/e2e/report.json and
# report.md, each criterion's log under artifacts/e2e/logs/.
#
# From WSL (the default) the app runs on Windows through the interop and the fixture server and
# the projects stay in WSL (K6 watches a WSL project, K7 speaks MCP across the interop). With
# --native the whole run is on Windows (Git Bash, the windows-2025 runner of e2e.yml): no WSL, no
# run-windows.sh.
#
# Usage:
#   scripts/e2e-windows.sh [--package <zip> | --app <dir>] [--native] [--only K4,K10]
#                          [--report <path>] [--keep] [--allow-missing firefox[,webkit]]
#   scripts/e2e-windows.sh --list [--only ...] [--native]   # the plan; nothing starts
#
#   --package <zip>   the package to test (default artifacts/windows/rbrowser-<version>-windows-x64.zip)
#   --app <dir>       an app folder instead (rbrowser.exe, rbrowser.com and the rest of a package)
#   --only <ids>      only these criteria (K1-K12, SYNC, M7, M9-M12), comma-separated
#   --report <path>   the report JSON (default artifacts/e2e/report.json; report.md beside it)
#   --keep            leave the last app running for a look
#   --allow-missing   an engine that is not installed skips its criterion instead of failing it
#
# Builds nothing and takes no lock: scripts/package-windows.sh makes the package. Exit: 0 every
# criterion passed, 1 one failed, 2 the run could not start.
# RB_E2E_STAGE_DIR (the staging folder) and RB_E2E_RUNNER (instead of `node tests/e2e/run.ts`)
# are for scripts/tests/e2e-windows.sh.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/npm.sh
source "$ROOT/scripts/lib/npm.sh"
# shellcheck source=lib/package.sh
source "$ROOT/scripts/lib/package.sh"
# shellcheck source=lib/tool-versions.sh
source "$ROOT/scripts/lib/tool-versions.sh"
# shellcheck source=lib/workspace-version.sh
source "$ROOT/scripts/lib/workspace-version.sh"

usage() {
    sed -n '/^# Usage:/,/^# Builds nothing/p' "$0" | sed 's/^# \{0,1\}//' >&2
    exit 2
}

needs_value() {
    [[ -n "${2:-}" && "${2:0:2}" != "--" ]] || { echo "e2e-windows.sh: $1 needs a value" >&2; exit 2; }
}

native=0
list=0
package=""
app=""
runner_args=()
# A path the caller gave, read against the caller's folder (the script works from $ROOT).
absolute() {
    case "$1" in
        /*) printf '%s\n' "$1" ;;
        *) printf '%s\n' "$PWD/$1" ;;
    esac
}
while [[ $# -gt 0 ]]; do
    case "$1" in
        --native) native=1; runner_args+=(--native); shift ;;
        --list) list=1; shift ;;
        --keep) runner_args+=(--keep); shift ;;
        --only|--allow-missing)
            needs_value "$1" "${2:-}"
            runner_args+=("$1" "$2"); shift 2 ;;
        --report) needs_value "$1" "${2:-}"; runner_args+=("$1" "$(absolute "$2")"); shift 2 ;;
        --package) needs_value "$1" "${2:-}"; package="$(absolute "$2")"; shift 2 ;;
        --app) needs_value "$1" "${2:-}"; app="$(absolute "$2")"; shift 2 ;;
        -h|--help) usage ;;
        *) echo "e2e-windows.sh: unknown argument $1" >&2; usage ;;
    esac
done
if [[ -n "$package" && -n "$app" ]]; then
    echo "e2e-windows.sh: --package and --app exclude each other" >&2
    exit 2
fi
cd "$ROOT"

runner() {
    if [[ -n "${RB_E2E_RUNNER:-}" ]]; then
        # shellcheck disable=SC2086  # a command line of the test, split on purpose
        ${RB_E2E_RUNNER} "$@"
    else
        node tests/e2e/run.ts "$@"
    fi
}

if [[ -z "${RB_E2E_RUNNER:-}" ]]; then
    rb_use_pinned_node "$ROOT" || exit 2
fi
if [[ "$list" == 1 ]]; then
    set +e
    runner --list "${runner_args[@]}"
    exit $?
fi

# The staging folder on Windows, as this shell sees it.
stage_dir() {
    if [[ -n "${RB_E2E_STAGE_DIR:-}" ]]; then
        echo "$RB_E2E_STAGE_DIR"
        return
    fi
    local local_app
    if [[ "$native" == 1 ]]; then
        [[ -n "${LOCALAPPDATA:-}" ]] || { echo "!! LOCALAPPDATA is not set: --native runs on Windows" >&2; return 1; }
        local_app="$(cygpath -u "$LOCALAPPDATA")"
    else
        local cmd
        cmd="$(command -v cmd.exe || echo /mnt/c/Windows/System32/cmd.exe)"
        if ! command -v wslpath > /dev/null 2>&1 || [[ ! -e "$cmd" ]]; then
            echo "!! wslpath or cmd.exe not found: this needs WSL (or --native on Windows)" >&2
            return 1
        fi
        # From /mnt/c, so cmd.exe does not warn about a UNC working folder.
        local_app="$(cd /mnt/c && "$cmd" /d /c 'echo %LOCALAPPDATA%' 2> /dev/null | tr -d '\r')"
        [[ -n "$local_app" && "$local_app" != "%LOCALAPPDATA%" ]] \
            || { echo "!! could not read %LOCALAPPDATA% through cmd.exe" >&2; return 1; }
        local_app="$(wslpath -u "$local_app")"
    fi
    echo "$local_app/rbrowser-e2e"
}

dest="$(stage_dir)" || exit 2
if [[ "$(basename "$dest")" != rbrowser-e2e* ]]; then
    echo "!! the staging folder must be named rbrowser-e2e*: $dest" >&2
    exit 2
fi
if ! mkdir -p "$dest" 2> /dev/null || [[ ! -w "$dest" ]]; then
    echo "!! $dest is not writable (C: mounted read-only in WSL? then run --native on the Windows host)" >&2
    exit 2
fi

if [[ -n "$app" ]]; then
    [[ -f "$app/rbrowser.exe" && -f "$app/rbrowser.com" ]] \
        || { echo "!! $app holds no rbrowser.exe and rbrowser.com" >&2; exit 2; }
    folder="$dest/app"
    echo "==> copying $app to $folder"
    rm -rf "$folder"
    mkdir -p "$folder"
    cp -r "$app/." "$folder/"
else
    [[ -n "$package" ]] || package="$ROOT/artifacts/windows/rbrowser-$(rb_workspace_version < Cargo.toml)-windows-x64.zip"
    if [[ ! -f "$package" ]]; then
        echo "!! no package $package: scripts/package-windows.sh makes it (or name one with --package, an app folder with --app)" >&2
        exit 2
    fi
    echo "==> unpacking $package into $dest"
    folder="$(rb_package_unzip "$package" "$dest")" || exit 2
fi
# What tests/e2e/lib/app.ts requires before it empties the folder's data\.
: > "$folder/.rbrowser-e2e"

if [[ -z "${RB_E2E_RUNNER:-}" ]]; then
    rb_npm_install_if_stale "$ROOT/tests/e2e" || exit 2
fi
echo "==> end-to-end run against $folder"
set +e
runner --app "$folder" "${runner_args[@]}"
status=$?
set -e
exit "$status"

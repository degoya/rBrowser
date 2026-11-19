#!/usr/bin/env bash
#
# The checks of the repository, one mode per occasion. Every stage runs even after another one
# failed; the summary lists them all and the run ends with `REAL EXIT: <n>`, and with
# `==> all requested checks passed` only when every stage was green.
#
# Usage:
#   scripts/check.sh --preflight   # compiles nothing: cargo fmt --check, bash -n, shellcheck
#                                  # (at the pinned version), the script tests, gitleaks,
#                                  # git diff --check, the job layout (archive-jobs.sh --check),
#                                  # the file length, the i18n lint (check-i18n.sh),
#                                  # cargo deny --offline check,
#                                  # set-version.sh --check, tool-versions.sh --check, the
#                                  # settings lint and reference (check-settings.sh,
#                                  # settings-reference.sh --check), the keyboard reference
#                                  # (keyboard-reference.sh --check), the MCP tool
#                                  # reference (mcp-reference.sh --check), the action pins
#                                  # (check-actions-pinned.sh), actionlint over .github/ and
#                                  # sdk/ci/ (at the pinned version), and
#                                  # `npm run build && npm test` in ui/, agent/, sidecar/ and
#                                  # the plugins' workspace plugins/ (each only once it exists;
#                                  # `npm ci` first when node_modules/ is missing or older than
#                                  # package-lock.json, scripts/lib/npm.sh); no lock,
#                                  # but each npm stage waits for free memory
#   scripts/check.sh --rust        # clippy over the workspace (-D warnings), the app again with
#                                  # `--no-default-features`, without self-update (RB-090-14),
#                                  # every test
#                                  # (--no-fail-fast: every failing test is named); the native
#                                  # tests of each first-party plugin, its unsigned build
#                                  # (build-plugins.sh --unsigned) and the analyser's end-to-end
#                                  # test on that build (analyzer_e2e, RB-010-13), and
#                                  # Playwright's connectOverCDP against the CDP proxy
#                                  # (playwright-e2e, RB-040-06; node and sidecar/node_modules);
#                                  # the plugins build and test in target/lanes/plugins
#                                  # (RB-080-25);
#                                  # the WSL helper rbrowser-wsl built for musl and held to a
#                                  # static x86-64 executable (wsl-helper, RB-030-04)
#   scripts/check.sh --windows     # cargo xwin clippy for x86_64-pc-windows-msvc (-D warnings),
#                                  # all targets, then the libraries alone with the features
#                                  # they ship with, then the app with `--no-default-features`;
#                                  # in target/lanes/windows
#   scripts/check.sh --full        # all three (also the default without a mode)
#
# A green --full on a clean tree leaves a stamp, HEAD's commit in `$(git rev-parse --git-path
# rb-check-full)` of that checkout (per worktree); `scripts/worktree.sh finish` warns when the
# branch it merges has none for its tip. The tests run in nextest's profile NEXTEST_PROFILE
# (nextest's own variable; `ci` in .config/nextest.toml).
#
# A tool a preflight stage needs (shellcheck, gitleaks, actionlint, cargo-deny, npm) that is not
# installed, or a ShellCheck or actionlint other than the version scripts/lib/tool-versions.sh
# pins, is a visible skip
# under --preflight and a failure under --full, the gate.
#
# Logs: one directory per run, $RB_CHECK_LOGS/<time>-<pid>/ (default RB_CHECK_LOGS is
# /tmp/claude-<uid>/check-<checkout>), one log per stage plus `failures` and `summary`.
# Parallelism: scripts/lib/jobs.sh (RB_JOBS, RB_CLIPPY_JOBS, RB_TEST_THREADS). The compiling modes
# take the build lock (scripts/lib/lock.sh) - never wrap this script in flock.
# shellcheck disable=SC2317,SC2329  # the stage functions are called through stage(), not directly
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WINDOWS_TARGET="x86_64-pc-windows-msvc"
LIMIT=500

usage() {
    echo "usage: scripts/check.sh [--preflight|--rust|--windows|--full]" >&2
    exit 2
}

[[ $# -le 1 ]] || usage
mode="${1:---full}"
case "$mode" in
    --preflight | --rust | --windows | --full) ;;
    -h | --help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) usage ;;
esac

# shellcheck source=lib/jobs.sh
source "$ROOT/scripts/lib/jobs.sh"
# shellcheck source=lib/tool-versions.sh
source "$ROOT/scripts/lib/tool-versions.sh"
# shellcheck source=lib/wsl-helper.sh
source "$ROOT/scripts/lib/wsl-helper.sh"
# shellcheck source=lib/npm.sh
source "$ROOT/scripts/lib/npm.sh"
# shellcheck source=lib/lock.sh
source "$ROOT/scripts/lib/lock.sh"
[[ "$mode" == --preflight ]] || rb_take_lock "$@"
cd "$ROOT"

LOGS="${RB_CHECK_LOGS:-/tmp/claude-$(id -u)/check-$(basename "$ROOT")}/$(date +%Y%m%d-%H%M%S)-$$"
mkdir -p "$LOGS"
: > "$LOGS/failures"
: > "$LOGS/summary"
echo "==> scripts/check.sh $mode in $ROOT (jobs $JOBS, clippy jobs $CLIPPY_JOBS, test threads $TEST_THREADS)"
echo "    logs: $LOGS"

failed=0
skipped=0

# The commit a green --full may stamp: HEAD, when the tree is clean as the run starts.
stamp_head=""
if [[ "$mode" == --full ]] && git rev-parse -q --verify HEAD > /dev/null \
    && [[ -z "$(git status --porcelain)" ]]; then
    stamp_head="$(git rev-parse HEAD)"
fi

# Runs one stage: `stage <name> <command...>`, its output into <name>.log. A failure is recorded
# and its log's end printed; the run goes on.
stage() {
    local name="$1" started status=0
    shift
    started="$(date +%s)"
    echo "--> $name"
    "$@" > "$LOGS/$name.log" 2>&1 || status=$?
    local took=$(($(date +%s) - started))
    if [[ "$status" -eq 0 ]]; then
        printf '  ok   %-14s %4ss\n' "$name" "$took" | tee -a "$LOGS/summary"
    else
        printf '  FAIL %-14s %4ss  exit %s  %s\n' "$name" "$took" "$status" "$LOGS/$name.log" \
            | tee -a "$LOGS/summary"
        echo "$name exit $status $LOGS/$name.log" >> "$LOGS/failures"
        echo "     --- last 30 lines of $name.log ---"
        tail -n 30 "$LOGS/$name.log" | sed 's/^/     /'
        failed=1
    fi
}

skip() {
    printf '  skip %-14s       %s\n' "$1" "$2" | tee -a "$LOGS/summary"
    skipped=$((skipped + 1))
}

# A stage whose tool is missing or not at its pinned version: a skip under --preflight alone, a
# failure under --full, so the gate never passes with a stage that did not run.
missing() {
    local name="$1" reason="$2"
    if [[ "$mode" == --full ]]; then
        printf '  FAIL %-14s       %s\n' "$name" "$reason" | tee -a "$LOGS/summary"
        echo "$name $reason" >> "$LOGS/failures"
        failed=1
    else
        skip "$name" "$reason (a failure under --full)"
    fi
}

# Every shell script of the repository, NUL-separated.
shell_scripts() {
    find scripts .claude/hooks -type f -name '*.sh' -print0 2> /dev/null
}

syntax() {
    local file status=0
    while IFS= read -r -d '' file; do
        bash -n "$file" || status=1
    done < <(shell_scripts)
    while IFS= read -r -d '' file; do
        python3 -c 'import ast, sys; ast.parse(open(sys.argv[1], encoding="utf-8").read(), sys.argv[1])' \
            "$file" || status=1
    done < <(find scripts .claude/hooks -type f -name '*.py' -print0 2> /dev/null)
    return "$status"
}

run_shellcheck() {
    local files=()
    while IFS= read -r -d '' file; do files+=("$file"); done < <(shell_scripts)
    shellcheck -x -P SCRIPTDIR "${files[@]}"
}

script_tests() {
    local test status=0
    for test in scripts/tests/*.sh; do
        [[ -e "$test" ]] || continue
        echo "==> $test"
        bash "$test" || status=1
    done
    return "$status"
}

# gitleaks over the tracked and the new (not ignored) files of the working tree, copied out and
# scanned from inside so that .gitleaks.toml applies and target/ is never read.
secret_scan() {
    local scanner="$1" copy status=0
    copy="$(mktemp -d)"
    git ls-files -z --cached --others --exclude-standard \
        | tar --null --ignore-failed-read -T - -cf - 2> /dev/null | tar -xf - -C "$copy" || status=$?
    if [[ "$status" -ne 0 || -z "$(find "$copy" -type f -print -quit)" ]]; then
        echo "!! copying the working tree for the scan failed (exit $status); nothing was scanned"
        rm -rf "$copy"
        return 1
    fi
    (cd "$copy" && "$scanner" dir . --no-banner --redact --exit-code 1) || status=$?
    rm -rf "$copy"
    [[ "$status" -eq 0 ]] || echo "!! a real secret is removed at the source; a fixture gets an entry in .gitleaks.toml"
    return "$status"
}

# Whitespace errors in the working tree against HEAD, or against the empty tree before the first
# commit, new files included. The new files are marked intent-to-add in a copy of the index, so
# the real one is never touched.
diff_check() {
    local base index status=0
    base="$(git rev-parse -q --verify HEAD || git hash-object -t tree /dev/null)"
    index="$(mktemp -u)"
    cp "$(git rev-parse --git-path index)" "$index" 2> /dev/null || true
    GIT_INDEX_FILE="$index" git add -A -N . || status=$?
    [[ "$status" -ne 0 ]] || GIT_INDEX_FILE="$index" git diff --check "$base" || status=$?
    rm -f "$index"
    return "$status"
}

# The 500-line rule without compiling: the same rule as
# crates/rbrowser/tests/repo_lints/file_length.rs, whose lists are empty - Rust sources and
# scripts, and the TypeScript and Vue sources of the front ends. It reads the plugins and the SDK
# too, so a wave agent who compiles nothing still sees a long plugin file. The spikes are
# throw-away and not read.
file_length() {
    local file lines status=0
    while IFS= read -r -d '' file; do
        lines="$(wc -l < "$file")"
        if [[ "$lines" -gt "$LIMIT" ]]; then
            echo "$file: $lines lines (limit $LIMIT; split it)"
            status=1
        fi
    done < <(find crates plugins sdk scripts .claude/hooks ui agent sidecar \
        \( -name target -o -name node_modules -o -name dist \) -prune -o -type f \
        \( -name '*.rs' -o -name '*.sh' -o -name '*.py' -o -name '*.ts' -o -name '*.mts' \
        -o -name '*.js' -o -name '*.mjs' -o -name '*.vue' \) -print0 2> /dev/null)
    return "$status"
}

# The front ends that exist (ui/, agent/, sidecar/, and the plugins' npm workspace plugins/ -
# the page-script, panel and runner projects under plugins/<id>/, one lock file and one install;
# RB-080-25), one per line: those with a package.json.
npm_packages() {
    local dir
    for dir in ui agent sidecar plugins; do
        [[ -f "$dir/package.json" ]] && echo "$dir"
    done
    return 0
}

# `npm ci` when node_modules/ is missing or older than package-lock.json (scripts/lib/npm.sh),
# then `npm run build && npm test` in front end $1, with the node .nvmrc pins
# (rb_use_pinned_node: fnm or nvm activate it for the rest of the run; without it this stage
# fails with the install command, and the run goes on). In plugins/ the scripts run every
# workspace. The preflight takes no lock, so each npm stage first waits for RB_MIN_FREE_MB of free
# memory (rb_wait_for_memory): several agents' preflights at once, each with its own Vite builds
# and Vitest workers, took WSL down on 2026-11-15.
npm_check() {
    local dir="$1"
    rb_wait_for_memory
    rb_use_pinned_node "$ROOT" || return 1
    rb_npm_install_if_stale "$dir" || return 1
    (cd "$dir" && npm run build && npm test)
}

# The installed ShellCheck's version; scripts/lib/tool-versions.sh pins one, since another
# version reads the scripts differently.
shellcheck_version() {
    shellcheck --version | sed -n 's/^version: //p'
}

# The workflows actionlint reads: the repository's own and the SDK's (RB-090-04). shellcheck, when
# installed, lints their `run:` blocks too.
run_actionlint() {
    local files=()
    while IFS= read -r -d '' file; do files+=("$file"); done < <(find .github/workflows sdk/ci \
        -type f \( -name '*.yml' -o -name '*.yaml' \) -print0 2> /dev/null | sort -z)
    if [[ ${#files[@]} -eq 0 ]]; then
        echo "no workflows"
        return 0
    fi
    actionlint "${files[@]}"
}

preflight() {
    echo "==> preflight"
    stage fmt cargo fmt --all --check
    stage syntax syntax
    if command -v shellcheck > /dev/null 2>&1; then
        stage shellcheck run_shellcheck
        local found
        found="$(shellcheck_version)"
        [[ "$found" == "$RB_SHELLCHECK_VERSION" ]] || missing shellcheck-version \
            "$found installed, $RB_SHELLCHECK_VERSION pinned (AGENTS.md, Toolchain)"
    else
        missing shellcheck "not installed (AGENTS.md, Toolchain)"
    fi
    stage script-tests script_tests
    local scanner
    scanner="${GITLEAKS:-$(command -v gitleaks || true)}"
    if [[ -n "$scanner" ]]; then
        stage gitleaks secret_scan "$scanner"
    else
        missing gitleaks "not installed (AGENTS.md, Toolchain)"
    fi
    stage diff-check diff_check
    if [[ -x scripts/archive-jobs.sh ]]; then
        stage archive-jobs scripts/archive-jobs.sh --check
    else
        skip archive-jobs "scripts/archive-jobs.sh is missing"
    fi
    stage file-length file_length
    stage links python3 -I scripts/lib/check_links.py .
    # The settings lint and the wiki's settings reference (RB-010-02, docs/settings.md).
    stage settings scripts/check-settings.sh
    stage settings-ref scripts/settings-reference.sh --check
    # The wiki's keyboard reference from schema/commands.v1.json (RB-010-17).
    stage keyboard-ref scripts/keyboard-reference.sh --check
    # The wiki's MCP tool reference from schema/mcp-tools.v1.json (RB-030-08).
    stage mcp-ref scripts/mcp-reference.sh --check
    # The GitHub workflows (RB-090-04): every action pinned to a commit, and actionlint.
    stage actions-pinned scripts/check-actions-pinned.sh
    if command -v actionlint > /dev/null 2>&1; then
        stage actionlint run_actionlint
        local linted
        linted="$(actionlint --version | head -n 1)"
        [[ "$linted" == "$RB_ACTIONLINT_VERSION" ]] || missing actionlint-version \
            "$linted installed, $RB_ACTIONLINT_VERSION pinned (AGENTS.md, Toolchain)"
    else
        missing actionlint "not installed (AGENTS.md, Toolchain; scripts/ci-tools.sh linters)"
    fi
    stage i18n scripts/check-i18n.sh
    if cargo deny --version > /dev/null 2>&1; then
        stage deny cargo deny --offline check
    else
        missing deny "cargo-deny is not installed (AGENTS.md, Toolchain)"
    fi
    stage version scripts/set-version.sh --check
    stage tool-versions scripts/tool-versions.sh --check
    local dir
    while IFS= read -r dir; do
        if command -v npm > /dev/null 2>&1; then
            stage "npm-${dir//\//-}" npm_check "$dir"
        else
            missing "npm-${dir//\//-}" "npm is not installed (.nvmrc pins Node)"
        fi
    done < <(npm_packages)
}

# The native tests of every first-party plugin (each its own workspace under plugins/), all in
# the one target directory scripts/build-plugins.sh builds them in (RB-080-25), so what the
# plugins share is compiled once.
plugin_tests() {
    local id status=0
    while IFS= read -r id; do
        [[ -n "$id" ]] || continue
        echo "==> plugins/$id"
        CARGO_BUILD_JOBS="$JOBS" CARGO_TARGET_DIR="${RB_PLUGIN_TARGET_DIR:-$ROOT/target/lanes/plugins}" \
            cargo test --locked --manifest-path "plugins/$id/Cargo.toml" \
            -- --test-threads "$TEST_THREADS" || status=1
    done < <(scripts/build-plugins.sh --list)
    return "$status"
}

# The WSL helper as the package ships it (RB-030-04): built for x86_64-unknown-linux-musl and
# held to a static x86-64 executable; the dev profile, so it costs little.
wsl_helper() {
    local helper
    helper="$(rb_wsl_helper_build dev "$JOBS")" || return 1
    rb_wsl_helper_check "$helper"
}

rust() {
    echo "==> rust"
    stage clippy env CARGO_BUILD_JOBS="$CLIPPY_JOBS" \
        cargo clippy --locked --workspace --all-targets -j "$CLIPPY_JOBS" -- -D warnings
    # The app without self-update (RB-090-14: on by default), linted in both variants.
    stage clippy-no-self-update env CARGO_BUILD_JOBS="$CLIPPY_JOBS" \
        cargo clippy --locked -p rbrowser --all-targets --no-default-features -j "$CLIPPY_JOBS" \
        -- -D warnings
    stage tests env CARGO_BUILD_JOBS="$JOBS" \
        cargo nextest run --locked --workspace --no-fail-fast --test-threads "$TEST_THREADS"
    stage wsl-helper wsl_helper
    # Ignored in the run above: they start node with the sidecar's playwright-core (RB-040-06)
    # and its flow worker (RB-040-07), which the preflight's npm stage installs.
    if command -v node > /dev/null 2>&1 && [[ -d sidecar/node_modules/playwright-core ]]; then
        stage playwright-e2e env CARGO_BUILD_JOBS="$JOBS" \
            cargo nextest run --locked -p rbrowser --lib --run-ignored only \
            -E 'test(/^(cdp_proxy::playwright_tests|flows::e2e_tests)::/)' \
            --test-threads "$TEST_THREADS"
    else
        missing playwright-e2e "needs node and sidecar/node_modules (npm ci in sidecar/)"
    fi
    stage plugin-tests plugin_tests
    if ! command -v wasm-tools > /dev/null 2>&1; then
        missing plugin-build "wasm-tools is not installed (scripts/build-plugins.sh)"
        missing analyzer-e2e "needs the plugin build"
        return 0
    fi
    # Unsigned: the gate never needs the release key; the e2e test loads the component in
    # development mode.
    stage plugin-build env CARGO_BUILD_JOBS="$JOBS" scripts/build-plugins.sh --unsigned
    stage analyzer-e2e env CARGO_BUILD_JOBS="$JOBS" \
        cargo nextest run --locked -p rbrowser --test analyzer_e2e --run-ignored all \
        --test-threads "$TEST_THREADS"
}

windows() {
    echo "==> windows"
    if ! cargo xwin --version > /dev/null 2>&1; then
        skip windows-clippy "cargo-xwin is not installed (AGENTS.md, Toolchain)"
        failed=1
        echo "windows-clippy cargo-xwin missing" >> "$LOGS/failures"
        return 0
    fi
    stage windows-clippy env CARGO_BUILD_JOBS="$CLIPPY_JOBS" CARGO_TARGET_DIR="$ROOT/target/lanes/windows" \
        cargo xwin clippy --locked --target "$WINDOWS_TARGET" --workspace --all-targets \
        -j "$CLIPPY_JOBS" -- -D warnings
    # `--all-targets` builds the tests, which switches on the dev-dependencies' features
    # (`test-support`, `testkit`, `memory`, ...) in every library; the libraries as they ship are
    # linted only by this second run.
    stage windows-lib env CARGO_BUILD_JOBS="$CLIPPY_JOBS" CARGO_TARGET_DIR="$ROOT/target/lanes/windows" \
        cargo xwin clippy --locked --target "$WINDOWS_TARGET" --workspace -j "$CLIPPY_JOBS" \
        -- -D warnings
    # The app without self-update (`--no-default-features`, RB-090-14), as a build may ship it.
    stage windows-no-self-update env CARGO_BUILD_JOBS="$CLIPPY_JOBS" \
        CARGO_TARGET_DIR="$ROOT/target/lanes/windows" \
        cargo xwin clippy --locked --target "$WINDOWS_TARGET" -p rbrowser --all-targets \
        --no-default-features -j "$CLIPPY_JOBS" -- -D warnings
}

# The stamp of a green --full, for scripts/worktree.sh finish: only when the tree was clean from
# the start, HEAD is the commit it started on and nothing changed the tree on the way.
write_stamp() {
    local file
    if [[ -z "$stamp_head" ]]; then
        echo "    no green stamp: the tree had uncommitted changes when the run started"
        return 0
    fi
    if [[ "$(git rev-parse HEAD)" != "$stamp_head" || -n "$(git status --porcelain)" ]]; then
        echo "    no green stamp: HEAD or the tree changed during the run"
        return 0
    fi
    file="$(git rev-parse --path-format=absolute --git-path rb-check-full)"
    printf '%s %s %s\n' "$stamp_head" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$LOGS" > "$file"
    echo "    green stamp for ${stamp_head:0:12} in $file"
}

case "$mode" in
    --preflight) preflight ;;
    --rust) rust ;;
    --windows) windows ;;
    --full)
        preflight
        rust
        windows
        ;;
esac

echo
echo "==> summary ($mode)"
cat "$LOGS/summary"
if [[ "$failed" -eq 0 ]]; then
    [[ "$skipped" -eq 0 ]] || echo "    $skipped stage(s) skipped, see above"
    [[ "$mode" != --full || "$skipped" -ne 0 ]] || write_stamp
    echo "==> all requested checks passed"
else
    echo "!! failed stages (logs in $LOGS):"
    sed 's/^/   /' "$LOGS/failures"
fi
echo "REAL EXIT: $failed"
exit "$failed"

#!/usr/bin/env bash
#
# scripts/e2e-windows.sh (RB-099-02) without Windows: the usage errors, the staging of an app
# folder and of a package ZIP into the staging folder (RB_E2E_STAGE_DIR) with the marker
# tests/e2e/lib/app.ts requires, the arguments the runner gets and its exit code passed on - the
# runner is a fake (RB_E2E_RUNNER). The plan of `--list` with the real runner when node is there.
#
#   scripts/tests/e2e-windows.sh
# shellcheck disable=SC2016  # single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

SCRIPT="$ROOT/scripts/e2e-windows.sh"

# The fake runner: prints its arguments, exits with FAKE_EXIT.
cat > "$SCRATCH/runner.sh" << 'EOF'
#!/usr/bin/env bash
echo "runner: $*"
exit "${FAKE_EXIT:-0}"
EOF
export RB_E2E_RUNNER="bash $SCRATCH/runner.sh"
export RB_E2E_STAGE_DIR="$SCRATCH/rbrowser-e2e"

app="$SCRATCH/built"
mkdir -p "$app/locales"
: > "$app/rbrowser.exe"
: > "$app/rbrowser.com"
: > "$app/locales/en-US.pak"

run_status "$SCRIPT" --fast
expect_status "an unknown argument is a usage error" 2
expect_output "it names the argument" "unknown argument --fast"

run_status "$SCRIPT" --only
expect_status "--only without its value" 2
expect_output "it says so" "--only needs a value"

run_status "$SCRIPT" --app "$app" --package "$SCRATCH/x.zip"
expect_status "--app and --package together" 2

run_status "$SCRIPT" --list --only K4,K10 --native
expect_status "--list runs the runner's plan" 0
expect_output "the plan gets --list and the options" "runner: --list --only K4,K10 --native"
expect_true "--list stages nothing" '[[ ! -e "$RB_E2E_STAGE_DIR" ]]'

run_status "$SCRIPT" --app "$SCRATCH/none"
expect_status "an app folder without rbrowser.exe" 2
expect_output "it names what is missing" "holds no rbrowser.exe and rbrowser.com"

run_status "$SCRIPT" --app "$app" --only K4 --keep --allow-missing firefox
expect_status "a run that passes" 0
expect_output "the runner gets the staged folder and the options" \
    "runner: --app $RB_E2E_STAGE_DIR/app --only K4 --keep --allow-missing firefox"
expect_true "the app is copied" '[[ -f "$RB_E2E_STAGE_DIR/app/locales/en-US.pak" ]]'
expect_true "the staged folder carries the marker" '[[ -f "$RB_E2E_STAGE_DIR/app/.rbrowser-e2e" ]]'

FAKE_EXIT=1 run_status "$SCRIPT" --app "$app"
expect_status "a failed criterion is exit 1" 1

python3 - "$SCRATCH/rbrowser-1.2.3-windows-x64.zip" << 'PY'
import sys, zipfile
with zipfile.ZipFile(sys.argv[1], "w") as package:
    package.writestr("rbrowser-1.2.3-windows-x64/rbrowser.exe", "")
    package.writestr("rbrowser-1.2.3-windows-x64/rbrowser.com", "")
PY
run_status "$SCRIPT" --package "$SCRATCH/rbrowser-1.2.3-windows-x64.zip"
expect_status "a package is unpacked and run" 0
expect_output "into its own folder" "runner: --app $RB_E2E_STAGE_DIR/rbrowser-1.2.3-windows-x64"
expect_true "the unpacked folder carries the marker" \
    '[[ -f "$RB_E2E_STAGE_DIR/rbrowser-1.2.3-windows-x64/.rbrowser-e2e" ]]'

run_status "$SCRIPT" --package "$SCRATCH/missing.zip"
expect_status "a missing package" 2
expect_output "it names how to make one" "scripts/package-windows.sh makes it"

RB_E2E_STAGE_DIR="$SCRATCH/elsewhere" run_status "$SCRIPT" --app "$app"
expect_status "a staging folder of another name is refused" 2
expect_true "nothing is copied there" '[[ ! -e "$SCRATCH/elsewhere/app" ]]'

if command -v node > /dev/null 2>&1; then
    RB_E2E_RUNNER="node $ROOT/tests/e2e/run.ts" run_status "$SCRIPT" --list --only K10
    expect_status "the real plan" 0
    expect_output "it lists the criterion" "K10  CDP proxy token and targets"
else
    ok "the real plan (skipped: no node)"
fi

finish_tests e2e-windows

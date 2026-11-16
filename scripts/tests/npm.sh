#!/usr/bin/env bash
#
# scripts/lib/npm.sh (RB-080-25), which the npm stages of scripts/check.sh use: `npm ci` runs
# when node_modules/ is missing and when package-lock.json is newer than the install's own
# node_modules/.package-lock.json (a lock file changed by a merge or `npm install`), and not
# otherwise. A fake npm records its calls and writes the hidden lock file as npm does.
#
# Pure bash, a second.
#
#   scripts/tests/npm.sh
# shellcheck disable=SC2016  # single-quoted on purpose: a child bash evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

BIN="$SCRATCH/bin"
mkdir -p "$BIN"
cat > "$BIN/npm" << 'SH'
#!/usr/bin/env bash
echo "$(basename "$PWD"): $*" >> "$RB_FAKE_NPM_LOG"
if [[ "$1" == ci ]]; then
    mkdir -p node_modules
    cp package-lock.json node_modules/.package-lock.json
fi
SH
chmod +x "$BIN/npm"
export RB_FAKE_NPM_LOG="$SCRATCH/npm.log"

# Runs snippet $1 in a bash that sourced the library, the fake npm first on PATH; $2... are its
# positional parameters.
in_lib() {
    local snippet="$1"
    shift
    run_status env PATH="$BIN:$PATH" bash -c \
        'set -euo pipefail; source "$RB_ROOT/scripts/lib/npm.sh"; '"$snippet" _ "$@"
}
export RB_ROOT="$ROOT"
calls() { paste -sd '|' "$RB_FAKE_NPM_LOG" 2> /dev/null || true; }

DIR="$SCRATCH/front"
mkdir -p "$DIR"
printf '{}\n' > "$DIR/package.json"
printf '{"lockfileVersion": 3}\n' > "$DIR/package-lock.json"

in_lib 'rb_npm_install_needed "$1"' "$DIR"
expect_status "without node_modules an install is needed" 0

in_lib 'rb_npm_install_if_stale "$1"' "$DIR"
expect_status "and rb_npm_install_if_stale runs it" 0
expect "as npm ci" "front: ci" "$(calls)"
expect_true "which leaves the hidden lock file" '[[ -f "$DIR/node_modules/.package-lock.json" ]]'

in_lib 'rb_npm_install_needed "$1"' "$DIR"
expect_status "an install as new as the lock file needs none" 1
rm -f "$RB_FAKE_NPM_LOG"
in_lib 'rb_npm_install_if_stale "$1"' "$DIR"
expect_status "and rb_npm_install_if_stale does nothing" 0
expect "no npm call" "" "$(calls)"

touch -d '2000-01-01 00:00:00' "$DIR/node_modules/.package-lock.json"
touch -d '2000-01-02 00:00:00' "$DIR/package-lock.json"
in_lib 'rb_npm_install_needed "$1"' "$DIR"
expect_status "a lock file newer than the install needs one" 0
rm -f "$RB_FAKE_NPM_LOG"
in_lib 'rb_npm_install_if_stale "$1"' "$DIR"
expect_status "and rb_npm_install_if_stale runs npm ci again" 0
expect "once" "front: ci" "$(calls)"
in_lib 'rb_npm_install_needed "$1"' "$DIR"
expect_status "after which the install is current" 1

rm -f "$DIR/node_modules/.package-lock.json"
in_lib 'rb_npm_install_needed "$1"' "$DIR"
expect_status "a node_modules without the hidden lock file needs one" 0

finish_tests npm

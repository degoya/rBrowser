#!/usr/bin/env bash
#
# scripts/self-update-smoke.sh (RB-090-14) without builds: the usage errors - a wrong count of
# arguments, a folder without the stand-in program or the updater, the same version twice - end
# with exit 2 before anything is installed; the workflow builds both versions and runs it on
# Linux and Windows. The run itself needs real binaries (.github/workflows/self-update.yml).
#
# Pure bash, a second.
#
#   scripts/tests/self-update-smoke.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

SMOKE="$ROOT/scripts/self-update-smoke.sh"
ext=""
case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) ext=".exe" ;; esac

run_status "$SMOKE" only three args
expect_status "three arguments are a usage error" 2
expect_output "and the usage is named" "usage: scripts/self-update-smoke.sh"

mkdir -p "$SCRATCH/old" "$SCRATCH/new"
run_status "$SMOKE" "$SCRATCH/old" 0.9.0 "$SCRATCH/new" 0.9.1
expect_status "a folder without the binaries is a usage error" 2
expect_output "and the missing one is named" "update_smoke_program$ext is missing"

for dir in old new; do
    for binary in update_smoke_program rbrowser-updater; do
        printf 'stand-in\n' > "$SCRATCH/$dir/$binary$ext"
    done
done
run_status "$SMOKE" "$SCRATCH/old" 0.9.0 "$SCRATCH/new" 0.9.0
expect_status "the same version twice is a usage error" 2
expect_output "and is named" "the versions are the same"

expect_true "the workflow runs the smoke on Linux and Windows" \
    'grep -q "ubuntu-24.04" "$ROOT/.github/workflows/self-update.yml" && grep -q "windows-2025" "$ROOT/.github/workflows/self-update.yml"'
expect_true "the stand-in program is rb-update's example" \
    '[[ -f "$ROOT/crates/rb-update/examples/update_smoke_program.rs" ]]'

finish_tests "self-update-smoke"

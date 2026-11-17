#!/usr/bin/env bash
#
# scripts/lib/licenses.py (the worker of scripts/licenses.sh, RB-010-18) against a fixture: two
# Rust roots (cargo metadata + cargo tree), an npm lock file with a production, a dev and an
# optional package. It lists a crate a tree names with its licence and every other locked crate
# as not shipped, the production npm package with its licence and the dev and optional ones as
# not shipped; --check then passes and finds the list stale after a lock file changes; a package
# without a licence stops it until an override names one; an override for no package stops it.
#
# Python only, compiles nothing: check.sh --preflight runs it.
#
#   scripts/tests/licenses.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
WORKER="$ROOT/scripts/lib/licenses.py"
LIST=ui/src/about/licenses/third-party.json

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

repo="$SCRATCH/repo"
inputs="$SCRATCH/inputs"
mkdir -p "$repo/ui/src/about/licenses" "$inputs"
cat > "$inputs/app.metadata.json" << 'EOF'
{ "packages": [
  { "name": "rbrowser", "version": "0.1.0", "source": null, "license": "GPL-3.0-or-later" },
  { "name": "serde", "version": "1.0.0", "source": "registry", "license": "MIT OR Apache-2.0" },
  { "name": "syn", "version": "2.0.0", "source": "registry", "license": "MIT OR Apache-2.0" }
] }
EOF
printf '%s\n' 'rbrowser v0.1.0 (/x/crates/rbrowser)' 'serde v1.0.0' 'serde v1.0.0 (*)' > "$inputs/app.tree.txt"
cat > "$inputs/plugin-a.metadata.json" << 'EOF'
{ "packages": [ { "name": "bitflags", "version": "2.0.0", "source": "registry", "license": "" } ] }
EOF
echo 'bitflags v2.0.0' > "$inputs/plugin-a.tree.txt"
cat > "$repo/ui/package-lock.json" << 'EOF'
{ "lockfileVersion": 3, "packages": {
  "": { "name": "shell" },
  "node_modules/vue": { "version": "3.5.0", "license": "MIT" },
  "node_modules/vite": { "version": "8.0.0", "license": "MIT", "dev": true },
  "node_modules/fsevents": { "version": "2.3.3", "license": "MIT", "optional": true }
} }
EOF

run_status python3 -B "$WORKER" "$repo" "$inputs"
expect_status "a crate without a licence and without an override stops the run" 2
expect_output "and names it" "bitflags@2.0.0: declares no licence"

cat > "$repo/ui/src/about/licenses/overrides.json" << 'EOF'
{ "rust": { "bitflags@2.0.0": { "license": "MIT OR Apache-2.0", "source": "its LICENSE files" } }, "npm": {} }
EOF
run_status python3 -B "$WORKER" "$repo" "$inputs"
expect_status "with the override the list is written" 0
# shellcheck disable=SC2034  # read by the conditions expect_true evaluates
list="$(cat "$repo/$LIST")"
expect_true "a crate the tree names is listed with its licence" \
    '[[ "$list" == *"{\"name\": \"serde\", \"version\": \"1.0.0\", \"license\": \"MIT OR Apache-2.0\"}"* ]]'
expect_true "the override gives the licence" '[[ "$list" == *"\"bitflags\", \"version\": \"2.0.0\", \"license\": \"MIT OR Apache-2.0\""* ]]'
expect_true "a locked crate no tree names is not shipped" '[[ "$list" == *"\"syn@2.0.0\""* ]]'
expect_true "the workspace member is not listed" '[[ "$list" != *"rbrowser"* ]]'
expect_true "the production npm package is listed" '[[ "$list" == *"{\"name\": \"vue\", \"version\": \"3.5.0\", \"license\": \"MIT\"}"* ]]'
expect_true "dev and optional npm packages are not shipped" \
    '[[ "$list" == *"\"vite@8.0.0\""* && "$list" == *"\"fsevents@2.3.3\""* ]]'

run_status python3 -B "$WORKER" "$repo" "$inputs" --check
expect_status "--check passes on a current list" 0

sed -i 's/"3.5.0"/"3.5.1"/' "$repo/ui/package-lock.json"
run_status python3 -B "$WORKER" "$repo" "$inputs" --check
expect_status "--check finds the list stale after a lock file changed" 1

cat > "$repo/ui/src/about/licenses/overrides.json" << 'EOF'
{ "rust": { "bitflags@2.0.0": { "license": "MIT" } }, "npm": { "gone@1.0.0": { "license": "MIT" } } }
EOF
run_status python3 -B "$WORKER" "$repo" "$inputs"
expect_status "an override for no package stops the run" 2
expect_output "and names it" "gone@1.0.0: an override for no locked package"

run_status python3 -B "$WORKER" "$repo" "$SCRATCH/empty"
expect_status "no Rust root is an error" 2

finish_tests licenses

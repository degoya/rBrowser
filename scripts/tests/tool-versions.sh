#!/usr/bin/env bash
#
# scripts/tool-versions.sh --check on a copy of the tracked tree: the copy as it is passes, a
# .nvmrc that names another Node version than the pin fails with exit 1 and names the mismatch,
# the packaged Node runtime's SHA-256 pin (RB-010-15) included, and so does a workflow that
# installs another cargo-deny than the pin (RB-090-04) and a WebKit SHA-256 in the sidecar that is
# not the pin's (RB-040-04).
#
#   scripts/tests/tool-versions.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

(cd "$ROOT" && git ls-files -z | xargs -0 cp --parents -t "$SCRATCH")

run_status "$ROOT/scripts/tool-versions.sh" --check --root "$SCRATCH"
expect_status "the tracked tree agrees with its pins" 0

pinned="$(cat "$SCRATCH/.nvmrc")"
echo "${pinned%.*}.999" > "$SCRATCH/.nvmrc"
run_status "$ROOT/scripts/tool-versions.sh" --check --root "$SCRATCH"
expect_status "a .nvmrc with another Node version fails" 1
expect_output "the mismatch names Node" "node"
expect_output "and the packaged runtime's pin" "RB_NODE_WIN_X64_SHA256 is for node"
echo "$pinned" > "$SCRATCH/.nvmrc"

mkdir -p "$SCRATCH/.github/workflows"
printf '%s\n' 'jobs:' '  a:' '    steps:' '      - with:' '          tool: cargo-deny@0.0.1' \
    > "$SCRATCH/.github/workflows/stray.yml"
run_status "$ROOT/scripts/tool-versions.sh" --check --root "$SCRATCH"
expect_status "a workflow installing another cargo-deny fails" 1
expect_output "the mismatch names cargo-deny" "cargo-deny 0.0.1"

pin_ts="$SCRATCH/sidecar/src/webkit/pin.ts"
cp "$pin_ts" "$SCRATCH/pin.ts.orig"
sed -i -E "s/^  sha256: '[0-9a-f]{64}',$/  sha256: '$(printf '0%.0s' {1..64})',/" "$pin_ts"
run_status "$ROOT/scripts/tool-versions.sh" --check --root "$SCRATCH"
expect_status "a sidecar WebKit pin with another SHA-256 fails" 1
expect_output "the mismatch names the WebKit pin" "webkit-sha256"
cp "$SCRATCH/pin.ts.orig" "$pin_ts"

finish_tests tool-versions

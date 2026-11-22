#!/usr/bin/env bash
#
# scripts/tool-versions.sh --check on a copy of the tracked tree: the copy as it is passes, a
# .nvmrc that names another Node version than the pin fails with exit 1 and names the mismatch,
# the packaged Node runtime's SHA-256 pins (RB-010-15; macOS, RB-097-01) included, and so does a
# macOS Node pin that is no SHA-256, a workflow that installs another cargo-deny than the pin
# (RB-090-04) and a WebKit SHA-256 in the sidecar that is not the pin's (RB-040-04).
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
expect_output "and the macOS bundle's pins" "RB_NODE_DARWIN_ARM64_SHA256 and RB_NODE_DARWIN_X64_SHA256 are for node"
echo "$pinned" > "$SCRATCH/.nvmrc"

lib="$SCRATCH/scripts/lib/tool-versions.sh"
cp "$lib" "$SCRATCH/tool-versions.sh.orig"
sed -i -E 's/^RB_NODE_DARWIN_X64_SHA256="[0-9a-f]{64}"$/RB_NODE_DARWIN_X64_SHA256="(placeholder)"/' "$lib"
run_status "$ROOT/scripts/tool-versions.sh" --check --root "$SCRATCH"
expect_status "a macOS Node pin that is no SHA-256 fails" 1
expect_output "and is named" "RB_NODE_DARWIN_X64_SHA256 is no SHA-256"
cp "$SCRATCH/tool-versions.sh.orig" "$lib"

run_status "$ROOT/scripts/tool-versions.sh"
expect_status "the listing runs" 0
expect_output "and names the macOS runtime" "node-darwin"

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

# The macOS archives (RB-097-02): one host's SHA-256 changed in the sidecar's copy.
sed -i -E "/^  'mac15-arm64'/s/sha256: '[0-9a-f]{64}'/sha256: '$(printf '1%.0s' {1..64})'/" "$pin_ts"
run_status "$ROOT/scripts/tool-versions.sh" --check --root "$SCRATCH"
expect_status "a macOS WebKit pin with another SHA-256 fails" 1
expect_output "the mismatch names the host's pin" "webkit-mac15-arm64-sha256"
cp "$SCRATCH/pin.ts.orig" "$pin_ts"

finish_tests tool-versions

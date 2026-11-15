#!/usr/bin/env bash
#
# scripts/release-assets.sh (RB-090-04): `sums` writes SHA256SUMS over a folder's files that
# `sha256sum -c` accepts and leaves itself and the Sigstore bundle out; `notes` prints a version's
# section of a CHANGELOG.md, a pre-release falls back to its release's section, and a version
# without a section fails.
#
#   scripts/tests/release-assets.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$ROOT/scripts/release-assets.sh"
# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

assets="$SCRATCH/assets"
mkdir -p "$assets"
echo zip > "$assets/rbrowser-1.1.0-windows-x64.zip"
echo plugin > "$assets/feature-example-1.0.0.rbplugin"
echo old > "$assets/SHA256SUMS.sigstore.json"

run_status "$SCRIPT" sums "$assets"
expect_status "sums writes the checksums" 0
expect "both packages are listed" "2" "$(wc -l < "$assets/SHA256SUMS" | tr -d ' ')"
expect "the bundle is not listed" "0" "$(grep -c sigstore "$assets/SHA256SUMS" || true)"
run_status bash -c "cd '$assets' && sha256sum --check --strict SHA256SUMS"
expect_status "sha256sum -c accepts the file" 0

run_status "$SCRIPT" sums "$SCRATCH/none"
expect_status "a folder that does not exist fails" 1

cat > "$SCRATCH/CHANGELOG.md" <<'EOF'
# Changelog

## [Unreleased]

## [1.1.0] - 2027-03-01

### Added

- The release (RB-110-03).

## [1.0.0] - 2027-01-10

### Fixed

- The old one.
EOF

run_status "$SCRIPT" notes 1.1.0 "$SCRATCH/CHANGELOG.md"
expect_status "notes of a version with a section" 0
expect_output "the section's entry is printed" "- The release (RB-110-03)."
expect "the next version's entry is not" "0" "$(grep -c 'The old one' <<< "$output" || true)"
expect "the heading itself is not" "0" "$(grep -c '^## \[1.1.0\]' <<< "$output" || true)"

run_status "$SCRIPT" notes 1.1.0-beta.2 "$SCRATCH/CHANGELOG.md"
expect_status "a pre-release falls back to its release's section" 0
expect_output "the release's entry is printed" "- The release (RB-110-03)."

run_status "$SCRIPT" notes 2.0.0 "$SCRATCH/CHANGELOG.md"
expect_status "a version without a section fails" 1
expect_output "the failure names the version" "no section ## [2.0.0]"

run_status "$SCRIPT" unknown
expect_status "an unknown command is a usage error" 2

finish_tests release-assets

#!/usr/bin/env bash
#
# scripts/release-assets.sh (RB-090-04): `sums` writes SHA256SUMS over a folder's files that
# `sha256sum -c` accepts and leaves itself and the Sigstore bundle out; `notes` prints a version's
# section of a CHANGELOG.md, a pre-release falls back to its release's section, and a version
# without a section fails; `complete` refuses a release folder that lacks one of the packages
# (Windows, and macOS per architecture as ZIP and DMG, RB-090-80); `downloads` names each
# package and how macOS lets an app that is not notarised start.
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

# --- complete: every package of the release, RB-090-80 ---
release="$SCRATCH/release"
mkdir -p "$release"
for name in windows-x64.zip macos-aarch64.zip macos-aarch64.dmg macos-x86_64.zip macos-x86_64.dmg \
    linux-x64.tar.gz linux-x64.deb linux-x64.rpm; do
    echo package > "$release/rbrowser-1.1.0-$name"
done
run_status "$SCRIPT" complete "$release" 1.1.0
expect_status "a release with every package is complete" 0
expect_output "the packages are counted" "8 package(s) of rBrowser 1.1.0"
rm "$release/rbrowser-1.1.0-macos-x86_64.dmg"
run_status "$SCRIPT" complete "$release" 1.1.0
expect_status "a release without a macOS DMG is refused" 1
expect_output "naming it" "rbrowser-1.1.0-macos-x86_64.dmg"
run_status "$SCRIPT" complete "$release" 1.2.0
expect_status "packages of another version do not count" 1
expect_output "naming the Windows ZIP of the version" "rbrowser-1.2.0-windows-x64.zip"
run_status "$SCRIPT" complete "$SCRATCH/none" 1.1.0
expect_status "a folder that does not exist fails" 1
run_status "$SCRIPT" complete "$release"
expect_status "complete without a version is a usage error" 2

# --- downloads: the release text's part on the packages ---
run_status "$SCRIPT" downloads 1.1.0
expect_status "downloads prints the packages" 0
expect_output "the Windows ZIP" "\`rbrowser-1.1.0-windows-x64.zip\`"
expect_output "Apple Silicon" "\`rbrowser-1.1.0-macos-aarch64.dmg\`"
expect_output "Intel" "\`rbrowser-1.1.0-macos-x86_64.dmg\`"
expect_output "how Gatekeeper lets the app start" "xattr -dr com.apple.quarantine /Applications/rBrowser.app"
expect_output "the Linux deb" "\`rbrowser-1.1.0-linux-x64.deb\`"
expect_output "the Linux tarball" "\`rbrowser-1.1.0-linux-x64.tar.gz\`"
expect_output "Open Anyway" "Open Anyway"
expect_output "the checksums" "sha256sum -c SHA256SUMS"
run_status "$SCRIPT" downloads
expect_status "downloads without a version is a usage error" 2

run_status "$SCRIPT" unknown
expect_status "an unknown command is a usage error" 2

finish_tests release-assets

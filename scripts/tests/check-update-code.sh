#!/usr/bin/env bash
#
# scripts/check-update-code.sh (RB-090-02): a binary built without the Cargo feature
# `self-update` carries no code of rb-update and not its update address. On fixture files: a
# clean binary passes; each marker - the crate's name as its symbols and module paths carry it,
# its source path in either separator as a panic location carries it, the release address of
# crates/rbrowser/src/updates/active.rs - fails the check, named with the file; so does a marker
# in the symbol list beside the binary (its PDB); --expect-present (the positive control of a
# build with the feature) turns the verdict round; a missing file and no file are usage errors.
# The markers are held to the sources they stand for, and the packager runs the check.
#
# Pure bash and python3, a second.
#
#   scripts/tests/check-update-code.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

CHECK="$ROOT/scripts/check-update-code.sh"

# A binary of `size` filler bytes with `text` in the middle, so a marker never sits at an edge.
binary() {
    local path="$1" text="${2:-}"
    {
        head -c 4096 /dev/zero
        printf 'MZ rb_config::user_settings crates/rbrowser/src/main.rs ureq/3.0 rBrowser/0.9.0 %s' "$text"
        head -c 4096 /dev/zero
    } > "$path"
}

mkdir -p "$SCRATCH/clean"
binary "$SCRATCH/clean/rbrowser.dll"
binary "$SCRATCH/clean/rbrowser.exe"
binary "$SCRATCH/clean/rbrowser.pdb" '_RNvNtCsj3HCowGrwMl_9rb_config13user_settings'
run_status "$CHECK" "$SCRATCH/clean/rbrowser.exe" "$SCRATCH/clean/rbrowser.dll"
expect_status "a build without rb-update passes" 0
expect_output "the pass names what was read" "no update code in 3 files"

markers=(
    rb_update '_RNvNtCs1a2b3c_9rb_update5fetch11user_agent'
    rb_update 'rb_update::gate'
    'crates/rb-update/' 'crates/rb-update/src/fetch.rs'
    "crates\\rb-update\\" "crates\\rb-update\\src\\download.rs"
    'releases/latest/download/' 'https://github.com/degoya/rBrowser/releases/latest/download/'
)
for ((i = 0; i < ${#markers[@]}; i += 2)); do
    marker="${markers[i]}" text="${markers[i + 1]}"
    dir="$SCRATCH/marked-$i"
    mkdir -p "$dir"
    binary "$dir/rbrowser.dll" "$text"
    run_status "$CHECK" "$dir/rbrowser.dll"
    expect_status "'$text' fails the check" 1
    expect_output "the failure names the marker of '$text'" "$dir/rbrowser.dll: '$marker'"
done

mkdir -p "$SCRATCH/symbols"
binary "$SCRATCH/symbols/rbrowser.dll"
binary "$SCRATCH/symbols/rbrowser.pdb" '_RNvNtCs1a2b3c_9rb_update4gate4open'
run_status "$CHECK" "$SCRATCH/symbols/rbrowser.dll"
expect_status "a symbol of rb-update in the PDB beside the binary fails the check" 1
expect_output "the failure names the PDB" "$SCRATCH/symbols/rbrowser.pdb: 'rb_update'"

mkdir -p "$SCRATCH/hyphen"
binary "$SCRATCH/hyphen/rbrowser-console.exe"
binary "$SCRATCH/hyphen/rbrowser_console.pdb" '_RNvNtCs1a2b3c_9rb_update4gate4open'
run_status "$CHECK" "$SCRATCH/hyphen/rbrowser-console.exe"
expect_status "the PDB of a hyphenated binary, named with an underscore, is read too" 1
expect_output "the failure names that PDB" "$SCRATCH/hyphen/rbrowser_console.pdb: 'rb_update'"

mkdir -p "$SCRATCH/count"
binary "$SCRATCH/count/rbrowser.dll" 'rb_update::a rb_update::b rb_update::c'
run_status "$CHECK" "$SCRATCH/count/rbrowser.dll"
expect_output "every occurrence is counted" "'rb_update' 3x"

run_status "$CHECK" --expect-present "$SCRATCH/marked-0/rbrowser.dll"
expect_status "--expect-present passes a build with rb-update" 0
expect_output "--expect-present names what it found" "'rb_update' 1x"
run_status "$CHECK" --expect-present "$SCRATCH/clean/rbrowser.dll"
expect_status "--expect-present fails a build without rb-update" 1

run_status "$CHECK" "$SCRATCH/clean/rbrowser.dll" "$SCRATCH/nowhere.dll"
expect_status "a missing file is a usage error" 2
expect_output "the missing file is named" "$SCRATCH/nowhere.dll"
run_status "$CHECK"
expect_status "no file is a usage error" 2
expect_output "no file prints the usage" "usage: scripts/check-update-code.sh"
run_status "$CHECK" --bogus "$SCRATCH/clean/rbrowser.dll"
expect_status "an unknown option is a usage error" 2

# The markers stand for real sources: the crate's name, its folder and the release address.
expect_true "the crate is named rb-update" \
    'grep -qx "name = \"rb-update\"" "$ROOT/crates/rb-update/Cargo.toml"'
expect_true "the release address is the one updates/active.rs fetches from" \
    'grep -qF "releases/latest/download/\";" "$ROOT/crates/rbrowser/src/updates/active.rs"'
expect_true "updates/active.rs is compiled only with the feature" \
    'grep -qx "#\[cfg(feature = \"self-update\")\]" "$ROOT/crates/rbrowser/src/updates/mod.rs"'
expect_true "the packager checks the staged binaries" \
    'grep -qF "scripts/check-update-code.sh" "$ROOT/scripts/package-windows.sh"'

finish_tests "check-update-code"

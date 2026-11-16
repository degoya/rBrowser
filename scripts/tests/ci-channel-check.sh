#!/usr/bin/env bash
#
# scripts/ci-channel-check.sh (RB-090-13) against a made-up program folder (RB_PROGRAM_DIR) with
# stand-ins for rbrowser and rbrowser.com that answer `--version`: an installation of the right
# version with its `scoop` marker passes; a wrong version, a missing twin or plugins folder, a
# missing or foreign marker for Scoop fail; winget without a marker passes with a notice; a wrong
# call is exit 2.
#
#   scripts/tests/ci-channel-check.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
DIR="$SCRATCH/app"
BIN="$SCRATCH/bin"

# install <version>: a program folder whose programs answer `rbrowser <version>`, the one on
# PATH a link to its rbrowser.com.
install() {
    rm -rf "$DIR" "$BIN"
    mkdir -p "$DIR/plugins" "$BIN"
    printf '#!/bin/sh\necho "rbrowser %s\r"\n' "$1" > "$DIR/rbrowser.com"
    cp "$DIR/rbrowser.com" "$DIR/rbrowser.exe"
    chmod +x "$DIR/rbrowser.com" "$DIR/rbrowser.exe"
    ln -s "$DIR/rbrowser.com" "$BIN/rbrowser"
}
check() { run_status env PATH="$BIN:$PATH" RB_PROGRAM_DIR="$DIR" "$ROOT/scripts/ci-channel-check.sh" "$@"; }

install 0.9.0
printf '# written by the Scoop manifest\nscoop\n' > "$DIR/install-kind"
check scoop v0.9.0
expect_status "a Scoop installation of the version with its marker" 0
expect_output "says so" "the scoop installation of 0.9.0 checks out"

check scoop v0.9.1
expect_status "another version on PATH fails" 1
expect_output "naming it" "rbrowser --version is not 0.9.1"

echo winget > "$DIR/install-kind"
check scoop 0.9.0
expect_status "a foreign marker fails" 1
expect_output "naming what the update check sees" "takes this installation for 'winget', not scoop"

rm "$DIR/install-kind"
check scoop 0.9.0
expect_status "no marker for Scoop fails" 1
expect_output "saying why" "would offer a self-update"

check winget 0.9.0
expect_status "winget without a marker passes" 0
expect_output "with a notice" "rb-update cannot tell this installation yet"

rm "$DIR/rbrowser.com"
ln -sf "$DIR/rbrowser.exe" "$BIN/rbrowser"
check winget 0.9.0
expect_status "a folder without the console twin fails" 1
expect_output "naming it" "no rbrowser.com in"

install 0.9.0
rmdir "$DIR/plugins"
check winget 0.9.0
expect_status "a folder without plugins fails" 1

install 0.9.0
printf '#!/bin/sh\necho "rbrowser 0.8.0"\n' > "$DIR/rbrowser.com"
rm "$BIN/rbrowser"
cp "$DIR/rbrowser.exe" "$BIN/rbrowser"
check winget 0.9.0
expect_status "a twin of another version fails" 1
expect_output "naming it" "rbrowser.com --version is not 0.9.0"

check chocolatey 0.9.0
expect_status "an unknown channel is a wrong call" 2
check scoop 0.9
expect_status "a version that is none is a wrong call" 2
check scoop
expect_status "a missing version is a wrong call" 2

finish_tests ci-channel-check

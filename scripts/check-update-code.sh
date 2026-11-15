#!/usr/bin/env bash
#
# Proves that a binary built without the Cargo feature `self-update` carries no update code
# (RB-090-02): neither rb-update - its symbols, module paths and source paths - nor the release
# address of crates/rbrowser/src/updates/active.rs, in the binary's strings or in the symbol list
# of the PDB beside it. scripts/package-windows.sh runs it on the staged binaries, which are
# never built with the feature; the markers and why: scripts/lib/update_code_scan.py.
#
# Usage:
#   scripts/check-update-code.sh <binary>...                   # none may carry update code
#   scripts/check-update-code.sh --expect-present <binary>...  # the positive control: a build
#                                                              # with --features self-update
#
# Exit 0 when the expectation holds (each hit is named with its file and count), 1 when not, 2 on
# a usage error or a missing file. Compiles nothing.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec python3 -I "$ROOT/scripts/lib/update_code_scan.py" "$@"

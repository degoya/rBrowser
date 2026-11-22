#!/usr/bin/env bash
#
# The read rule of the settings (RB-094-30): every core settings key is read by some code - a key
# in the schema that nothing reads is a switch without effect. The rule and its limits:
# scripts/lib/settings_unread.py. Compiles nothing; scripts/check.sh --preflight runs it.
#
# Usage:
#   scripts/check-settings-read.sh            # this checkout
#   scripts/check-settings-read.sh <root>     # another tree (the tests' fixtures)
#
# Exit 0 clean, 1 with unread keys (each named), 2 when the data does not read.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
[[ $# -le 1 ]] || { echo "usage: scripts/check-settings-read.sh [<root>]" >&2; exit 2; }
exec python3 -I "$ROOT/scripts/lib/settings_unread.py" "${1:-$ROOT}"

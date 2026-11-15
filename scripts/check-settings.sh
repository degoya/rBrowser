#!/usr/bin/env bash
#
# The settings lint (RB-010-02): every key of the settings schemas - core (schema/*.json,
# schema/parts/*.json) and the bundled plugins' - carries x-category, x-section, x-order and
# x-scope with a known category and section, a unique order in its section and its i18n texts;
# no plugin adds a top-level category. The rules: docs/settings.md; the code:
# scripts/lib/settings_lint.py. Compiles nothing; scripts/check.sh --preflight runs it.
#
# Usage:
#   scripts/check-settings.sh            # lint this checkout
#   scripts/check-settings.sh <root>     # lint another tree (the tests' fixtures)
#
# Exit 0 clean, 1 with problems (each names its file and key), 2 when the data does not read.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
[[ $# -le 1 ]] || { echo "usage: scripts/check-settings.sh [<root>]" >&2; exit 2; }
exec python3 -I "$ROOT/scripts/lib/settings_lint.py" "${1:-$ROOT}"

#!/usr/bin/env bash
#
# The i18n lint (RB-010-14): every key a source names exists in locales/en.json (with the wave
# parts not merged yet), no key of the catalogue is unused, every catalogue is well-formed and
# en.json canonical, and no .vue template writes visible text raw. The rules are
# scripts/lib/check_i18n.py; check.sh --preflight runs this. Compiles nothing.
#
#   scripts/check-i18n.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec python3 "$ROOT/scripts/lib/check_i18n.py" "$ROOT"

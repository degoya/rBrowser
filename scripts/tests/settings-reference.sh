#!/usr/bin/env bash
#
# scripts/settings-reference.sh (scripts/lib/settings_reference.py) against the settings fixture
# (lib/settings-fixture.sh) and a temp wiki: it writes reference/settings.md with every key under
# its category and section, its label, default, scopes and notes; --check then passes, finds the
# page stale after a schema change, and passes without a page or a wiki (nothing generated yet).
# The repository's own data reads.
#
# Python only, compiles nothing: check.sh --preflight runs it.
#
#   scripts/tests/settings-reference.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
GENERATOR="$ROOT/scripts/settings-reference.sh"

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
# shellcheck source=lib/settings-fixture.sh
source "$ROOT/scripts/tests/lib/settings-fixture.sh"

fixture="$(settings_fixture "$SCRATCH/repo")"
wiki="$SCRATCH/wiki"
# shellcheck disable=SC2034  # read by the conditions expect_true evaluates
page="$wiki/reference/settings.md"
mkdir -p "$wiki"

run_status env RB_WIKI="$SCRATCH/none" "$GENERATOR" --check "$ROOT"
expect_status "the repository's data reads (no wiki: nothing to compare)" 0

run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "a wiki without the page is not stale yet" 0
expect_output "the missing page is said" "not generated yet"

run_status env RB_WIKI="$wiki" "$GENERATOR" "$fixture"
expect_status "the page is written" 0
expect_true "the page exists" '[[ -f "$page" ]]'
expect_true "the categories are headings, in order" \
    'grep -n "^## " "$page" | head -n 1 | grep -qF "## General" && grep -qF "## Breakpoints & Devices" "$page"'
expect_true "a section is a heading below its category" 'grep -qxF "### Scroll" "$page"'
expect_true "a key is a row with its label and default" \
    'grep -F "| \`sync.scroll\` |" "$page" | grep -qF "Sync scrolling" && grep -F "| \`sync.scroll\` |" "$page" | grep -qF "\`true\`"'
expect_true "a local-only key says so" \
    'grep -F "| \`network.ignoreCertificateErrors\` |" "$page" | grep -qF "local only"'
expect_true "a plugin key names its plugin" \
    'grep -F "| \`seo.titleLength\` |" "$page" | grep -qF "plugin \`seo\`"'
expect_true "the deep link of a key is given" 'grep -qF "rbrowser://settings/sync.scroll" "$page"'

run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "--check passes right after writing" 0

python3 -I - "$fixture/schema/config.v1.json" << 'EOF'
import json, sys
path = sys.argv[1]
doc = json.load(open(path, encoding="utf-8"))
doc["$defs"]["Sync"]["properties"]["scroll"]["default"] = False
json.dump(doc, open(path, "w", encoding="utf-8"), indent=2)
EOF
run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "--check finds the page stale after a schema change" 1
expect_output "it names the generator" "scripts/settings-reference.sh"

run_status env RB_WIKI="$wiki" "$GENERATOR" "$fixture"
run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "regenerating makes it current again" 0

printf '{ "type": ' > "$fixture/schema/config.v1.json"
run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "a schema with lint problems is refused" 2

finish_tests settings-reference

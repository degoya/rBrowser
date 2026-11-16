#!/usr/bin/env bash
#
# scripts/keyboard-reference.sh (scripts/lib/keyboard_reference.py) against a small fixture (a
# commands file with three commands in two groups, a reservation list with two entries, their
# English texts, the bundled plugins of lib/plugin-fixture.sh) and a temp wiki: it writes
# reference/keyboard-shortcuts.md with every group as a heading, every command (a plugin's
# marked) as a row with its title, default keys and MCP tool, then the
# reserved combinations with their reasons; --check then passes, finds the page stale after the
# commands change, and passes without a page or a wiki (nothing generated yet). The repository's
# own data reads.
#
# Python only, compiles nothing: check.sh --preflight runs it.
#
#   scripts/tests/keyboard-reference.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
GENERATOR="$ROOT/scripts/keyboard-reference.sh"

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
# shellcheck source=lib/plugin-fixture.sh
source "$ROOT/scripts/tests/lib/plugin-fixture.sh"

fixture="$SCRATCH/repo"
mkdir -p "$fixture/schema" "$fixture/crates/rb-commands/data" "$fixture/locales"
cat > "$fixture/schema/commands.v1.json" << 'EOF'
{
  "version": 1,
  "leader": "Ctrl+Shift+Space",
  "groups": [
    { "id": "breakpoints", "key": "B", "titleKey": "commands.group.breakpoints" },
    { "id": "view", "key": "V", "titleKey": "commands.group.view" },
    { "id": "plugins", "key": "X", "titleKey": "commands.group.plugins" },
    { "id": "settings", "key": ",", "titleKey": "commands.group.settings" }
  ],
  "commands": [
    { "id": "view.back", "titleKey": "commands.view.back", "group": "view",
      "defaultBindings": ["Ctrl+Shift+Space V B"], "mcp": { "covered": "back" } },
    { "id": "view.focus_address_bar", "titleKey": "commands.view.focus_address_bar",
      "group": "view", "defaultBindings": ["Ctrl+Shift+Space V A"], "mcp": { "omitted": "ui-only" } },
    { "id": "settings.open", "titleKey": "commands.settings.open", "group": "settings",
      "defaultBindings": ["Ctrl+Shift+Space , ,"], "mcp": { "omitted": "ui-only" } }
  ]
}
EOF
cat > "$fixture/crates/rb-commands/data/reserved-shortcuts.json" << 'EOF'
{
  "version": 1,
  "entries": [
    { "combo": "Ctrl+T", "platforms": ["windows"], "source": "browser",
      "reasonKey": "shortcuts.reserved.browser_new_tab", "reason": "Browser: new tab" },
    { "combo": "Ctrl+Alt+<any>", "platforms": ["windows"], "source": "altgr",
      "reasonKey": "shortcuts.reserved.altgr", "reason": "AltGr" }
  ]
}
EOF
cat > "$fixture/locales/en.json" << 'EOF'
{
  "commands": {
    "group": { "breakpoints": "Breakpoints", "view": "View", "plugins": "Plugins", "settings": "Settings" },
    "view": { "back": "Back", "focus_address_bar": "Focus the address bar" },
    "settings": { "open": "Open settings" }
  },
  "shortcuts": {
    "reserved": { "altgr": "AltGr on German, French and Spanish keyboards" }
  }
}
EOF

plugin_fixture "$fixture"

wiki="$SCRATCH/wiki"
# shellcheck disable=SC2034  # read by the conditions expect_true evaluates
page="$wiki/reference/keyboard-shortcuts.md"
mkdir -p "$wiki"

run_status env RB_WIKI="$SCRATCH/none" "$GENERATOR" --check "$ROOT"
expect_status "the repository's data reads (no wiki: nothing to compare)" 0

run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "a wiki without the page is not stale yet" 0
expect_output "the missing page is said" "not generated yet"

run_status env RB_WIKI="$wiki" "$GENERATOR" "$fixture"
expect_status "the page is written" 0
expect_true "the page exists" '[[ -f "$page" ]]'
expect_true "the groups are headings with their leader key, in order" \
    'grep "^## " "$page" | head -n 2 | tr "\n" "|" | grep -qF "## Breakpoints (\`⌘ B\`)|## View (\`⌘ V\`)|"'
expect_true "a group without commands says so" 'grep -qxF "No commands yet." "$page"'
expect_true "a command is a row with its title, id, keys and MCP tool" \
    'grep -qxF "| Back | \`view.back\` | \`⌘ V B\` | \`back\` |" "$page"'
expect_true "an omitted command gives the reason" \
    'grep -F "\`view.focus_address_bar\`" "$page" | grep -qF "none (ui-only)"'
expect_true "a bundled plugin's command is a row in its group, marked, with its prefixed tool" \
    'grep -qxF "| Run the sample *(plugin: Sample Tool)* | \`plugin.sample_tool.run\` | \`⌘ V X\` | \`samp_run\` |" "$page"'
expect_true "a plugin command without a group is in the plugins group, its omission given" \
    'sed -n "/^## Plugins/,/^## Settings/p" "$page" | grep -qxF "| Open the sample panel *(plugin: Sample Tool)* | \`plugin.sample_tool.panel\` | \`⌘ X S\` | none (ui-only: a panel) |"'
expect_true "a TEST-ONLY plugin is not listed" '! grep -qF "test_only" "$page"'
expect_true "the settings group's comma key reads" 'grep -qF "\`⌘ , ,\`" "$page"'
expect_true "the reserved rules are linked" \
    'grep -qF "(reference/shortcuts#keys-rbrowser-never-takes)" "$page"'
expect_true "a reserved combination is a row with its reason" \
    'grep -qxF "| \`Ctrl+T\` | Browser: new tab |" "$page"'
expect_true "a reason takes the English text of its key" \
    'grep -F "\`Ctrl+Alt+<any>\`" "$page" | grep -qF "AltGr on German, French and Spanish keyboards"'

run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "--check passes right after writing" 0

sed -i 's/Ctrl+Shift+Space V B/Ctrl+Shift+Space V J/' "$fixture/schema/commands.v1.json"
run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "--check finds the page stale after a binding changes" 1
expect_output "it names the generator" "scripts/keyboard-reference.sh"

run_status env RB_WIKI="$wiki" "$GENERATOR" "$fixture"
run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "regenerating makes it current again" 0

sed -i 's/"group": "settings"/"group": "nowhere"/' "$fixture/schema/commands.v1.json"
run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "a command in an unknown group is refused" 2
expect_output "the refusal names how the file is regenerated" "RB_UPDATE_SCHEMA=1"

printf '{ "version": ' > "$fixture/schema/commands.v1.json"
run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "a commands file that is not JSON is refused" 2

finish_tests keyboard-reference

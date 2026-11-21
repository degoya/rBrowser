#!/usr/bin/env bash
#
# scripts/mcp-reference.sh (scripts/lib/mcp_reference.py) against a small fixture (a tools file
# with three tools in two areas and one pending tool, a commands file that runs two of them, waits
# on the pending one and omits one, their English texts, the bundled plugins of
# lib/plugin-fixture.sh) and a temp wiki: it writes reference/mcp-tools.md with an overview row
# per tool, an area heading, a section per tool with its target, command, guard and parameter
# table, a section per plugin with its tools (marked; a core tool's name and a TEST-ONLY plugin
# left out), the prompts with their arguments and the resources with their URIs (RB-095-01), the
# coming tools and the commands without a tool; --check then passes, finds the page stale after a
# tool is added, and passes without a page or a wiki (nothing generated yet); a tools file without
# prompts and resources has neither section. The repository's own data reads.
#
# Python only, compiles nothing: check.sh --preflight runs it.
#
#   scripts/tests/mcp-reference.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
GENERATOR="$ROOT/scripts/mcp-reference.sh"

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
# shellcheck source=lib/plugin-fixture.sh
source "$ROOT/scripts/tests/lib/plugin-fixture.sh"

fixture="$SCRATCH/repo"
mkdir -p "$fixture/schema" "$fixture/locales"
cat > "$fixture/schema/mcp-tools.v1.json" << 'EOF'
{
  "version": 1,
  "tools": [
    { "name": "devtools_open", "area": "app", "target": "one",
      "description": "Open DevTools for one view.",
      "inputSchema": { "type": "object", "properties": {
        "target": { "description": "One view.",
          "anyOf": [{ "$ref": "#/$defs/TargetArg" }, { "type": "null" }] } },
        "$defs": { "TargetArg": { "anyOf": [{ "type": "integer" }, { "type": "string" }] } } } },
    { "name": "navigate", "area": "views", "target": "all",
      "description": "Load a URL in the targeted views.",
      "inputSchema": { "type": "object", "required": ["url"], "properties": {
        "url": { "type": "string", "description": "The URL to load." },
        "mode": { "type": ["string", "null"], "enum": ["all", "edges", null] },
        "urls": { "type": "array", "items": { "type": "string" }, "default": [] } } } },
    { "name": "settings_set", "area": "project", "target": "none", "actsOn": "the settings",
      "description": "Change a setting as a person would.",
      "guard": { "note": "Protected keys are refused." },
      "withoutCommand": "writes one setting as the settings window does",
      "inputSchema": { "type": "object", "properties": {} } }
  ],
  "pending": [ { "tool": "click", "job": "RB-020-03" } ],
  "prompts": [
    { "name": "bug_repro", "description": "Reproduce a bug in every view.",
      "arguments": [ { "name": "url", "description": "The page.", "required": false } ] }
  ],
  "resources": [
    { "uri": "rbrowser://views", "name": "views", "mimeType": "application/json",
      "description": "The views of the grid." }
  ],
  "resourceTemplates": [
    { "uriTemplate": "rbrowser://screenshots/latest/{index}", "name": "screenshot",
      "mimeType": "image/png", "description": "One PNG." }
  ]
}
EOF
cat > "$fixture/schema/commands.v1.json" << 'EOF'
{
  "version": 1,
  "commands": [
    { "id": "view.navigate", "titleKey": "commands.view.navigate", "mcp": { "covered": "navigate" } },
    { "id": "devtools.open", "titleKey": "commands.devtools.open", "mcp": { "covered": "devtools_open" } },
    { "id": "view.click", "titleKey": "commands.view.click", "mcp": { "covered": "click" } },
    { "id": "view.zoom_in", "titleKey": "commands.view.zoom_in", "mcp": { "omitted": "ui-only: grid zoom" } }
  ]
}
EOF
cat > "$fixture/locales/en.json" << 'EOF'
{
  "commands": {
    "view": { "navigate": "Navigate", "zoom_in": "Zoom in" },
    "devtools": { "open": "Open DevTools" }
  }
}
EOF

plugin_fixture "$fixture"

wiki="$SCRATCH/wiki"
# shellcheck disable=SC2034  # read by the conditions expect_true evaluates
page="$wiki/reference/mcp-tools.md"
mkdir -p "$wiki"

run_status env RB_WIKI="$SCRATCH/none" "$GENERATOR" --check "$ROOT"
expect_status "the repository's data reads (no wiki: nothing to compare)" 0

run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "a wiki without the page is not stale yet" 0
expect_output "the missing page is said" "not generated yet"

run_status env RB_WIKI="$wiki" "$GENERATOR" "$fixture"
expect_status "the page is written" 0
expect_true "the page exists" '[[ -f "$page" ]]'
expect_true "the overview has a row per tool with area, target, command and guard" \
    'grep -qxF "| [\`navigate\`](#navigate) | Views and navigation | all | \`view.navigate\` | - |" "$page"'
expect_true "a guarded tool without a command says so in the overview" \
    'grep -qxF "| [\`settings_set\`](#settings_set) | Project and settings | - | - | yes |" "$page"'
expect_true "the areas are headings in the page's order, the plugins after the core's" \
    'grep "^## " "$page" | tr "\n" "|" | grep -qF "## Views and navigation|## Project and settings|## App|## Plugin: Sample Tool|## Prompts|## Resources|## Coming tools|## Commands without a tool|"'
expect_true "a tool is a section of its own" 'grep -qxF "### \`navigate\`" "$page"'
expect_true "a view tool takes all" \
    'grep -qxF -- "- **Target:** \`\"all\"\` (default), a view id or a breakpoint id" "$page"'
expect_true "a single-view tool says one view" 'grep -qF -- "- **Target:** one view" "$page"'
expect_true "an untargeted tool says what it acts on" \
    'grep -qxF -- "- **Target:** none - acts on the settings" "$page"'
expect_true "the command is named with its title" \
    'grep -qxF -- "- **Command:** \`view.navigate\` (Navigate)" "$page"'
expect_true "a tool without a command gives the reason" \
    'grep -qxF -- "- **Command:** none - writes one setting as the settings window does" "$page"'
expect_true "the guard is given" 'grep -qxF -- "- **Guarded:** Protected keys are refused." "$page"'
expect_true "a required parameter is a row with its type and description" \
    'grep -qxF "| \`url\` | string | yes | The URL to load. |" "$page"'
expect_true "an enum lists its values, null left out" \
    'grep -qxF "| \`mode\` | \`\"all\"\` or \`\"edges\"\` | no | - |" "$page"'
expect_true "an array names its items and its default" \
    'grep -qxF "| \`urls\` | array of string | no | Default: \`[]\`. |" "$page"'
expect_true "a target reference resolves to its types" \
    'grep -qxF "| \`target\` | integer or string | no | One view. |" "$page"'
expect_true "a tool without parameters says so" 'grep -qxF "No parameters." "$page"'
expect_true "a coming tool names its job and command" \
    'grep -qxF "| \`click\` | RB-020-03 | \`view.click\` |" "$page"'
expect_true "an omitted command gives the reason" \
    'grep -qxF "| \`view.zoom_in\` (Zoom in) | ui-only: grid zoom |" "$page"'
expect_true "a plugin tool has an overview row, marked, with the command that runs it" \
    'grep -qxF "| [\`samp_run\`](#samp_run) | Plugin: Sample Tool | all | \`plugin.sample_tool.run\` | - |" "$page"'
expect_true "a plugin tool's description comes from the plugin's locales" \
    'grep -qxF "Runs the sample in the views of the target. *(plugin: Sample Tool)*" "$page"'
expect_true "a plugin tool takes target like every view tool" \
    'grep -qF "| \`target\` | string or integer | no |" "$page"'
expect_true "a plugin tool's parameters come from its manifest" \
    'grep -qxF "| \`depth\` | integer | yes | - |" "$page"'
expect_true "a plugin tool running the caller's script is guarded" \
    'grep -qF -- "- **Guarded:** setting \`mcp.allowEvaluate\`." "$page"'
expect_true "a plugin tool with a core tool's name is left out" \
    '[[ "$(grep -cxF "### \`settings_set\`" "$page")" == 1 ]] && ! grep -qF "Core Twin" "$page"'
expect_true "a TEST-ONLY plugin is not listed" '! grep -qF "testonly_hidden" "$page"'
expect_true "a prompt is a section with its description" \
    'grep -qxF "### \`bug_repro\`" "$page" && grep -qxF "Reproduce a bug in every view." "$page"'
expect_true "a prompt's argument is a row" 'grep -qxF "| \`url\` | no | The page. |" "$page"'
expect_true "a resource is a row with its type" \
    'grep -qxF "| \`rbrowser://views\` | application/json | The views of the grid. |" "$page"'
expect_true "a resource template is a row too" \
    'grep -qxF "| \`rbrowser://screenshots/latest/{index}\` | image/png | One PNG. |" "$page"'
expect_true "a plugin's omitted command gives the reason, marked" \
    'grep -qxF "| \`plugin.sample_tool.panel\` (Open the sample panel) *(plugin: Sample Tool)* | ui-only: a panel |" "$page"'

run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "--check passes right after writing" 0

python3 - "$fixture/schema/mcp-tools.v1.json" << 'EOF'
import json, sys
path = sys.argv[1]
data = json.load(open(path))
data["tools"].append({"name": "reload", "area": "views", "target": "all",
                      "description": "Reload the targeted views.",
                      "inputSchema": {"type": "object", "properties": {}}})
json.dump(data, open(path, "w"))
EOF
run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "--check finds the page stale after a tool is added" 1
expect_output "it names the generator" "scripts/mcp-reference.sh"

run_status env RB_WIKI="$wiki" "$GENERATOR" "$fixture"
run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "regenerating makes it current again" 0

python3 - "$fixture/schema/mcp-tools.v1.json" << 'EOF'
import json, sys
path = sys.argv[1]
data = json.load(open(path))
for key in ("prompts", "resources", "resourceTemplates"):
    del data[key]
json.dump(data, open(path, "w"))
EOF
run_status env RB_WIKI="$wiki" "$GENERATOR" "$fixture"
expect_status "a tools file without prompts and resources is written" 0
expect_true "it has neither section" '! grep -qE "^## (Prompts|Resources)$" "$page"'

sed -i 's/"target": "one"/"target": "some"/' "$fixture/schema/mcp-tools.v1.json"
run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "a tool with an unknown target is refused" 2
expect_output "the refusal names how the file is regenerated" "RB_UPDATE_SCHEMA=1"

printf '{ "version": ' > "$fixture/schema/mcp-tools.v1.json"
run_status env RB_WIKI="$wiki" "$GENERATOR" --check "$fixture"
expect_status "a tools file that is not JSON is refused" 2

finish_tests mcp-reference

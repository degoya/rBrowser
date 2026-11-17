#!/usr/bin/env bash
#
# scripts/check-settings.sh (scripts/lib/settings_lint.py) against small repositories in a temp
# directory: a fixture whose schema, plugin and catalogue follow every rule passes, and each
# broken rule fails with the key named - a key without x-category, an unknown section, a repeated
# x-order in a section, a local-only key that allows Project, a missing i18n key (label or help
# text), a path key without x-input or with a wrong one, and a plugin schema that adds a
# top-level category. The repository itself passes too.
#
# Python only, compiles nothing: check.sh --preflight runs it.
#
#   scripts/tests/settings-lint.sh
# shellcheck disable=SC2016  # the Python edits are single-quoted on purpose
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
LINT="$ROOT/scripts/check-settings.sh"

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
# shellcheck source=lib/settings-fixture.sh
source "$ROOT/scripts/tests/lib/settings-fixture.sh"

# Edits the JSON file $1 of the fixture in place with the Python statement $2 on `doc`.
edit_json() {
    python3 -I - "$1" "$2" << 'EOF'
import json, sys
path, statement = sys.argv[1], sys.argv[2]
with open(path, encoding="utf-8") as handle:
    doc = json.load(handle)
exec(statement)
with open(path, "w", encoding="utf-8") as handle:
    json.dump(doc, handle, indent=2)
EOF
}

run_status "$LINT" "$ROOT"
expect_status "the repository passes" 0

fixture="$(settings_fixture "$SCRATCH/good")"
run_status "$LINT" "$fixture"
expect_status "a fixture that follows every rule passes" 0
expect_output "the summary counts the keys" "6 keys (5 core, 1 from 1 plugin)"

fixture="$(settings_fixture "$SCRATCH/no-category")"
edit_json "$fixture/schema/config.v1.json" \
    'doc["$defs"]["Sync"]["properties"]["hover"] = {"type": "boolean"}'
run_status "$LINT" "$fixture"
expect_status "a key without x-category fails" 1
expect_output "the key is named" "sync.hover: missing x-category"

fixture="$(settings_fixture "$SCRATCH/section")"
edit_json "$fixture/schema/config.v1.json" \
    'doc["$defs"]["Sync"]["properties"]["scroll"]["x-section"] = "media-features"'
run_status "$LINT" "$fixture"
expect_status "an unknown section fails" 1
expect_output "the section is named" "sync.scroll: unknown section \`media-features\` in category \`sync\`"

fixture="$(settings_fixture "$SCRATCH/order")"
edit_json "$fixture/schema/config.v1.json" \
    'doc["$defs"]["Sync"]["properties"]["navigation"]["x-section"] = "scroll"'
run_status "$LINT" "$fixture"
expect_status "a repeated x-order in a section fails" 1
expect_output "both keys are named" "sync.navigation: x-order 1 repeats \`sync.scroll\` in section sync/scroll"

fixture="$(settings_fixture "$SCRATCH/local")"
edit_json "$fixture/schema/config.v1.json" \
    'doc["$defs"]["Network"]["properties"]["ignoreCertificateErrors"]["x-scope"] = ["project", "local"]'
run_status "$LINT" "$fixture"
expect_status "a local-only key that allows Project fails" 1
expect_output "the rule is named" "network.ignoreCertificateErrors: a local-only key has x-scope [\"local\"]"

fixture="$(settings_fixture "$SCRATCH/i18n")"
edit_json "$fixture/locales/en.json" 'del doc["settings"]["key"]["sync"]["scroll"]["label"]'
run_status "$LINT" "$fixture"
expect_status "a missing label fails" 1
expect_output "the i18n key is named" "sync.scroll: missing i18n key settings.key.sync.scroll.label"

fixture="$(settings_fixture "$SCRATCH/help")"
edit_json "$fixture/locales/en.json" 'del doc["settings"]["key"]["sync"]["scroll"]["help"]'
run_status "$LINT" "$fixture"
expect_status "a key without a help text fails (RB-080-20)" 1
expect_output "the help key is named" "sync.scroll: missing i18n key settings.key.sync.scroll.help"

fixture="$(settings_fixture "$SCRATCH/path-input")"
edit_json "$fixture/schema/config.v1.json" \
    'doc["properties"]["cacheDir"] = {"type": "string", "default": "", "x-category": "project", "x-section": "start-url", "x-order": 2, "x-scope": ["project", "local"]}'
run_status "$LINT" "$fixture"
expect_status "a path key without x-input fails" 1
expect_output "the rule is named" 'cacheDir: a path key needs x-input ("file" or "folder")'

fixture="$(settings_fixture "$SCRATCH/bad-input")"
edit_json "$fixture/schema/config.v1.json" 'doc["properties"]["startUrl"]["x-input"] = "url"'
run_status "$LINT" "$fixture"
expect_status "an x-input that is neither file nor folder fails" 1
expect_output "the value is refused" 'startUrl: x-input is not "file" or "folder"'

fixture="$(settings_fixture "$SCRATCH/input-type")"
edit_json "$fixture/schema/config.v1.json" \
    'doc["$defs"]["Sync"]["properties"]["scroll"]["x-input"] = "file"'
run_status "$LINT" "$fixture"
expect_status "an x-input on a boolean fails" 1
expect_output "the key is named" "sync.scroll: x-input sits on a key that is no string"

fixture="$(settings_fixture "$SCRATCH/input-ok")"
edit_json "$fixture/schema/config.v1.json" \
    'doc["properties"]["startUrl"]["x-input"] = "file"; doc["properties"]["startUrl"]["x-options"] = ["a", "b"]'
run_status "$LINT" "$fixture"
expect_status "a file input with suggestions on a string passes" 0

fixture="$(settings_fixture "$SCRATCH/options")"
edit_json "$fixture/schema/config.v1.json" 'doc["properties"]["startUrl"]["x-options"] = "a"'
run_status "$LINT" "$fixture"
expect_status "x-options that is no list fails" 1
expect_output "the rule is named" "startUrl: x-options is not a list of strings"

# A source the core fills at runtime (RB-080-72) names known values of a list or a map's keys.
fixture="$(settings_fixture "$SCRATCH/options-source")"
edit_json "$fixture/schema/config.v1.json" \
    'doc["properties"]["startUrl"].update({"type": "array", "items": {"type": "string"}, "default": [], "x-options": "plugins"})'
run_status "$LINT" "$fixture"
expect_status "a source on a list passes" 0

fixture="$(settings_fixture "$SCRATCH/options-source-string")"
edit_json "$fixture/schema/config.v1.json" 'doc["properties"]["startUrl"]["x-options"] = "plugins"'
run_status "$LINT" "$fixture"
expect_status "a source on a string fails" 1
expect_output "the place is named" 'startUrl: x-options "plugins" sits on a key that is no list or map'

fixture="$(settings_fixture "$SCRATCH/options-source-unknown")"
edit_json "$fixture/schema/config.v1.json" \
    'doc["properties"]["startUrl"].update({"type": "array", "items": {"type": "string"}, "default": [], "x-options": "breakpoint-ids"})'
run_status "$LINT" "$fixture"
expect_status "an unknown source fails" 1
expect_output "the value is refused" "startUrl: x-options is not a list of strings"

fixture="$(settings_fixture "$SCRATCH/section-name")"
edit_json "$fixture/locales/parts/titles.json" 'del doc["settings"]["section"]["sync"]["scroll"]'
run_status "$LINT" "$fixture"
expect_status "a missing section title fails" 1
expect_output "the title key is named" "missing i18n key settings.section.sync.scroll"

fixture="$(settings_fixture "$SCRATCH/plugin")"
edit_json "$fixture/plugins/seo/settings.schema.json" \
    'doc["properties"]["titleLength"]["x-category"] = "seo-tools"'
run_status "$LINT" "$fixture"
expect_status "a plugin schema with a new top-level category fails" 1
expect_output "the plugin key is named" \
    "seo.titleLength: unknown category \`seo-tools\` (a plugin cannot add a top-level category)"

fixture="$(settings_fixture "$SCRATCH/plugin-label")"
edit_json "$fixture/plugins/seo/locales/en.json" 'doc["plugin"]["seo"]["settings"] = {}'
run_status "$LINT" "$fixture"
expect_status "a plugin key without its label fails" 1
expect_output "the plugin's i18n key is named" "missing i18n key plugin.seo.settings.titleLength.label"

fixture="$(settings_fixture "$SCRATCH/broken")"
printf '{ "type": ' > "$fixture/schema/config.v1.json"
run_status "$LINT" "$fixture"
expect_status "a schema that is not JSON fails" 1
expect_output "the file is named" "schema/config.v1.json: not JSON"

# RB-090-16: every option of a choice has a name in the catalogue - a core key's at
# settings.option.<key>.<value>, a plugin key's at plugin.<id>.settings.option.<path>.<value>,
# the value's first letter in lower case; a nested field adds its name, a list's items and a
# map's values add nothing; a number names itself.
CHOICE_KEY='doc["properties"]["paper"] = {"type": "string", "enum": ["A4", "letter"], "default": "A4", "x-category": "project", "x-section": "start-url", "x-order": 2, "x-scope": ["project", "local"]}; doc["properties"]["delay"] = {"type": "integer", "enum": [0, 5], "default": 0, "x-category": "project", "x-section": "start-url", "x-order": 3, "x-scope": ["project", "local"]}; doc["properties"]["devices"] = {"type": "array", "default": [], "items": {"type": "object", "properties": {"kind": {"anyOf": [{"type": "string", "enum": ["mobile"]}, {"type": "null"}]}}}, "x-category": "project", "x-section": "start-url", "x-order": 4, "x-scope": ["project", "local"]}'
CHOICE_TEXTS='keys = doc["settings"]["key"]
for name in ("paper", "delay", "devices"):
    keys[name] = {"label": name, "description": name, "help": name}'
fixture="$(settings_fixture "$SCRATCH/option-missing")"
edit_json "$fixture/schema/config.v1.json" "$CHOICE_KEY"
edit_json "$fixture/locales/en.json" "$CHOICE_TEXTS"
run_status "$LINT" "$fixture"
expect_status "a choice without names fails" 1
expect_output "the option's i18n key is named" "paper: missing i18n key settings.option.paper.a4 (option \"A4\")"
expect_output "every option is named" "paper: missing i18n key settings.option.paper.letter (option \"letter\")"
expect_output "a nested field's option names its field" \
    "devices: missing i18n key settings.option.devices.kind.mobile (option \"mobile\")"
expect_true "a number option needs no name" '[[ "$output" != *"settings.option.delay"* ]]'

fixture="$(settings_fixture "$SCRATCH/option-named")"
edit_json "$fixture/schema/config.v1.json" "$CHOICE_KEY"
edit_json "$fixture/locales/en.json" "$CHOICE_TEXTS"'
doc["settings"]["option"] = {"paper": {"a4": "A4", "letter": "Letter"}, "devices": {"kind": {"mobile": "Phone"}}}'
run_status "$LINT" "$fixture"
expect_status "a choice with a name for every option passes" 0

fixture="$(settings_fixture "$SCRATCH/plugin-option")"
edit_json "$fixture/plugins/seo/settings.schema.json" \
    'doc["properties"]["platforms"] = {"type": "array", "default": [], "items": {"type": "string", "enum": ["teams", "x"]}, "x-category": "checks", "x-section": "seo", "x-order": 2, "x-scope": ["project", "local"]}'
edit_json "$fixture/plugins/seo/locales/en.json" '
seo = doc["plugin"]["seo"]["settings"]
seo["platforms"] = {"label": "Platforms", "description": "Platforms", "help": "Platforms"}
seo["option"] = {"platforms": {"x": "X"}}'
run_status "$LINT" "$fixture"
expect_status "a plugin choice without a name fails" 1
expect_output "the plugin's option key is named" \
    "seo.platforms: missing i18n key plugin.seo.settings.option.platforms.teams (option \"teams\")"
expect_true "a named plugin option passes" '[[ "$output" != *"platforms.x "* ]]'

fixture="$(settings_fixture "$SCRATCH/plugin-option-reserved")"
edit_json "$fixture/plugins/seo/settings.schema.json" \
    'doc["properties"]["option"] = {"type": "boolean", "default": True, "x-category": "checks", "x-section": "seo", "x-order": 2, "x-scope": ["project", "local"]}'
edit_json "$fixture/plugins/seo/locales/en.json" \
    'doc["plugin"]["seo"]["settings"]["option"] = {"label": "On", "description": "On", "help": "On"}'
run_status "$LINT" "$fixture"
expect_status "a plugin key named option fails" 1
expect_output "the reserved name is named" "seo.option: \`option\` is reserved for the names of the choices"

finish_tests settings-lint

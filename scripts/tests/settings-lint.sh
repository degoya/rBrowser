#!/usr/bin/env bash
#
# scripts/check-settings.sh (scripts/lib/settings_lint.py) against small repositories in a temp
# directory: a fixture whose schema, plugin and catalogue follow every rule passes, and each
# broken rule fails with the key named - a key without x-category, an unknown section, a repeated
# x-order in a section, a local-only key that allows Project, a missing i18n key (label or help
# text), a path key without x-input or with a wrong one, and a plugin schema that adds a
# top-level category, and a key whose text says it applies at the next start without x-restart
# (RB-094-23), a link to a secret key that names no panel (RB-094-27). The repository itself
# passes too.
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

# A secret's values never show (RB-094-26): `x-secret` is true or false, and only on a protected key.
fixture="$(settings_fixture "$SCRATCH/secret")"
edit_json "$fixture/schema/config.v1.json" 'doc["properties"]["startUrl"]["x-secret"] = "yes"'
run_status "$LINT" "$fixture"
expect_status "an x-secret that is no boolean fails" 1
expect_output "the value is refused" "startUrl: x-secret is not true or false"

fixture="$(settings_fixture "$SCRATCH/secret-open")"
edit_json "$fixture/schema/config.v1.json" 'doc["properties"]["startUrl"]["x-secret"] = True'
run_status "$LINT" "$fixture"
expect_status "a secret MCP may write fails" 1
expect_output "the rule is named" "startUrl: a secret key is x-protected"

fixture="$(settings_fixture "$SCRATCH/secret-ok")"
edit_json "$fixture/schema/config.v1.json" \
    'doc["properties"]["startUrl"]["x-secret"] = True; doc["properties"]["startUrl"]["x-protected"] = True'
run_status "$LINT" "$fixture"
expect_status "a protected secret passes" 0

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

fixture="$(settings_fixture "$SCRATCH/options-source-one")"
edit_json "$fixture/schema/config.v1.json" 'doc["properties"]["startUrl"]["x-options"] = "layouts"'
run_status "$LINT" "$fixture"
expect_status "a one-value source on a string passes" 0

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

# RB-090-81: a choice that still accepts its earlier form (a branch marked `deprecated`, the old
# switch) names its options as any choice does; the earlier form needs no name.
fixture="$(settings_fixture "$SCRATCH/option-deprecated")"
edit_json "$fixture/schema/config.v1.json" \
    'doc["properties"]["paper"] = {"oneOf": [{"type": "string", "const": "a4"}, {"type": "boolean", "deprecated": True}], "default": "a4", "x-category": "project", "x-section": "start-url", "x-order": 2, "x-scope": ["project", "local"]}'
edit_json "$fixture/locales/en.json" \
    'doc["settings"]["key"]["paper"] = {"label": "Paper", "description": "Paper", "help": "Paper"}'
run_status "$LINT" "$fixture"
expect_status "a choice with an earlier form still needs its names" 1
expect_output "the option is named" "paper: missing i18n key settings.option.paper.a4 (option \"a4\")"

# RB-090-29: a description or help text names an option by the name the window shows, not by
# its id - an id other than its name that opens a sentence or a clause, stands before "(the
# default)", or is an id no word looks like (camelCase, a hyphen, a digit) anywhere.
DOCK_KEY='doc["properties"]["dock"] = {"type": "string", "enum": ["floating", "left", "perReport", "hidden"], "default": "left", "x-category": "project", "x-section": "start-url", "x-order": 2, "x-scope": ["project", "local"]}'
DOCK_NAMES='doc["settings"]["option"] = {"dock": {"floating": "Own window", "left": "Left (docked)", "perReport": "A window per report", "hidden": "Hidden"}}'
fixture="$(settings_fixture "$SCRATCH/option-ids")"
edit_json "$fixture/schema/config.v1.json" "$DOCK_KEY"
edit_json "$fixture/locales/en.json" "$DOCK_NAMES"'
doc["settings"]["key"]["dock"] = {"label": "Dock", "description": "Where they open: floating in a window, or docked.", "help": "Docked. floating (the default) opens a window; perReport opens one per report."}'
run_status "$LINT" "$fixture"
expect_status "a text naming option ids fails" 1
expect_output "an id opening a clause is named" \
    'dock: settings.key.dock.description names the option id "floating" - name it "Own window"'
expect_output "an id before (the default) is named" \
    'dock: settings.key.dock.help names the option id "floating" - name it "Own window"'
expect_output "a coded id anywhere is named" \
    'dock: settings.key.dock.help names the option id "perReport" - name it "A window per report"'

fixture="$(settings_fixture "$SCRATCH/option-names")"
edit_json "$fixture/schema/config.v1.json" "$DOCK_KEY"
edit_json "$fixture/locales/en.json" "$DOCK_NAMES"'
doc["settings"]["key"]["dock"] = {"label": "Dock", "description": "Where they open: own window, a floating one, or Left.", "help": "Left (the default) docks them; hidden hides them; A window per report opens one per report. Not that: own window."}'
run_status "$LINT" "$fixture"
expect_status "a text naming the options by their names passes" 0

# RB-090-85: every settings target of a notice and every openSettings key opens a place in the
# settings window - a key, a category, a section or a key prefix; a constant is read where it
# is defined, the nearest definition first.
fixture="$(settings_fixture "$SCRATCH/targets-ok")"
mkdir -p "$fixture/crates/rbrowser/src/a" "$fixture/crates/rbrowser/src/b" "$fixture/ui/src/x"
cat > "$fixture/crates/rbrowser/src/a/mod.rs" << 'EOF'
const SETTINGS_KEY: &str = "sync";
fn notices() {
    let _ = Notification { settings_key: Some(SETTINGS_KEY.to_owned()) };
    let _ = Notification { settings_key: Some("sync.scroll".to_owned()) };
    let _ = Notification { settings_key: Some(super::b::AREA.to_owned()) };
    let _ = warning_notification(KEY, Some("network"));
    let _ = Notification { settings_key: Some(key.to_owned()) };
}
impl Failure {
    pub fn settings_key(&self) -> Option<&'static str> {
        match self {
            Failure::Off => Some("start-url"),
            _ => None,
        }
    }
}
EOF
printf 'pub const AREA: &str = "scroll";\n' > "$fixture/crates/rbrowser/src/a/b.rs"
printf 'const SETTINGS_KEY: &str = "no.such";\n' > "$fixture/crates/rbrowser/src/b/mod.rs"
printf "send({ type: 'openSettings', key: 'checks' });\n" > "$fixture/ui/src/x/page.ts"
run_status "$LINT" "$fixture"
expect_status "keys, categories, sections and prefixes as targets pass" 0
expect_output "the targets are counted" "6 notice targets"

fixture="$(settings_fixture "$SCRATCH/targets-bad")"
mkdir -p "$fixture/crates/rbrowser/src/a" "$fixture/ui/src/x"
cat > "$fixture/crates/rbrowser/src/a/mod.rs" << 'EOF'
const SETTINGS_KEY: &str = "sitemaps";
fn notices() {
    let _ = Notification { settings_key: Some(SETTINGS_KEY.to_owned()) };
    let _ = Notification { settings_key: Some(MISSING_KEY.to_owned()) };
}
EOF
printf "const message = { settingsKey: 'sync.nothing' };\n" > "$fixture/ui/src/x/page.ts"
run_status "$LINT" "$fixture"
expect_status "a target that opens nothing fails" 1
expect_output "the constant's target is named" \
    "crates/rbrowser/src/a/mod.rs:3: settings target \`sitemaps\` is no key, category, section or key prefix"
expect_output "a constant that does not read is named" \
    "crates/rbrowser/src/a/mod.rs:4: settings target: the constant MISSING_KEY does not read"
expect_output "a page's target is named" \
    "ui/src/x/page.ts:1: settings target \`sync.nothing\` is no key"

# RB-094-21: a text that names a settings path ("Settings > ...", "Settings › ...") is a link -
# its key is in crates/rbrowser/data/settings-links.json, mapped to a place that opens, or listed
# as unlinked with why; the settings window's own texts and MCP tool descriptions need none. A
# panel's `openSettings('<key>')` and a page's `setting-key="<key>"` are settings targets too.
MENTIONS='doc["sync"] = {"notice": {"off": "Scrolling is off. Switch it on in Settings > Sync > Scroll."}}
doc["phone"] = {"hint": "Turn off the phone'"'"'s blocker (Settings › Security)."}
doc["settings"]["key"]["sync"]["scroll"]["help"] = "Off by default; see Settings > Sync."'
PLUGIN_MENTION='doc["plugin"]["seo"]["panel"] = {"off": "Titles are not checked (Settings > Checks > SEO)."}
doc["plugin"]["seo"]["tools"] = {"check": "Checks titles (default: Settings > Checks > SEO)."}'
LINKS_TABLE='{"links": {"sync.notice.off": "sync.scroll", "plugin.seo.panel.off": "seo.titleLength"},
 "unlinked": {"phone.hint": "the phone'"'"'s own Settings app"}}'

fixture="$(settings_fixture "$SCRATCH/mentions-missing")"
edit_json "$fixture/locales/en.json" "$MENTIONS"
edit_json "$fixture/plugins/seo/locales/en.json" "$PLUGIN_MENTION"
run_status "$LINT" "$fixture"
expect_status "a settings mention without a link fails" 1
expect_output "the core text is named" \
    "locales/en.json: \`sync.notice.off\` names a settings path but links nowhere"
expect_output "the other arrow counts too" \
    "locales/en.json: \`phone.hint\` names a settings path but links nowhere"
expect_output "the plugin's text is named" \
    "plugins/seo/locales/en.json: \`plugin.seo.panel.off\` names a settings path but links nowhere"
expect_true "the settings window's help needs no link" '[[ "$output" != *"sync.scroll.help"* ]]'
expect_true "an MCP tool description needs no link" '[[ "$output" != *"plugin.seo.tools.check"* ]]'

fixture="$(settings_fixture "$SCRATCH/mentions-linked")"
edit_json "$fixture/locales/en.json" "$MENTIONS"
edit_json "$fixture/plugins/seo/locales/en.json" "$PLUGIN_MENTION"
mkdir -p "$fixture/crates/rbrowser/data" "$fixture/plugins/seo/web/src/panel" "$fixture/ui/src/x"
printf '%s\n' "$LINKS_TABLE" > "$fixture/crates/rbrowser/data/settings-links.json"
printf "panel.openSettings('seo.titleLength');\n" > "$fixture/plugins/seo/web/src/panel/off.ts"
printf '<SettingLink setting-key="sync.scroll" @open="send" />\n<SettingLink :setting-key="AREA" />\n' \
    > "$fixture/ui/src/x/Off.vue"
printf "const line = { tone: 'error', text, setting: { key: 'seo', label } };\n" \
    > "$fixture/plugins/seo/web/src/panel/line.ts"
run_status "$LINT" "$fixture"
expect_status "every mention linked or unlinked with a reason passes" 0
expect_output "the mentions are counted" "3 settings mentions"
expect_output "a panel's, its status line's and a page's link are targets" "3 notice targets"

fixture="$(settings_fixture "$SCRATCH/mentions-bad")"
edit_json "$fixture/locales/en.json" "$MENTIONS"
mkdir -p "$fixture/crates/rbrowser/data" "$fixture/plugins/seo/web/src/panel"
printf '%s\n' '{"links": {"sync.notice.off": "sync.nothing", "sync.gone": "sync.scroll"},
 "unlinked": {"phone.hint": " "}}' > "$fixture/crates/rbrowser/data/settings-links.json"
printf "panel.openSettings('seo.nothing');\n" > "$fixture/plugins/seo/web/src/panel/off.ts"
run_status "$LINT" "$fixture"
expect_status "a link that opens nothing, a stale row and an unlinked text without why fail" 1
expect_output "the target is named" \
    "settings-links.json: \`sync.notice.off\` links to \`sync.nothing\`, which is no key, category, section or key prefix"
expect_output "a row whose text names no settings path is named" \
    "settings-links.json: \`sync.gone\` is no text that names a settings path"
expect_output "an unlinked text needs its reason" \
    "settings-links.json: unlinked \`phone.hint\` says not why"
expect_output "a panel's target is checked" \
    "plugins/seo/web/src/panel/off.ts:1: settings target \`seo.nothing\` is no key"

printf '{ "links": ' > "$fixture/crates/rbrowser/data/settings-links.json"
run_status "$LINT" "$fixture"
expect_status "a table that is not JSON fails" 1
expect_output "the table is named" "crates/rbrowser/data/settings-links.json: not JSON"

# RB-094-27: a secret key is no setting of the window. Its `x-panel` names the panel of its plugin
# that edits it; a link to it - in the table (its text names no settings path) or a notice's
# target - opens that panel. A link to a secret key without one, an x-panel on another key, one
# of no panel of the plugin or of another shape, and a panel link whose text names a settings
# path fail.
VAULT='doc["properties"]["vault"] = {"type": "object", "default": {},
  "x-category": "checks", "x-section": "seo", "x-order": 2, "x-scope": ["local"],
  "x-local-only": True, "x-protected": True, "x-secret": True,
  "x-panel": {"panel": "seo", "tab": "vault"}}'
VAULT_TEXTS='doc["plugin"]["seo"]["settings"]["vault"] = {"label": "Vault", "description": "Kept here.", "help": "Kept on this machine."}
doc["plugin"]["seo"]["panel"] = {"vaultGone": "The vault (SEO › Vault) does not read."}'
vault_fixture() {
    local fixture
    fixture="$(settings_fixture "$1")"
    edit_json "$fixture/plugins/seo/settings.schema.json" "$VAULT"
    edit_json "$fixture/plugins/seo/locales/en.json" "$VAULT_TEXTS"
    printf '\n[[panels]]\nid = "seo"\ntitle = "plugin.seo.panel.title"\nentry = "panel/index.html"\n' \
        >> "$fixture/plugins/seo/manifest.toml"
    mkdir -p "$fixture/crates/rbrowser/data" "$fixture/plugins/seo/web/src/panel"
    printf '%s\n' '{"links": {"plugin.seo.panel.vaultGone": "seo.vault"}, "unlinked": {}}' \
        > "$fixture/crates/rbrowser/data/settings-links.json"
    printf "const line = { tone: 'error', text, setting: { key: 'seo.vault', label } };\n" \
        > "$fixture/plugins/seo/web/src/panel/line.ts"
    printf '%s\n' "$fixture"
}

fixture="$(vault_fixture "$SCRATCH/vault-ok")"
run_status "$LINT" "$fixture"
expect_status "a secret key linked to the panel that edits it passes" 0
expect_output "its status line's link is a target" "1 notice targets"

fixture="$(vault_fixture "$SCRATCH/vault-no-panel")"
edit_json "$fixture/plugins/seo/settings.schema.json" 'del doc["properties"]["vault"]["x-panel"]'
run_status "$LINT" "$fixture"
expect_status "a link to a secret key without its panel fails" 1
expect_output "the table's link is refused" \
    "settings-links.json: \`plugin.seo.panel.vaultGone\` links to \`seo.vault\`, a secret key: it is not in the settings window and names no panel (x-panel)"
expect_output "the notice's target is refused" \
    "plugins/seo/web/src/panel/line.ts:1: settings target \`seo.vault\` is a secret key: it is not in the settings window and names no panel (x-panel)"

fixture="$(vault_fixture "$SCRATCH/vault-unknown-panel")"
edit_json "$fixture/plugins/seo/settings.schema.json" 'doc["properties"]["vault"]["x-panel"]["panel"] = "keys"'
run_status "$LINT" "$fixture"
expect_status "an x-panel of no panel of the plugin fails" 1
expect_output "the panel is named" "seo.vault: x-panel names \`keys\`, no panel of plugin seo"

fixture="$(vault_fixture "$SCRATCH/vault-shape")"
edit_json "$fixture/plugins/seo/settings.schema.json" 'doc["properties"]["vault"]["x-panel"] = "seo"'
run_status "$LINT" "$fixture"
expect_status "an x-panel of another shape fails" 1
expect_output "the shape is named" 'seo.vault: x-panel is not {"panel": <id>, "tab": <id>}'

fixture="$(vault_fixture "$SCRATCH/vault-not-secret")"
edit_json "$fixture/plugins/seo/settings.schema.json" \
    'doc["properties"]["titleLength"]["x-panel"] = {"panel": "seo"}'
run_status "$LINT" "$fixture"
expect_status "an x-panel on a key that is no secret fails" 1
expect_output "the rule is named" "seo.titleLength: x-panel is only on an x-secret key"

fixture="$(vault_fixture "$SCRATCH/vault-settings-text")"
edit_json "$fixture/plugins/seo/locales/en.json" \
    'doc["plugin"]["seo"]["panel"]["vaultGone"] = "The vault (Settings > Checks > SEO) does not read."'
run_status "$LINT" "$fixture"
expect_status "a panel link whose text names a settings path fails" 1
expect_output "the text is named" \
    "settings-links.json: \`plugin.seo.panel.vaultGone\` opens the panel seo.seo#vault but its text names a settings path"

# RB-094-23: a key applies at once, or carries x-restart; a text that says the next start needs it.
fixture="$(settings_fixture "$SCRATCH/restart-text")"
edit_json "$fixture/locales/en.json" \
    'doc["settings"]["key"]["sync"]["scroll"]["help"] = "Syncs the scroll position. Applies at the next start."'
run_status "$LINT" "$fixture"
expect_status "a key whose text says the next start fails without x-restart" 1
expect_output "the key and the rule are named" \
    "sync.scroll: its text says it applies at the next start but it has no x-restart"
edit_json "$fixture/schema/config.v1.json" 'doc["$defs"]["Sync"]["properties"]["scroll"]["x-restart"] = True'
run_status "$LINT" "$fixture"
expect_status "the same key with x-restart passes" 0

fixture="$(settings_fixture "$SCRATCH/restart-value")"
edit_json "$fixture/schema/config.v1.json" 'doc["$defs"]["Sync"]["properties"]["scroll"]["x-restart"] = "yes"'
run_status "$LINT" "$fixture"
expect_status "an x-restart that is not true fails" 1
expect_output "the value is named" "sync.scroll: x-restart is not true"

fixture="$(settings_fixture "$SCRATCH/restart-other")"
edit_json "$fixture/locales/en.json" \
    'doc["settings"]["key"]["sync"]["scroll"]["help"] = "Applies after the Node sidecar restarts; loads at the next start."'
run_status "$LINT" "$fixture"
expect_status "a process of its own starting again is no restart of rBrowser" 0

finish_tests settings-lint

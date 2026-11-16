#!/usr/bin/env bash
#
# scripts/lib/merge_locales.py and scripts/lib/check_i18n.py against small trees built in a temp
# directory. The merge: parts land in en.json nested and sorted, a second run is a no-op, --check
# tells a pending part, a conflict (with en.json or between parts), a malformed message and a key
# that is a text and a prefix at once write nothing, --prune removes the parts. The lint: a clean
# tree passes; an unknown key in Rust or Vue, an unused key, raw text in a template, a malformed
# plural and a non-canonical en.json fail; a key only a part holds and a family looked up by a
# computed key pass. Last, the Python message parser agrees with the shared format vectors
# (locales/tests/format-cases.json) that rb-i18n's tests hold the Rust side to.
#
# Pure python3 and bash: it runs in a second. check.sh --preflight runs it.
#
#   scripts/tests/i18n.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MERGE="$ROOT/scripts/lib/merge_locales.py"
LINT="$ROOT/scripts/lib/check_i18n.py"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
TREE="$SCRATCH/repo"

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
# shellcheck disable=SC2034  # `status` is read inside the eval of expect()
merge() { python3 "$MERGE" --root "$TREE" "$@" > "$SCRATCH/out" 2>&1 && status=0 || status=$?; }
# shellcheck disable=SC2034
lint() { python3 "$LINT" "$TREE" > "$SCRATCH/out" 2>&1 && status=0 || status=$?; }
has() { grep -qF -- "$1" "$SCRATCH/out"; }
fresh() {
    rm -rf "$TREE"
    mkdir -p "$TREE/locales/parts" "$TREE/crates/demo/src" "$TREE/ui/src"
    printf '{}\n' > "$TREE/locales/en.json"
    touch "$TREE/locales/parts/.gitkeep"
}

# --- merge ---------------------------------------------------------------------------------
fresh
printf '{"toolbar": {"reload": "Reload"}, "grid": {"views": "{count, plural, one {# view} other {# views}}"}}' \
    > "$TREE/locales/parts/RB-010-07.json"
printf '{"config": {"error": {"parse": "Cannot read {path}"}}, "toolbar": {"reload": "Reload"}}' \
    > "$TREE/locales/parts/RB-010-01.json"
merge --check
expect_true "check tells a pending part" '[[ $status -eq 1 ]]'
merge
expect_true "the parts are merged" '[[ $status -eq 0 ]]'
expected='{
  "config": {
    "error": {
      "parse": "Cannot read {path}"
    }
  },
  "grid": {
    "views": "{count, plural, one {# view} other {# views}}"
  },
  "toolbar": {
    "reload": "Reload"
  }
}'
expect "en.json is nested and sorted" "$expected" "$(cat "$TREE/locales/en.json")"
cp "$TREE/locales/en.json" "$SCRATCH/first"
merge
expect_true "a second run is a no-op" '[[ $status -eq 0 ]] && cmp -s "$SCRATCH/first" "$TREE/locales/en.json" && has "up to date"'
merge --check
expect_true "check passes once merged" '[[ $status -eq 0 ]]'

printf '{"toolbar": {"reload": "Refresh"}}' > "$TREE/locales/parts/RB-010-09.json"
merge
expect_true "a conflict with en.json writes nothing" '[[ $status -eq 1 ]] && has "conflict: \`toolbar.reload\`" && cmp -s "$SCRATCH/first" "$TREE/locales/en.json"'
rm "$TREE/locales/parts/RB-010-09.json"
printf '{"grid": {"title": "Grid"}}' > "$TREE/locales/parts/RB-010-20.json"
printf '{"grid": {"title": "Layout"}}' > "$TREE/locales/parts/RB-010-21.json"
merge
expect_true "a conflict between two parts writes nothing" '[[ $status -eq 1 ]] && has "RB-010-21.json" && cmp -s "$SCRATCH/first" "$TREE/locales/en.json"'
rm "$TREE/locales/parts/RB-010-2"*.json
printf '{"grid": {"bad": "{count, plural, one {# view}}"}}' > "$TREE/locales/parts/RB-010-22.json"
merge
expect_true "a malformed plural is refused" '[[ $status -eq 1 ]] && has "needs an \`other\` branch"'
printf '{"grid": {"views": {"short": "Views"}}}' > "$TREE/locales/parts/RB-010-22.json"
merge
expect_true "a key that is a text and a prefix is refused" '[[ $status -eq 1 ]] && has "prefix"'
printf '{"grid": {"a": "A", "a": "B"}}' > "$TREE/locales/parts/RB-010-22.json"
merge
expect_true "a key given twice in one part is refused" '[[ $status -eq 1 ]] && has "given twice"'
rm "$TREE/locales/parts/RB-010-22.json"
merge --prune
expect_true "prune removes the parts and keeps .gitkeep" '[[ $status -eq 0 && -z "$(find "$TREE/locales/parts" -name "*.json")" && -e "$TREE/locales/parts/.gitkeep" ]]'
merge --check --prune
expect_true "check and prune exclude each other" '[[ $status -eq 2 ]]'

# --- lint ----------------------------------------------------------------------------------
fresh
printf '{\n  "grid": {\n    "views": "{count, plural, one {# view} other {# views}}"\n  },\n  "toolbar": {\n    "reload": "Reload",\n    "zoom": {\n      "in": "Zoom in"\n    }\n  }\n}\n' \
    > "$TREE/locales/en.json"
cat > "$TREE/crates/demo/src/lib.rs" <<'EOF'
/// `t!("toolbar.documented")` in a comment is no lookup.
pub fn label() -> String {
    rb_i18n::t!(
        "grid.views",
        count = 3
    )
}
EOF
cat > "$TREE/ui/src/Toolbar.vue" <<'EOF'
<script setup lang="ts">
const zoom = (dir: string) => t(`toolbar.zoom.${dir}`)
</script>
<template>
  <!-- A comment with Words is no text -->
  <button :title="t('toolbar.reload')">{{ t('toolbar.reload') }}</button>
  <span>{{ count > 1 ? '+' : '-' }} &times; 42</span>
</template>
EOF
lint
expect_true "a clean tree passes" '[[ $status -eq 0 ]] && has "3 key(s)"'

cp "$TREE/ui/src/Toolbar.vue" "$SCRATCH/Toolbar.vue"
printf '<template><b>{{ $t("toolbar.missing") }}</b></template>\n' > "$TREE/ui/src/Missing.vue"
lint
expect_true "an unknown key in a .vue file fails" '[[ $status -eq 1 ]] && has "ui/src/Missing.vue:1: \`toolbar.missing\`"'
rm "$TREE/ui/src/Missing.vue"
printf 'fn x() -> String { t("config.error.nope") }\n' > "$TREE/crates/demo/src/more.rs"
lint
expect_true "an unknown key in Rust fails" '[[ $status -eq 1 ]] && has "crates/demo/src/more.rs:1: \`config.error.nope\`"'
printf 'fn x() -> String { t(&format!("config.error.{}", 1)) }\n' > "$TREE/crates/demo/src/more.rs"
lint
expect_true "a computed key is no unknown key" '[[ $status -eq 0 ]]'
rm "$TREE/crates/demo/src/more.rs"

printf '<template>\n  <button>Save</button>\n</template>\n' > "$TREE/ui/src/Raw.vue"
lint
expect_true "raw text in a template fails" '[[ $status -eq 1 ]] && has "ui/src/Raw.vue:2: raw text \`Save\`"'
printf '<template>\n  <input placeholder="Search" :title="t(%s)">\n</template>\n' "'toolbar.reload'" > "$TREE/ui/src/Raw.vue"
lint
expect_true "a raw visible attribute fails" '[[ $status -eq 1 ]] && has "placeholder=\"Search\""'
rm "$TREE/ui/src/Raw.vue"

python3 - "$TREE/locales/en.json" <<'EOF'
import json, sys
path = sys.argv[1]
data = json.load(open(path))
data["toolbar"]["forgotten"] = "Forgotten"
open(path, "w").write(json.dumps(data, indent=2, sort_keys=True) + "\n")
EOF
lint
expect_true "an unused key fails" '[[ $status -eq 1 ]] && has "\`toolbar.forgotten\` is used nowhere"'
printf 'const KEY: &str = "toolbar.forgotten";\n' > "$TREE/crates/demo/src/keys.rs"
lint
expect_true "a key written as a literal counts as used" '[[ $status -eq 0 ]]'
mkdir -p "$TREE/crates/demo/src/x"
printf 'fn y() { t("toolbar.forgotten"); }\n' > "$TREE/crates/demo/src/x/keys_tests.rs"
rm "$TREE/crates/demo/src/keys.rs"
lint
expect_true "a key only a test names is unused" '[[ $status -eq 1 ]] && has "\`toolbar.forgotten\` is used nowhere"'
rm -r "$TREE/crates/demo/src/x"
printf '<template><i>{{ t("toolbar.parted") }}</i></template>\n' > "$TREE/ui/src/Part.vue"
printf '{"toolbar": {"parted": "From a part"}}' > "$TREE/locales/parts/RB-010-07.json"
python3 - "$TREE/locales/en.json" <<'EOF'
import json, sys
path = sys.argv[1]
data = json.load(open(path))
del data["toolbar"]["forgotten"]
open(path, "w").write(json.dumps(data, indent=2, sort_keys=True) + "\n")
EOF
lint
expect_true "a key only a part holds passes" '[[ $status -eq 0 ]]'
rm "$TREE/ui/src/Part.vue" "$TREE/locales/parts/RB-010-07.json"

printf '{"toolbar": {"reload": "Reload", "zoom": {"in": "Zoom in"}}, "grid": {"views": "{count, plural, one {# view} other {# views}}"}}\n' \
    > "$TREE/locales/en.json"
lint
expect_true "a non-canonical en.json fails" '[[ $status -eq 1 ]] && has "not canonical"'
printf '{\n  "grid": {\n    "views": "{count, plural, one {# view}}"\n  },\n  "toolbar": {\n    "reload": "Reload",\n    "zoom": {\n      "in": "Zoom in"\n    }\n  }\n}\n' \
    > "$TREE/locales/en.json"
lint
expect_true "a malformed plural in en.json fails" '[[ $status -eq 1 ]] && has "grid.views"'

# RB-080-90: no error code set into a text, no count without its plural form - a plugin's too.
printf '{\n  "grid": {\n    "views": "{count, plural, one {# view} other {# views}}"\n  },\n  "toolbar": {\n    "reload": "Reload",\n    "zoom": {\n      "in": "Zoom in"\n    }\n  }\n}\n' \
    > "$TREE/locales/en.json"
mkdir -p "$TREE/plugins/demo/locales"
printf '{"plugin": {"demo": {"panel": {"failed": "Failed: {code}", "marked": "Marked {count} elements in {views, plural, one {# view} other {# views}}.", "size": "{width} pixels wide; it {count} is"}}}}' \
    > "$TREE/plugins/demo/locales/en.json"
lint
expect_true "a {code} text fails" '[[ $status -eq 1 ]] && has "plugin.demo.panel.failed\` sets an error code"'
expect_true "a count without its plural form fails" '[[ $status -eq 1 ]] && has "\`{count} elements\` has no plural form"'
expect_true "a plural, a size and a verb pass" '! has "{views} views" && ! has "plugin.demo.panel.size"'
printf '{"plugin": {"demo": {"panel": {"marked": "Marked {count, plural, one {# element} other {# elements}}."}}}}' \
    > "$TREE/plugins/demo/locales/en.json"
lint
expect_true "the plural form passes" '[[ $status -eq 0 ]]'
rm -r "$TREE/plugins"

# --- no key walks into a prototype (security review c20) ----------------------------------
fresh
printf '{"plugin": {"demo": {"constructor": {"prototype": {"x": "Polluted"}}}}}' > "$TREE/locales/en.json"
lint
expect_true "a constructor or prototype segment is no key" '[[ $status -eq 1 ]] && has "plugin.demo.constructor.prototype.x"'

# --- every plugin command has its title (RB-080-82) ---------------------------------------
fresh
mkdir -p "$TREE/plugins/demo/locales"
printf 'id = "demo"\n[locales]\nen = "locales/en.json"\n[[commands]]\nid = "run"\ntitle = "plugin.demo.commands.run"\n' \
    > "$TREE/plugins/demo/manifest.toml"
printf '{"plugin": {"demo": {"commands": {"run": "Demo: Run"}}}}' > "$TREE/plugins/demo/locales/en.json"
lint
expect_true "a plugin command with its title passes" '[[ $status -eq 0 ]]'
printf '{"plugin": {"demo": {"commands": {}}}}' > "$TREE/plugins/demo/locales/en.json"
lint
expect_true "a plugin command without its text fails" '[[ $status -eq 1 ]] && has "plugin.demo.commands.run"'
printf 'id = "demo"\n[locales]\nen = "locales/en.json"\n[[commands]]\nid = "run"\ntitle = "plugin.other.commands.run"\n' \
    > "$TREE/plugins/demo/manifest.toml"
lint
expect_true "a title below another plugin fails" '[[ $status -eq 1 ]] && has "not below plugin.demo."'
printf 'id = "demo"\n[[commands]]\nid = "run"\n' > "$TREE/plugins/demo/manifest.toml"
lint
expect_true "a plugin command without a title key fails" '[[ $status -eq 1 ]] && has "has no title key"'

# --- the shared format vectors -------------------------------------------------------------
if python3 - "$ROOT/locales/tests/format-cases.json" "$ROOT/scripts/lib" > "$SCRATCH/out" 2>&1 <<'EOF'
import json, sys
sys.dont_write_bytecode = True
sys.path.insert(0, sys.argv[2])
from i18n_catalogue import message_error
doc = json.load(open(sys.argv[1]))
wrong = [c["message"] for c in doc["cases"] if message_error(c["message"])]
wrong += [m for m in doc["invalid"] if not message_error(m)]
print("\n".join(wrong))
sys.exit(1 if wrong else 0)
EOF
then
    ok "the Python parser agrees with the shared format vectors"
else
    fail "the Python parser agrees with the shared format vectors" "$(cat "$SCRATCH/out")"
fi

finish_tests i18n

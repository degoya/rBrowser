#!/usr/bin/env bash
#
# scripts/release-plugins.sh (RB-090-79), the plugin half of a release, on a small tree in a temp
# directory with a stand-in rb-plugin-pack and gh: `sign` signs every built plugin, `index` lists
# every first-party plugin but the test-only one (RB-091-04) with addresses in the release
# `plugins-v<version>`, its groups and the previous index carried,
# `assets` publishes every first-party package but the test-only one and SHA256SUMS - no offline
# bundle, the package's own plugins/ is that (RB-091-04) - `notes` lists plugin and version,
# `gate` writes the official index for the newest plain tag only, and `publish-index` makes the
# fixed release `plugin-index` once and replaces its asset. Nothing here builds or reaches GitHub.
#
#   scripts/tests/release-plugins.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

TREE="$SCRATCH/tree"
mkdir -p "$TREE/scripts/lib" "$TREE/target/plugins" "$TREE/artifacts/plugins"
cp "$ROOT/scripts/release-plugins.sh" "$ROOT/scripts/release-assets.sh" \
    "$ROOT/scripts/build-plugins.sh" "$ROOT/scripts/catalogue-plugins.sh" "$TREE/scripts/"
cp "$ROOT/scripts/lib/"*.sh "$TREE/scripts/lib/"
SCRIPT="$TREE/scripts/release-plugins.sh"
plugin() {
    mkdir -p "$TREE/plugins/$1"
    printf '[package]\nname = "rbrowser-plugin-%s"\n' "$1" > "$TREE/plugins/$1/Cargo.toml"
    printf 'manifest_version = 1\nid = "%s"\nname = "%s"\nversion = "%s"\n' "$1" "$2" "$3" \
        > "$TREE/plugins/$1/manifest.toml"
    echo component > "$TREE/target/plugins/$1.wasm"
}
plugin seo "SEO" 0.1.0
plugin outline "Outline" 1.2.0
touch "$TREE/plugins/outline/CATALOG-ONLY"
plugin hello "Hello panel" 1.0.0
touch "$TREE/plugins/hello/TEST-ONLY"
mkdir -p "$TREE/plugins/web-common"

# The stand-in packer: `package` writes the folder's name and the key it was given, `index` the
# packages of its folder and its arguments; every call is logged.
PACK="$SCRATCH/rb-plugin-pack"
cat > "$PACK" <<'EOF'
#!/usr/bin/env bash
echo "$*" >> "${PACK_LOG}"
out=""; key=""
args=("$@")
for ((i = 0; i < ${#args[@]}; i++)); do
    case "${args[i]}" in
        --out) out="${args[i + 1]}" ;;
        --key) key="${args[i + 1]}" ;;
    esac
done
case "$1" in
    package) echo "signed $(basename "$2") with $(basename "$key")" > "$out" ;;
    index) { ls "$2"; echo "${args[*]:2}"; } > "$out" ;;
esac
EOF
chmod +x "$PACK"
export PACK_LOG="$SCRATCH/pack.log"
KEY="$SCRATCH/keys/plugin-signing.pem"
mkdir -p "$SCRATCH/keys"
echo secret > "$KEY"
PACK_ENV=(env RB_PLUGIN_PACK="$PACK")

# --- sign ---
run_status "${PACK_ENV[@]}" "$SCRIPT" sign "$KEY"
expect_status "sign signs the built plugins" 0
expect "every built plugin is signed, the test-only one too" "3" "$(grep -c '^package ' "$PACK_LOG")"
expect "the package carries the release key" "signed seo with plugin-signing.pem" \
    "$(cat "$TREE/artifacts/plugins/seo-0.1.0.rbplugin")"
expect_true "the package is named by id and version" \
    "[[ -f '$TREE/artifacts/plugins/outline-1.2.0.rbplugin' ]]"

run_status "${PACK_ENV[@]}" "$SCRIPT" sign "$TREE/key.pem"
expect_status "a key inside the checkout is refused" 2
expect_output "and the reason named" "outside the checkout"

rm "$TREE/target/plugins/seo.wasm"
run_status "${PACK_ENV[@]}" "$SCRIPT" sign "$KEY"
expect_status "a plugin that was not built fails the signing" 1
expect_output "naming the component" "target/plugins/seo.wasm"
echo component > "$TREE/target/plugins/seo.wasm"

# --- index ---
: > "$PACK_LOG"
run_status "${PACK_ENV[@]}" GITHUB_REPOSITORY=degoya/rBrowser INDEX_VALID_DAYS=120 \
    "$SCRIPT" index "$KEY" v1.4.0
expect_status "index signs the catalogue's index" 0
index="$TREE/artifacts/plugin-index/rbrowser-plugin-index.json"
expect "the index lists every first-party plugin but the test-only one" \
    "outline-1.2.0.rbplugin seo-0.1.0.rbplugin" "$(grep rbplugin "$index" | tr '\n' ' ' | sed 's/ $//')"
output="$(cat "$PACK_LOG")"
expect_output "with its address in the plugin release" \
    "--base-url https://github.com/degoya/rBrowser/releases/download/plugins-v1.4.0"
expect_output "under the release key's id" "--key-id rbrowser-plugin-v1"
expect_output "valid for the configured days" "--validity-days 120"
expect_output "with the groups of the first-party list" "--categories $TREE/plugins/first-party.json"
expect "and no previous index unless one is named" "0" "$(grep -c -- '--previous' "$PACK_LOG" || true)"
echo '{}' > "$SCRATCH/previous.json"
: > "$PACK_LOG"
run_status "${PACK_ENV[@]}" GITHUB_REPOSITORY=degoya/rBrowser PREVIOUS_INDEX="$SCRATCH/previous.json" \
    "$SCRIPT" index "$KEY" v1.4.0
expect_status "index carries the previous index" 0
output="$(cat "$PACK_LOG")"
expect_output "named to the packer" "--previous $SCRATCH/previous.json"
run_status "${PACK_ENV[@]}" GITHUB_REPOSITORY=degoya/rBrowser PREVIOUS_INDEX="$SCRATCH/none.json" \
    "$SCRIPT" index "$KEY" v1.4.0
expect_status "a previous index that is not there fails" 1
expect_true "the sequence is the time of signing" \
    "grep -qE -- '--sequence [0-9]{10,}' '$PACK_LOG'"

run_status "${PACK_ENV[@]}" GITHUB_REPOSITORY="" "$SCRIPT" index "$KEY" v1.4.0
expect_status "index without GITHUB_REPOSITORY fails" 1

# --- assets ---
assets="$SCRATCH/assets"
run_status "$SCRIPT" assets "$TREE/artifacts/plugins" "$assets"
expect_status "assets stages the plugin release" 0
expect "every first-party package but the test-only one, and the sums" \
    "SHA256SUMS outline-1.2.0.rbplugin seo-0.1.0.rbplugin" \
    "$(find "$assets" -maxdepth 1 -type f -printf '%f\n' | LC_ALL=C sort | tr '\n' ' ' | sed 's/ $//')"
expect "no offline bundle: the package's bundled plugins are the offline way" "0" \
    "$(find "$assets" -name '*.zip' | wc -l | tr -d ' ')"
run_status bash -c "cd '$assets' && sha256sum --check --strict SHA256SUMS"
expect_status "sha256sum -c accepts the plugin release's sums" 0
run_status "$SCRIPT" assets "$TREE/artifacts/plugins" "$assets" 1.4.0
expect_status "a third argument is a usage error" 2

rm "$TREE/artifacts/plugins/seo-0.1.0.rbplugin"
run_status "$SCRIPT" assets "$TREE/artifacts/plugins" "$SCRATCH/assets-missing"
expect_status "a missing package fails the plugin release" 1
expect_output "naming it" "seo-0.1.0.rbplugin"

# --- notes ---
run_status "$SCRIPT" notes 1.4.0
expect_status "notes print the release text" 0
expect_output "a row per published plugin" "| SEO | \`seo\` | 0.1.0 |"
expect_output "the catalogue-only one too" "| Outline | \`outline\` | 1.2.0 |"
expect "but not the test-only one" "0" "$(grep -c 'Hello panel' <<< "$output" || true)"
expect "no offline bundle is offered" "0" "$(grep -c 'rbrowser-plugins-' <<< "$output" || true)"

# --- gate: an origin with release tags, and a checkout of it whose scripts/ are the tree's ---
git init --quiet --bare "$SCRATCH/origin.git"
work="$SCRATCH/work"
git init --quiet "$work"
git -C "$work" -c user.name=t -c user.email=t@example.invalid commit --quiet --allow-empty -m init
for tag in v1.2.0 v1.9.0 v1.10.0 v1.11.0-beta.1 plugins-v1.10.0 plugin-index; do
    git -C "$work" tag "$tag"
done
git -C "$work" remote add origin "$SCRATCH/origin.git"
git -C "$work" push --quiet origin --tags
ln -s "$TREE/scripts" "$work/scripts"
gate() {
    : > "$SCRATCH/output"
    run_status env GITHUB_OUTPUT="$SCRATCH/output" "$@" "$work/scripts/release-plugins.sh" gate
    output+=$'\n'"$(cat "$SCRATCH/output")"
}
cd "$work"
gate REF_NAME=v1.10.0
expect_status "the gate runs" 0
expect_output "the newest plain tag writes the official index" "index=true"
gate REF_NAME=v1.9.0
expect_output "an older tag leaves it" "index=false"
expect_output "naming the newest" "not the newest release tag (v1.10.0)"
gate REF_NAME=v1.11.0-beta.1
expect_output "a pre-release leaves it" "index=false"
expect_output "and says why" "a pre-release never writes the official plugin index"
cd "$ROOT"

# --- publish-index: a stand-in gh ---
mkdir -p "$SCRATCH/bin"
cat > "$SCRATCH/bin/gh" <<'EOF'
#!/usr/bin/env bash
echo "gh $*" >> "${GH_LOG}"
[[ "$1 $2" == "release view" ]] && exit "${GH_VIEW_STATUS:-0}"
exit 0
EOF
chmod +x "$SCRATCH/bin/gh"
export GH_LOG="$SCRATCH/gh.log"
publish() {
    : > "$GH_LOG"
    run_status env PATH="$SCRATCH/bin:$PATH" GITHUB_REPOSITORY=degoya/rBrowser GITHUB_SHA=abc123 \
        "$@" "$SCRIPT" publish-index "$index"
    output="$(cat "$GH_LOG")"
}
publish GH_VIEW_STATUS=0
expect_status "publish-index runs" 0
expect_output "the index replaces the asset of the fixed release" \
    "gh release upload plugin-index $index --clobber --repo degoya/rBrowser"
expect "an existing release is not made again" "0" "$(grep -c 'release create' "$GH_LOG" || true)"
publish GH_VIEW_STATUS=1
expect_output "a missing release is made, never the latest" "--latest=false"
expect_output "on the tag's commit" "--target abc123"
run_status env PATH="$SCRATCH/bin:$PATH" GITHUB_REPOSITORY=degoya/rBrowser GITHUB_SHA=abc123 \
    "$SCRIPT" publish-index "$SCRATCH/none.json"
expect_status "a missing index file fails" 1

run_status "$SCRIPT" unknown
expect_status "an unknown command is a usage error" 2

finish_tests release-plugins

#!/usr/bin/env bash
#
# The plugin half of a release (RB-090-79, after rDownloader's release-plugins.yml): the signed
# first-party packages as a GitHub release of their own, `plugins-v<version>` "rBrowser plugins
# <version>", beside the application's `v<version>`, and the official catalogue's signed index
# `rbrowser-plugin-index.json` (RB-091-01) as the one asset of the fixed release `plugin-index`,
# the address the app knows (OFFICIAL_INDEX_URL, crates/rb-plugin-host/src/repository_types.rs).
#
#   scripts/release-plugins.sh sign <key.pem>                  # every built plugin, signed
#   scripts/release-plugins.sh index <key.pem> <tag>           # the catalogue's signed index
#   scripts/release-plugins.sh assets <packages> <out>
#   scripts/release-plugins.sh notes <version>                 # the plugin release's text
#   scripts/release-plugins.sh gate                            # output index=true|false
#   scripts/release-plugins.sh publish-index <index.json>      # into the release plugin-index
#
# `sign` and `index` run in the step that holds the plugin release key rbrowser-plugin-v1
# (.github/workflows/release-plugins.yml) and compile nothing: they run the rb-plugin-pack and the
# components `scripts/build-plugins.sh --unsigned` built before the key was on the runner
# (RB_PLUGIN_PACK names another packer). `sign` writes `artifacts/plugins/<id>-<version>.rbplugin`
# for every plugin folder, as build-plugins.sh does; `index` lists the catalogue's plugins
# (scripts/catalogue-plugins.sh) with their addresses in `plugins-<tag>`, the sequence the time of
# signing (it only ever rises, so an older index served again is refused as a replay) and valid for
# INDEX_VALID_DAYS (120) days. Both refuse a key inside the checkout.
#
# `assets` stages the plugin release from the signed packages: every first-party plugin but a
# `TEST-ONLY` one and SHA256SUMS over them. No offline bundle: the plugins the package carries
# (its plugins/ folder) are the way without a network, as in rDownloader (RB-091-04). `notes`
# lists plugin, id and version from the manifests.
#
# `gate` (REF_NAME) writes the official index for the newest plain vX.Y.Z tag only: the index
# cannot tell a beta from a stable channel, so a pre-release never replaces it and a re-run of an
# older tag cannot take the catalogue back. `publish-index` (GH_TOKEN, GITHUB_REPOSITORY,
# GITHUB_SHA) makes the release `plugin-index` the first time - never `latest`, which stays the
# application's - and replaces its asset.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACK="${RB_PLUGIN_PACK:-$ROOT/target/release/rb-plugin-pack}"
INDEX_RELEASE=plugin-index
INDEX_KEY_ID=rbrowser-plugin-v1

usage() {
    echo "usage: scripts/release-plugins.sh sign <key.pem> | index <key.pem> <tag>" \
        "| assets <packages> <out> | notes <version> | gate | publish-index <index.json>" >&2
    exit 2
}

# The manifest's value of top-level key $2 of plugin $1.
manifest_value() {
    sed -n "s/^$2 = \"\(.*\)\"$/\1/p" "$ROOT/plugins/$1/manifest.toml" | head -n 1
}

# The first-party plugins a release publishes: every plugin folder but a test-only one.
published_ids() {
    local id
    while IFS= read -r id; do
        [[ -n "$id" && ! -f "$ROOT/plugins/$id/TEST-ONLY" ]] && printf '%s\n' "$id"
    done < <("$ROOT/scripts/build-plugins.sh" --list)
    return 0
}

check_key() {
    local key="$1"
    case "$(realpath -m "$key")" in
        "$(realpath "$ROOT")"/*) echo "!! the signing key must live outside the checkout" >&2; exit 2 ;;
    esac
    [[ -f "$key" ]] || { echo "!! no signing key at $key" >&2; exit 2; }
}

sign() {
    local key="$1" id version component members count=0
    check_key "$key"
    # shellcheck source=lib/plugin-stage.sh
    source "$ROOT/scripts/lib/plugin-stage.sh"
    mkdir -p "$ROOT/artifacts/plugins"
    while IFS= read -r id; do
        [[ -n "$id" ]] || continue
        component="$ROOT/target/plugins/$id.wasm"
        [[ -f "$component" ]] || {
            echo "!! $component is missing: scripts/build-plugins.sh --unsigned builds it" >&2
            exit 1
        }
        version="$(manifest_value "$id" version)"
        members="$(rb_plugin_pack_dir "$ROOT/plugins" "$id" "$ROOT/target/plugins/stage/$id")"
        "$PACK" package "$members" --component "$component" \
            --out "$ROOT/artifacts/plugins/$id-$version.rbplugin" --key "$key" > /dev/null
        count=$((count + 1))
    done < <("$ROOT/scripts/build-plugins.sh" --list)
    echo "==> $count plugin(s) signed into artifacts/plugins/"
}

index() {
    local key="$1" tag="$2" stage="$ROOT/target/plugin-index-stage" out="$ROOT/artifacts/plugin-index"
    local id version package previous=()
    check_key "$key"
    [[ -n "${GITHUB_REPOSITORY:-}" ]] || { echo "!! GITHUB_REPOSITORY is not set" >&2; exit 1; }
    rm -rf "$stage"
    mkdir -p "$stage" "$out"
    while IFS= read -r id; do
        [[ -n "$id" ]] || continue
        version="$(manifest_value "$id" version)"
        package="$ROOT/artifacts/plugins/$id-$version.rbplugin"
        [[ -f "$package" ]] || { echo "!! no signed package $package" >&2; exit 1; }
        cp "$package" "$stage/"
    done < <("$ROOT/scripts/catalogue-plugins.sh" "$ROOT")
    # The published index (fetched by the workflow, RB-091-04): the new one keeps the newest entry
    # of every plugin per contract this release does not build for, as rDownloader's index does.
    if [[ -n "${PREVIOUS_INDEX:-}" ]]; then
        [[ -f "$PREVIOUS_INDEX" ]] || { echo "!! no previous index at $PREVIOUS_INDEX" >&2; exit 1; }
        previous=(--previous "$PREVIOUS_INDEX")
    fi
    # Every package verifies under the compiled-in root before it is listed, which also catches
    # a secret that is not the release key.
    "$PACK" index "$stage" --out "$out/rbrowser-plugin-index.json" \
        --key "$key" --key-id "$INDEX_KEY_ID" \
        --categories "$ROOT/plugins/first-party.json" "${previous[@]}" \
        --sequence "$(date +%s)" --validity-days "${INDEX_VALID_DAYS:-120}" \
        --base-url "https://github.com/${GITHUB_REPOSITORY}/releases/download/plugins-${tag}"
}

assets() {
    local from="$1" out="$2" id package count=0
    rm -rf "$out"
    mkdir -p "$out"
    while IFS= read -r id; do
        package="$id-$(manifest_value "$id" version).rbplugin"
        [[ -f "$from/$package" ]] || { echo "!! no signed package $from/$package" >&2; exit 1; }
        cp "$from/$package" "$out/"
        count=$((count + 1))
    done < <(published_ids)
    [[ "$count" -gt 0 ]] || { echo "!! no first-party plugins to publish" >&2; exit 1; }
    "$ROOT/scripts/release-assets.sh" sums "$out"
    echo "==> $count package(s) in $out"
}

notes() {
    local version="$1" id
    cat <<EOF
The first-party plugins of rBrowser $version, each signed with the plugin release key
\`rbrowser-plugin-v1\` that rBrowser trusts out of the box. The application itself is the release
\`v$version\`.

| Plugin | Id | Version |
| --- | --- | --- |
EOF
    while IFS= read -r id; do
        # shellcheck disable=SC2016  # the backticks are Markdown, not a command
        printf '| %s | `%s` | %s |\n' "$(manifest_value "$id" name)" "$id" "$(manifest_value "$id" version)"
    done < <(published_ids)
    cat <<EOF

- **One plugin**: download its \`.rbplugin\` and install it in Settings > Plugins.
- **Without a network**: the application's package carries these plugins and installs them from
  there.
- \`SHA256SUMS\` lists the checksum of every file of this release (\`sha256sum -c SHA256SUMS\`).
EOF
}

# The newest plain vX.Y.Z tag of origin; sort -V orders v1.10.0 after v1.9.0.
newest() {
    git ls-remote --tags --refs origin 'refs/tags/v*' \
        | sed 's#.*refs/tags/##' | { grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' || true; } \
        | sort -V | tail -n 1
}

gate() {
    local newest
    if [[ "${REF_NAME}" == *-* ]]; then
        echo "::notice::${REF_NAME}: a pre-release never writes the official plugin index; it stays on the last release"
        echo "index=false" >> "${GITHUB_OUTPUT}"
        return 0
    fi
    newest="$(newest)"
    if [[ "${REF_NAME}" != "${newest}" ]]; then
        echo "::notice::${REF_NAME} is not the newest release tag (${newest:-none}); the official plugin index stays where it is"
        echo "index=false" >> "${GITHUB_OUTPUT}"
    else
        echo "index=true" >> "${GITHUB_OUTPUT}"
    fi
}

publish_index() {
    local file="$1" repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is not set}"
    [[ -f "$file" ]] || { echo "!! no index at $file" >&2; exit 1; }
    if ! gh release view "$INDEX_RELEASE" --repo "$repo" > /dev/null 2>&1; then
        gh release create "$INDEX_RELEASE" --repo "$repo" --target "${GITHUB_SHA:?GITHUB_SHA is not set}" \
            --title "rBrowser plugin index" --latest=false \
            --notes "The signed index of the official plugin catalogue; rBrowser reads its one asset. Replaced by every release."
    fi
    gh release upload "$INDEX_RELEASE" "$file" --clobber --repo "$repo"
}

case "${1:-}" in
    sign) [[ $# -eq 2 ]] || usage; sign "$2" ;;
    index) [[ $# -eq 3 ]] || usage; index "$2" "$3" ;;
    assets) [[ $# -eq 3 ]] || usage; assets "$2" "$3" ;;
    notes) [[ $# -eq 2 ]] || usage; notes "$2" ;;
    gate) [[ $# -eq 1 ]] || usage; gate ;;
    publish-index) [[ $# -eq 2 ]] || usage; publish_index "$2" ;;
    *) usage ;;
esac

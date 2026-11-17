#!/usr/bin/env bash
#
# The one version of rBrowser: `[workspace.package] version` in Cargo.toml. Every crate takes
# it with `version.workspace = true`; Cargo.lock repeats it for each workspace crate, and the
# front ends' package.json (ui/, agent/, sidecar/) repeat it as `"version"`. The plugin SDK repeats
# it too, once it exists: `min_app_version` of every template in sdk/templates/, the release a
# scaffold needs, and `RBROWSER_VERSION` of the workflows in sdk/ci/, the release they run. A new
# version also cuts CHANGELOG.md: what `## [Unreleased]` holds becomes `## [X.Y.Z] - <today>`
# (scripts/lib/changelog.sh).
#
# Usage:
#   scripts/set-version.sh                 # print the current version
#   scripts/set-version.sh X.Y.Z           # set it, bring Cargo.lock, package.json and the SDK
#                                          # along, and cut CHANGELOG.md's [Unreleased] into
#                                          # [X.Y.Z]
#   scripts/set-version.sh --check         # every crate takes the workspace version, and
#                                          # Cargo.lock agrees; exit 1 otherwise
#   scripts/set-version.sh X.Y.Z --check   # the same, and the version is X.Y.Z
#
# Takes no build lock: it edits text files and runs `cargo update --workspace`, which resolves
# but compiles nothing.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/workspace-version.sh
source "$ROOT/scripts/lib/workspace-version.sh"
# shellcheck source=lib/changelog.sh
source "$ROOT/scripts/lib/changelog.sh"
cd "$ROOT"

usage() {
    echo "usage: scripts/set-version.sh [X.Y.Z] [--check]" >&2
    exit 2
}

wanted=""
check=0
for argument in "$@"; do
    case "$argument" in
        --check) check=1 ;;
        -*) usage ;;
        *)
            [[ -z "$wanted" ]] || usage
            wanted="$argument"
            ;;
    esac
done
if [[ -n "$wanted" && ! "$wanted" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$ ]]; then
    echo "!! '$wanted' is not a version (X.Y.Z, optionally -pre)" >&2
    exit 2
fi

# The front ends' package.json, which carry the version too.
PACKAGES=(ui/package.json agent/package.json sidecar/package.json)

current="$(rb_workspace_version < Cargo.toml)"
if [[ -z "$current" ]]; then
    echo "!! Cargo.toml has no [workspace.package] version" >&2
    exit 1
fi

# The problems with the version as written: a crate with a version of its own, a Cargo.lock entry
# of a workspace crate (no `source`) with another version.
problems() {
    local manifest name
    for manifest in crates/*/Cargo.toml; do
        [[ -e "$manifest" ]] || continue
        if ! grep -qx 'version.workspace = true' "$manifest"; then
            echo "$manifest: does not take the workspace version (version.workspace = true)"
        fi
    done
    [[ -f Cargo.lock ]] || { echo "Cargo.lock is missing"; return 0; }
    for manifest in crates/*/Cargo.toml; do
        [[ -e "$manifest" ]] || continue
        name="$(sed -n '/^\[package\]/,/^\[/p' "$manifest" | sed -n 's/^name = "\(.*\)"/\1/p' | head -n 1)"
        awk -v name="$name" -v want="$1" '
            /^\[\[package\]\]/ { if (n == name && !src) found = v; n = ""; v = ""; src = 0 }
            /^name = /    { n = $3; gsub(/"/, "", n) }
            /^version = / { v = $3; gsub(/"/, "", v) }
            /^source = /  { src = 1 }
            END {
                if (n == name && !src) found = v
                if (found == "") print "Cargo.lock: no entry for " name
                else if (found != want) print "Cargo.lock: " name " is " found ", not " want
            }' Cargo.lock
    done
    local lock
    for lock in plugins/*/Cargo.lock; do
        [[ -e "$lock" ]] || continue
        awk -v want="$1" -v lock="$lock" '
            /^\[\[package\]\]/ { if ((n == "rb-plugin-api" || n == "rb-plugin-types") && !src && v != want) print lock ": " n " is " v ", not " want; n = ""; v = ""; src = 0 }
            /^name = /    { n = $3; gsub(/"/, "", n) }
            /^version = / { v = $3; gsub(/"/, "", v) }
            /^source = /  { src = 1 }
            END { if ((n == "rb-plugin-api" || n == "rb-plugin-types") && !src && v != want) print lock ": " n " is " v ", not " want }' "$lock"
    done
    for manifest in "${PACKAGES[@]}"; do
        [[ -e "$manifest" ]] || continue
        grep -qx "  \"version\": \"$1\"," "$manifest" \
            || echo "$manifest: version is not $1"
    done
    for manifest in sdk/templates/*/manifest.toml; do
        [[ -e "$manifest" ]] || continue
        grep -qx "min_app_version = \"$1\"" "$manifest" \
            || echo "$manifest: min_app_version is not $1"
    done
    for workflow in sdk/ci/*.yml; do
        [[ -e "$workflow" ]] || continue
        grep -qx "  RBROWSER_VERSION: $1" "$workflow" \
            || echo "$workflow: RBROWSER_VERSION is not $1"
    done
}

if [[ "$check" -eq 1 ]]; then
    status=0
    if [[ -n "$wanted" && "$wanted" != "$current" ]]; then
        echo "!! the workspace version is $current, not $wanted"
        status=1
    fi
    found="$(problems "$current")"
    if [[ -n "$found" ]]; then
        echo "!! the version copies disagree with Cargo.toml ($current):"
        printf '   %s\n' "$found"
        echo "   scripts/set-version.sh $current brings Cargo.lock, package.json and the SDK along"
        status=1
    fi
    [[ "$status" -ne 0 ]] || echo "==> version $current, every copy agrees"
    exit "$status"
fi

if [[ -z "$wanted" ]]; then
    echo "$current"
    exit 0
fi

sed -i "/^\[workspace\.package\]/,/^\[/ s/^version = \"$current\"/version = \"$wanted\"/" Cargo.toml
[[ "$(rb_workspace_version < Cargo.toml)" == "$wanted" ]] || {
    echo "!! could not write $wanted into Cargo.toml" >&2
    exit 1
}
for manifest in "${PACKAGES[@]}"; do
    [[ -e "$manifest" ]] || continue
    sed -i "s/^  \"version\": \".*\",$/  \"version\": \"$wanted\",/" "$manifest"
done
# The SDK's copies: the templates' `min_app_version`, the workflows' pinned release.
for manifest in sdk/templates/*/manifest.toml; do
    [[ -e "$manifest" ]] || continue
    sed -i "s/^min_app_version = \".*\"$/min_app_version = \"$wanted\"/" "$manifest"
done
for workflow in sdk/ci/*.yml; do
    [[ -e "$workflow" ]] || continue
    sed -i "s/^  RBROWSER_VERSION: .*$/  RBROWSER_VERSION: $wanted/" "$workflow"
done
# The lock entries of the workspace crates; offline first, since nothing new has to be fetched.
cargo update --workspace --offline 2> /dev/null || cargo update --workspace
# The plugins build outside the workspace, each with a lock file of its own that repeats the
# version of rb-plugin-api and rb-plugin-types (path dependencies).
for manifest in plugins/*/Cargo.toml; do
    [[ -e "$manifest" && -e "${manifest%Cargo.toml}Cargo.lock" ]] || continue
    cargo update --workspace --offline --manifest-path "$manifest" 2> /dev/null \
        || cargo update --workspace --manifest-path "$manifest"
done
found="$(problems "$wanted")"
if [[ -n "$found" ]]; then
    printf '!! %s\n' "$found" >&2
    exit 1
fi
if [[ "$wanted" != "$current" ]]; then
    rb_changelog_cut CHANGELOG.md "$wanted" "$(date +%F)"
fi
echo "==> version $current -> $wanted (Cargo.toml, Cargo.lock, package.json, sdk/, CHANGELOG.md)"

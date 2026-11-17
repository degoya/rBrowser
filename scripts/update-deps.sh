#!/usr/bin/env bash
#
# Moves every dependency to its newest release (RB-000-02: "latest where possible, pinned
# exactly"), then runs the gate:
#
#   1. Rust: `cargo update` (newest compatible versions into Cargo.lock), then
#      `cargo upgrade --incompatible` (newest versions, majors included, into the Cargo.toml
#      requirements) when cargo-edit is installed - `--pinned` too, so an exact `=x.y.z` moves.
#   2. npm: `npm outdated`, then `npm update` in every front end (ui/, agent/, sidecar/ and the
#      plugins' workspace plugins/) that has a package.json; a major listed by `npm outdated` is
#      for a person to take.
#   3. Hints only, nothing changed: the newest stable Rust against rust-toolchain.toml, the newest
#      stable Node against .nvmrc, and the cef crate (= Chromium) against Cargo.lock. The pins in
#      scripts/lib/tool-versions.sh move by hand, and `scripts/tool-versions.sh --check` then
#      finds every copy that has to follow.
#   4. `scripts/check.sh` (default --full; --check-mode picks another).
#
# Usage:
#   scripts/update-deps.sh                        # all of the above
#   scripts/update-deps.sh --dry-run              # report what is outdated, change nothing
#   scripts/update-deps.sh --no-check             # update, but skip scripts/check.sh
#   scripts/update-deps.sh --check-mode --rust    # run `scripts/check.sh --rust` at the end
#
# Takes no build lock itself (cargo update resolves, it compiles nothing); scripts/check.sh takes
# it. Needs the network.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/tool-versions.sh
source "$ROOT/scripts/lib/tool-versions.sh"
cd "$ROOT"

usage() {
    echo "usage: scripts/update-deps.sh [--dry-run] [--no-check] [--check-mode <check.sh mode>]" >&2
    exit 2
}

dry=0
check=1
check_mode=--full
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) dry=1 ;;
        --no-check) check=0 ;;
        --check-mode)
            [[ $# -ge 2 ]] || usage
            check_mode="$2"
            shift
            ;;
        -h | --help) sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) usage ;;
    esac
    shift
done

echo "==> Rust dependencies"
if [[ "$dry" -eq 1 ]]; then
    cargo update --dry-run
else
    cargo update
fi
if cargo upgrade --version > /dev/null 2>&1; then
    if [[ "$dry" -eq 1 ]]; then
        cargo upgrade --incompatible --pinned --dry-run
    else
        cargo upgrade --incompatible --pinned
    fi
else
    echo "note: cargo-edit is not installed, so incompatible (major) upgrades are not applied;"
    echo "      cargo install cargo-edit --locked --version $RB_CARGO_EDIT_VERSION"
fi

echo
echo "==> npm packages"
shopt -s nullglob
packages=0
for manifest in ui/package.json agent/package.json sidecar/package.json plugins/package.json; do
    [[ -f "$manifest" ]] || continue
    dir="$(dirname "$manifest")"
    packages=$((packages + 1))
    echo "--> $dir"
    if ! command -v npm > /dev/null 2>&1; then
        echo "!! npm is not installed; Node $(rb_node_version "$ROOT") is pinned in .nvmrc" >&2
        exit 1
    fi
    # `npm outdated` exits 1 when anything is outdated; that is its report, not a failure.
    (cd "$dir" && npm outdated) || true
    [[ "$dry" -eq 1 ]] || (cd "$dir" && npm update)
done
[[ "$packages" -gt 0 ]] || echo "    no front end with a package.json yet"

echo
echo "==> toolchain"
pinned_rust="$(rb_rust_version "$ROOT")"
newest_rust="$(rustup check 2> /dev/null | sed -n 's/^stable-[^ ]* - .* -> \([0-9.]*\) .*/\1/p; s/^stable-[^ ]* - Up to date : \([0-9.]*\) .*/\1/p' | head -n 1 || true)"
if [[ -z "$newest_rust" ]]; then
    echo "    Rust: rust-toolchain.toml pins $pinned_rust; \`rustup check\` named no stable release"
elif [[ "$newest_rust" != "$pinned_rust" ]]; then
    echo "    Rust: $newest_rust is out, rust-toolchain.toml pins $pinned_rust - bump \`channel\`"
else
    echo "    Rust: $pinned_rust, the newest stable"
fi
pinned_node="$(rb_node_version "$ROOT")"
newest_node="$(curl -fsS https://nodejs.org/dist/index.json 2> /dev/null \
    | python3 -I -c 'import json, sys
for release in json.load(sys.stdin):
    if "-" not in release["version"]:
        print(release["version"].lstrip("v")); break' 2> /dev/null || true)"
if [[ -n "$newest_node" && "$newest_node" != "$pinned_node" ]]; then
    echo "    Node: $newest_node is the newest stable release, .nvmrc pins $pinned_node - bump it (and the packaged runtime)"
else
    echo "    Node: .nvmrc pins $pinned_node${newest_node:+, the newest stable release}"
fi
cef="$(awk '/^name = "cef"$/ { getline; gsub(/"/, "", $3); print $3; exit }' Cargo.lock 2> /dev/null || true)"
if [[ -n "$cef" ]]; then
    echo "    CEF: cef $cef in Cargo.lock; \`cargo search cef --limit 1\` names the newest (= Chromium)"
    echo "    CEF: a new cef crate brings a new archive; move RB_CEF_* in scripts/lib/tool-versions.sh with it"
fi
echo "    tools: scripts/tool-versions.sh --installed lists pins and installs; the pins are in"
echo "           scripts/lib/tool-versions.sh"

if [[ "$dry" -eq 1 || "$check" -eq 0 ]]; then
    echo
    echo "==> done (scripts/check.sh not run)"
    exit 0
fi
echo
echo "==> scripts/check.sh $check_mode"
exec "$ROOT/scripts/check.sh" "$check_mode"

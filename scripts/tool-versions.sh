#!/usr/bin/env bash
#
# The development tools' pins, one source each (RB-000-02): Rust in rust-toolchain.toml, Node in
# .nvmrc, cargo-nextest, cargo-xwin, cargo-deny, cargo-edit, shellcheck, gitleaks, actionlint and wix
# (and the packaged Node runtime's SHA-256s for Windows and macOS, rcedit) in
# scripts/lib/tool-versions.sh. Scripts read those; a document, a package.json or a workflow
# cannot, so `--check` holds every copy to its source, as `scripts/set-version.sh --check` does
# for the application's version.
#
# A copy is a line one of the rules below matches. A `required` rule that matches nothing fails:
# the copy moved, and the rule has to follow it.
#
# Usage:
#   scripts/tool-versions.sh                 # every tool and its pinned version
#   scripts/tool-versions.sh --installed     # every tool, its pin and what is installed here
#   scripts/tool-versions.sh --check         # every copy agrees with its source, or exit 1
#   scripts/tool-versions.sh --check --root DIR
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() { echo "usage: scripts/tool-versions.sh [--installed | --check [--root DIR]]" >&2; exit 2; }

mode=list
tree="$ROOT"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --check) mode=check ;;
        --installed) mode=installed ;;
        --root) tree="${2:?--root needs a directory}"; shift ;;
        -h | --help) sed -n '2,17p' "$0"; exit 0 ;;
        *) usage ;;
    esac
    shift
done

# The sources, read from the tree under check.
# shellcheck source=lib/tool-versions.sh
source "$tree/scripts/lib/tool-versions.sh"
declare -A pin=(
    [rust]="$(rb_rust_version "$tree")"
    [node]="$(rb_node_version "$tree")"
    [cargo-nextest]="$RB_NEXTEST_VERSION"
    [cargo-xwin]="$RB_CARGO_XWIN_VERSION"
    [cargo-deny]="$RB_CARGO_DENY_VERSION"
    [cargo-edit]="$RB_CARGO_EDIT_VERSION"
    [shellcheck]="$RB_SHELLCHECK_VERSION"
    [gitleaks]="$RB_GITLEAKS_VERSION"
    [actionlint]="$RB_ACTIONLINT_VERSION"
    [wix]="$RB_WIX_VERSION"
)
pin[node-major]="${pin[node]%%.*}"
# The WebKit pin (RB-040-04) and its copy in the sidecar, which reads it at run time.
pin[webkit-playwright]="$RB_WEBKIT_PLAYWRIGHT_VERSION"
pin[webkit-revision]="$RB_WEBKIT_REVISION"
pin[webkit-sha256]="$RB_WEBKIT_ARCHIVE_SHA256"
pin[webkit-bytes]="$RB_WEBKIT_ARCHIVE_BYTES"
order=(rust node cargo-nextest cargo-xwin cargo-deny cargo-edit shellcheck gitleaks actionlint wix)

# The version of tool $1 installed here, or `-`.
installed() {
    local out=""
    case "$1" in
        rust) out="$(rustc --version 2> /dev/null)" ;;
        node) out="$(node --version 2> /dev/null)" ;;
        cargo-nextest) out="$(cargo nextest --version 2> /dev/null | head -n 1)" ;;
        cargo-xwin) out="$(cargo xwin --version 2> /dev/null)" ;;
        cargo-deny) out="$(cargo deny --version 2> /dev/null)" ;;
        cargo-edit) out="$(cargo upgrade --version 2> /dev/null)" ;;
        shellcheck) out="$(shellcheck --version 2> /dev/null | sed -n 's/^version: //p')" ;;
        gitleaks) out="$(gitleaks version 2> /dev/null)" ;;
        actionlint) out="$(actionlint --version 2> /dev/null | head -n 1)" ;;
        wix) out="$(wix --version 2> /dev/null)" ;;
    esac
    grep -oE '[0-9]+\.[0-9]+\.[0-9]+' <<< "$out" | head -n 1 || echo -
}

case "$mode" in
    list)
        for tool in "${order[@]}"; do printf '%-14s %s\n' "$tool" "${pin[$tool]}"; done
        printf '%-14s %s (released %s, archive SHA-256 %s)\n' cef "$RB_CEF_VERSION" \
            "$RB_CEF_RELEASE_DATE" "$RB_CEF_ARCHIVE_SHA256"
        printf '%-14s %s (packaged runtime, ZIP SHA-256 %s)\n' node-win-x64 "$RB_NODE_WIN_X64_VERSION" \
            "$RB_NODE_WIN_X64_SHA256"
        printf '%-14s %s (macOS bundle, tar.gz SHA-256 arm64 %s, x64 %s)\n' node-darwin \
            "$RB_NODE_DARWIN_VERSION" "$RB_NODE_DARWIN_ARM64_SHA256" "$RB_NODE_DARWIN_X64_SHA256"
        printf '%-14s %s (packaged runtime, tarball SHA-256 %s)\n' node-linux-x64 \
            "$RB_NODE_LINUX_X64_VERSION" "$RB_NODE_LINUX_X64_SHA256"
        printf '%-14s %s (linux64 archive SHA-256 %s)\n' cef-linux "$RB_CEF_VERSION" \
            "$RB_CEF_ARCHIVE_LINUX_X64_SHA256"
        printf '%-14s %s (package step, SHA-256 %s)\n' rcedit "$RB_RCEDIT_VERSION" "$RB_RCEDIT_SHA256"
        printf '%-14s %s (playwright-core %s, win64 archive SHA-256 %s)\n' webkit "$RB_WEBKIT_REVISION" \
            "$RB_WEBKIT_PLAYWRIGHT_VERSION" "$RB_WEBKIT_ARCHIVE_SHA256"
        exit 0
        ;;
    installed)
        printf '%-14s %-10s %s\n' tool pinned installed
        for tool in "${order[@]}"; do
            printf '%-14s %-10s %s\n' "$tool" "${pin[$tool]}" "$(installed "$tool")"
        done
        # The CEF cache: the pinned archive, held to its SHA-256 (scripts/lib/cef.sh).
        # shellcheck source=lib/cef.sh
        source "$tree/scripts/lib/cef.sh"
        cef_state="-"
        if rb_cef_verify "$(rb_cef_path)" 2> /dev/null; then cef_state="$RB_CEF_VERSION (SHA-256 ok)"; fi
        printf '%-14s %-10s %s\n' cef "$RB_CEF_VERSION" "$cef_state"
        exit 0
        ;;
esac

V='([0-9]+\.[0-9]+\.[0-9]+)'
copies=0
wrong=0

# The copies of tool $1 that extended regex $3 (the version as its last group) matches in files
# $4...: each must say the tool's pin. $2 is what it takes when none matches: `required` fails,
# `any` passes.
copies_of() {
    local tool="$1" need="$2" regex="$3" expected="${pin[$1]}" file hit value found=0 groups
    shift 3
    groups="${regex//[^(]/}"
    if [[ -z "$expected" ]]; then
        echo "!! no ${tool} version at its source" >&2
        wrong=$((wrong + 1))
        return
    fi
    for file in "$@"; do
        [[ -f "$file" ]] || continue
        while IFS= read -r hit; do
            found=$((found + 1))
            value="$(sed -E "s/.*${regex}.*/\\${#groups}/" <<< "${hit#*:}")"
            if [[ "$value" != "$expected" ]]; then
                echo "!! ${file}:${hit%%:*}: ${tool} ${value}, the source says ${expected}" >&2
                wrong=$((wrong + 1))
            fi
        done < <(grep -nE -- "$regex" "$file" || true)
    done
    copies=$((copies + found))
    if [[ "$found" -eq 0 && "$need" == required ]]; then
        echo "!! no copy of ${tool} matches /${regex}/: the copy moved, and its rule in scripts/tool-versions.sh follows it" >&2
        wrong=$((wrong + 1))
    fi
}

cd "$tree"
shopt -s nullglob
WORKFLOWS=(.github/workflows/*.yml .github/actions/*/action.yml)
PACKAGES=(ui/package.json agent/package.json sidecar/package.json plugins/*/package.json)
# AGENTS.md's Toolchain table is the working card's, which the public export leaves out
# (scripts/public-exclude.txt, RB-090-12): there the other copies are held, and the table not.
if [[ -f AGENTS.md ]]; then
    for tool in "${order[@]}"; do
        copies_of "$tool" required "^\\| ${tool/rust/Rust} \\| $V \\|" AGENTS.md
    done
else
    echo "note: no AGENTS.md in this tree (the public export); its Toolchain table is not checked"
fi
copies_of node any "\"node\": \">=$V\"" "${PACKAGES[@]}"
copies_of node-major any "node-version: ([0-9]+)$" "${WORKFLOWS[@]}"
# The SDK's copies (RB-080-87): the Node its CI builds a template's panel with, and the panel's pin.
copies_of node required "^  NODE_VERSION: $V$" sdk/ci/plugin.yml
copies_of node any "\"node\": \"$V\"" sdk/templates/*/web/package.json
copies_of rust any "toolchain: $V" "${WORKFLOWS[@]}"
copies_of cargo-nextest any "tool: (cargo-)?nextest@$V" "${WORKFLOWS[@]}"
copies_of cargo-xwin any "tool: cargo-xwin@$V" "${WORKFLOWS[@]}"
copies_of cargo-deny any "tool: cargo-deny@$V" "${WORKFLOWS[@]}"
# WiX (RB-099-27) is installed by scripts/ci-tools.sh wix from the pin; a workflow, a document or
# a script that names the tool or its UI extension at a version of its own holds that version.
WIX_TEXTS=("${WORKFLOWS[@]}" README.md docs/*.md scripts/*.sh scripts/lib/*.sh)
copies_of wix any "tool (install|update) --global wix --version $V" "${WIX_TEXTS[@]}"
# (`.` for the `/` before the version: the rule is also a sed expression.)
copies_of wix any "WixToolset\\.[A-Za-z]+\\.wixext.$V" "${WIX_TEXTS[@]}"

# The WebKit pin's copy in the sidecar (sidecar/src/webkit/pin.ts) and the playwright-core it is for.
WEBKIT_PIN_TS=sidecar/src/webkit/pin.ts
copies_of webkit-playwright required "^  playwright: '$V',$" "$WEBKIT_PIN_TS"
copies_of webkit-playwright required "\"playwright-core\": \"$V\"" sidecar/package.json
copies_of webkit-revision required "^  revision: '([0-9]+)',$" "$WEBKIT_PIN_TS"
copies_of webkit-sha256 required "^  sha256: '([0-9a-f]{64})',$" "$WEBKIT_PIN_TS"
copies_of webkit-bytes required "^  bytes: ([0-9_]+),$" "$WEBKIT_PIN_TS"
# The macOS archives (RB-097-02), one line per host in `MAC_PINS`.
copies_of webkit-playwright required "^  '?mac[0-9]+(-arm64)?'?: \\{ playwright: '$V'," "$WEBKIT_PIN_TS"
for host in mac14 mac14-arm64 mac15 mac15-arm64 mac26 mac26-arm64; do
    var="RB_WEBKIT_${host^^}"
    var="${var//-/_}"
    sha="${var}_SHA256"
    bytes="${var}_BYTES"
    pin[webkit-$host-sha256]="${!sha:-}"
    pin[webkit-$host-bytes]="${!bytes:-}"
    line="^  '?${host}'?: \\{ playwright: '[0-9.]+', revision: '[0-9]+', archive: 'webkit-${host/mac/mac-}\\.zip', "
    copies_of "webkit-$host-sha256" required "${line}sha256: '([0-9a-f]{64})'," "$WEBKIT_PIN_TS"
    copies_of "webkit-$host-bytes" required "${line}sha256: '[0-9a-f]{64}', bytes: ([0-9_]+) \\}" "$WEBKIT_PIN_TS"
done

# The packaged Node runtime's SHA-256 belongs to one version, which has to be .nvmrc's.
if [[ "${RB_NODE_WIN_X64_VERSION:-}" != "${pin[node]}" ]]; then
    echo "!! scripts/lib/tool-versions.sh: RB_NODE_WIN_X64_SHA256 is for node ${RB_NODE_WIN_X64_VERSION:-(none)}, .nvmrc says ${pin[node]} - pin the new ZIP's SHA-256" >&2
    wrong=$((wrong + 1))
fi
# The same for the macOS bundle's runtime (RB-097-01): one version for both architectures, and
# each pin a SHA-256, never a placeholder.
if [[ "${RB_NODE_DARWIN_VERSION:-}" != "${pin[node]}" ]]; then
    echo "!! scripts/lib/tool-versions.sh: RB_NODE_DARWIN_ARM64_SHA256 and RB_NODE_DARWIN_X64_SHA256 are for node ${RB_NODE_DARWIN_VERSION:-(none)}, .nvmrc says ${pin[node]} - pin the new tarballs' SHA-256s" >&2
    wrong=$((wrong + 1))
fi
for name in RB_NODE_DARWIN_ARM64_SHA256 RB_NODE_DARWIN_X64_SHA256; do
    if [[ ! "${!name:-}" =~ ^[0-9a-f]{64}$ ]]; then
        echo "!! scripts/lib/tool-versions.sh: $name is no SHA-256 (${!name:-empty}) - take it from https://nodejs.org/dist/v${pin[node]}/SHASUMS256.txt" >&2
        wrong=$((wrong + 1))
    fi
done
# The same for the Linux package's runtime (RB-097-05).
if [[ "${RB_NODE_LINUX_X64_VERSION:-}" != "${pin[node]}" ]]; then
    echo "!! scripts/lib/tool-versions.sh: RB_NODE_LINUX_X64_SHA256 is for node ${RB_NODE_LINUX_X64_VERSION:-(none)}, .nvmrc says ${pin[node]} - pin the new tarball's SHA-256" >&2
    wrong=$((wrong + 1))
fi

if [[ "$wrong" -gt 0 ]]; then
    echo "!! ${wrong} tool pin(s) disagree with their source (scripts/lib/tool-versions.sh, rust-toolchain.toml, .nvmrc)" >&2
    exit 1
fi
echo "    ${copies} copies of ${#order[@]} tool pins agree with their sources"

#!/usr/bin/env bash
#
# The package managers' files of one release (RB-090-13, RB-110-04), rendered from packaging/ with the
# version and the archives' SHA-256 from the release's SHA256SUMS: the Scoop bucket's manifest,
# the three winget manifests and the Homebrew tap's cask (macOS) and formula (Linux), each
# repository with its README. Adapted from rCommander's scripts/package-managers.sh.
#
# Usage:
#   scripts/package-managers.sh <version> <SHA256SUMS> <outdir> [--repository OWNER/NAME]
#                               [--base-url URL]
#   scripts/package-managers.sh --check [--sums <SHA256SUMS>] [--repository OWNER/NAME]
#
# Writes the files as each repository holds them:
#   <outdir>/scoop/bucket/rbrowser.json, README.md
#   <outdir>/winget/degoya.rBrowser.yaml, .installer.yaml, .locale.en-US.yaml
#   <outdir>/homebrew/Casks/rbrowser.rb, Formula/rbrowser.rb, README.md
# The release workflow pushes the bucket and the tap after the release is published and submits
# the winget manifests as a pull request to microsoft/winget-pkgs.
#
# Unlike rCommander, a channel is rendered only when the release carries its archives: phase 1
# ships the Windows ZIP alone, the Linux tarball and the macOS ZIPs arrive with RB-097-05 and
# RB-110-02. The Windows ZIP is required (Scoop and winget); the cask needs both macOS ZIPs and
# the formula the Linux tarball, and one that is missing is skipped with a note. A channel with
# only part of its archives (one macOS ZIP of two) is an incomplete release and refused.
#
#   --repository   the GitHub repository whose releases they install, default degoya/rBrowser;
#                  the tap and the bucket are <owner>/homebrew-rbrowser and <owner>/scoop-rbrowser
#   --base-url     where the archives are downloaded from, default
#                  https://github.com/<repository>/releases/download/v<version>
#
# --check holds the templates against the last release: it renders them with the version and
# the SHA256SUMS of the repository's latest release (downloaded, or the file --sums names, its
# version read from the Windows ZIP's name) into a temporary folder.
#
# Every archive must be in SHA256SUMS (`<sha256>  <name>`, as scripts/release-assets.sh writes it,
# or `<sha256>  ./<name>`), and nothing of a template may stay unreplaced; a failure writes nothing.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/render-template.sh
source "$ROOT/scripts/lib/render-template.sh"
# Bash 5.2 would put the matched text in place of a `&` in a substitution's replacement.
shopt -u patsub_replacement 2> /dev/null || true

usage() {
    echo "usage: $0 <version> <SHA256SUMS> <outdir> [--repository OWNER/NAME] [--base-url URL]" >&2
    echo "       $0 --check [--sums <SHA256SUMS>] [--repository OWNER/NAME]" >&2
    exit 2
}

valid_repository() {
    if [[ ! "$1" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]]; then
        echo "error: '$1' is not OWNER/NAME" >&2
        exit 1
    fi
}

if [[ "${1:-}" == --check ]]; then
    shift
    sums=""
    repository="degoya/rBrowser"
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --sums) [[ $# -ge 2 ]] || usage; sums="$2"; shift 2 ;;
            --repository) [[ $# -ge 2 ]] || usage; repository="$2"; shift 2 ;;
            *) usage ;;
        esac
    done
    valid_repository "$repository"
    scratch="$(mktemp -d)"
    trap 'rm -rf "$scratch"' EXIT
    if [[ -z "$sums" ]]; then
        sums="$scratch/SHA256SUMS"
        url="https://github.com/$repository/releases/latest/download/SHA256SUMS"
        if ! curl --fail --silent --show-error --location --retry 3 --output "$sums" "$url"; then
            echo "error: the last release's SHA256SUMS could not be downloaded ($url)" >&2
            exit 1
        fi
    fi
    [[ -f "$sums" ]] || { echo "error: $sums does not exist" >&2; exit 1; }
    version="$(sed -n 's#^[0-9a-fA-F]\{64\}  *[*]\{0,1\}\(\./\)\{0,1\}rbrowser-\(.*\)-windows-x64\.zip$#\2#p' "$sums" | head -n 1)"
    if [[ -z "$version" ]]; then
        echo "error: $sums names no rbrowser-<version>-windows-x64.zip; its version is unknown" >&2
        exit 1
    fi
    if "$0" "$version" "$sums" "$scratch/out" --repository "$repository" > "$scratch/log" 2>&1; then
        echo "the templates render against the last release, $version: $(grep -c '^wrote ' "$scratch/log") files"
        exit 0
    fi
    cat "$scratch/log" >&2
    echo "error: the templates do not render against the last release, $version" >&2
    exit 1
fi

[[ $# -ge 3 ]] || usage
version="${1#v}"
sums="$2"
outdir="$3"
shift 3
repository="degoya/rBrowser"
base_url=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --repository) [[ $# -ge 2 ]] || usage; repository="$2"; shift 2 ;;
        --base-url) [[ $# -ge 2 ]] || usage; base_url="$2"; shift 2 ;;
        *) usage ;;
    esac
done

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
    echo "error: '$version' is not a release version (X.Y.Z or X.Y.Z-suffix)" >&2
    exit 1
fi
valid_repository "$repository"
[[ -f "$sums" ]] || { echo "error: $sums does not exist" >&2; exit 1; }
owner="${repository%%/*}"
base_url="${base_url:-https://github.com/${repository}/releases/download/v${version}}"
base_url="${base_url%/}"

# The hash of archive $1 in SHA256SUMS, lower case; nothing when it is not there. By awk, so it
# runs under macOS's /bin/bash 3.2 too.
hash_of() {
    awk -v wanted="$1" '{
        name = $2
        sub(/^\*/, "", name)
        sub(/^\.\//, "", name)
        if (name == wanted) { print tolower($1); exit }
    }' "$sums"
}

values=(
    "VERSION=$version"
    "REPOSITORY=$repository"
    "OWNER=$owner"
    "BASE_URL=$base_url"
    "TAP=${owner}/homebrew-rbrowser"
    "BUCKET=${owner}/scoop-rbrowser"
)

# Adds the placeholders of one channel's archives (pairs PLACEHOLDER=archive) to `values`.
# Returns 0 when all are there, 1 when none is, and fails the run when only some are.
channel() {
    local name="$1" pair hash found=() lacking=()
    shift
    for pair in "$@"; do
        hash="$(hash_of "${pair#*=}")"
        if [[ "$hash" =~ ^[0-9a-f]{64}$ ]]; then
            found+=("${pair%%=*}=$hash")
        else
            lacking+=("${pair#*=}")
        fi
    done
    if [[ "${#lacking[@]}" -eq 0 ]]; then
        values+=("${found[@]}")
        return 0
    fi
    if [[ "${#found[@]}" -eq 0 ]]; then
        return 1
    fi
    echo "error: $sums has no SHA-256 for ${lacking[*]}, but the other archives of the $name" >&2
    exit 1
}

if ! channel "Scoop and winget" SHA256_WINDOWS_X64="rbrowser-$version-windows-x64.zip"; then
    echo "error: $sums has no SHA-256 for rbrowser-$version-windows-x64.zip" >&2
    exit 1
fi
# winget-pkgs writes InstallerSha256 in upper case.
windows_hash="$(hash_of "rbrowser-$version-windows-x64.zip")"
values+=("SHA256_WINDOWS_X64_UPPER=$(printf '%s' "$windows_hash" | tr 'a-f' 'A-F')")
cask=0
formula=0
if channel "Homebrew cask" \
    SHA256_MACOS_AARCH64="rbrowser-$version-macos-aarch64.zip" \
    SHA256_MACOS_X86_64="rbrowser-$version-macos-x86_64.zip"; then
    cask=1
fi
if channel "Homebrew formula" SHA256_LINUX_X64="rbrowser-$version-linux-x64.tar.gz"; then
    formula=1
fi

render() {
    mkdir -p "$(dirname "$2")"
    rb_render_template "$1" "$2" "${values[@]}" || exit 1
    echo "wrote $2"
}

packaging="$ROOT/packaging"
render "$packaging/scoop/rbrowser.json.in" "$outdir/scoop/bucket/rbrowser.json"
render "$packaging/scoop/README.md.in" "$outdir/scoop/README.md"
for manifest in degoya.rBrowser.yaml degoya.rBrowser.installer.yaml \
    degoya.rBrowser.locale.en-US.yaml; do
    render "$packaging/winget/$manifest.in" "$outdir/winget/$manifest"
done
if [[ "$cask" -eq 1 ]]; then
    render "$packaging/homebrew/rbrowser-cask.rb.in" "$outdir/homebrew/Casks/rbrowser.rb"
else
    echo "skip the Homebrew cask: the release has no macOS archives (RB-110-02)"
fi
if [[ "$formula" -eq 1 ]]; then
    render "$packaging/homebrew/rbrowser.rb.in" "$outdir/homebrew/Formula/rbrowser.rb"
else
    echo "skip the Homebrew formula: the release has no Linux archive (RB-097-05)"
fi
if [[ "$cask" -eq 1 || "$formula" -eq 1 ]]; then
    render "$packaging/homebrew/README.md.in" "$outdir/homebrew/README.md"
fi

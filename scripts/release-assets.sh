#!/usr/bin/env bash
#
# The release assets of release.yml (RB-090-04, after rCommander's script of the same name):
#
#   scripts/release-assets.sh sums <dir>                    # SHA256SUMS over the files in <dir>
#   scripts/release-assets.sh notes <version> [CHANGELOG]   # the version's CHANGELOG.md section
#
# `sums` lists every file of the folder but SHA256SUMS itself and its Sigstore bundle, by name, so
# `sha256sum -c SHA256SUMS` in the unpacked download checks them. `notes` prints the body of
# `## [<version>] - <date>`; a pre-release (`1.1.0-beta.2`) without a section of its own takes its
# release's (`1.1.0`). A version without either fails: a release says what it brings.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
    echo "usage: scripts/release-assets.sh sums <dir> | notes <version> [CHANGELOG.md]" >&2
    exit 2
}

sums() {
    local dir="$1" files=()
    [[ -d "$dir" ]] || { echo "!! $dir is not a folder" >&2; return 1; }
    mapfile -t files < <(cd "$dir" && find . -maxdepth 1 -type f ! -name 'SHA256SUMS' \
        ! -name 'SHA256SUMS.*' -printf '%f\n' | LC_ALL=C sort)
    [[ ${#files[@]} -gt 0 ]] || { echo "!! no files in $dir" >&2; return 1; }
    (cd "$dir" && sha256sum -- "${files[@]}") > "$dir/SHA256SUMS"
    echo "    SHA256SUMS over ${#files[@]} file(s) in $dir"
}

# The body of section `## [$1] - ...` of changelog $2, without blank lines at its ends.
section() {
    awk -v heading="## [$1]" '
        index($0, heading " ") == 1 || $0 == heading { found = 1; next }
        found && /^## / { exit }
        found { print }
    ' "$2" | sed -e '/./,$!d' | sed -e ':a' -e '/^\n*$/{$d;N;ba' -e '}'
}

notes() {
    local version="$1" changelog="$2" body
    [[ -f "$changelog" ]] || { echo "!! no $changelog" >&2; return 1; }
    for candidate in "$version" "${version%%-*}"; do
        body="$(section "$candidate" "$changelog")"
        if [[ -n "${body//[[:space:]]/}" ]]; then
            printf '%s\n' "$body"
            return 0
        fi
    done
    echo "!! $changelog has no section ## [$version] (nor ## [${version%%-*}])" >&2
    return 1
}

case "${1:-}" in
    sums) [[ $# -eq 2 ]] || usage; sums "$2" ;;
    notes) [[ $# -ge 2 && $# -le 3 ]] || usage; notes "$2" "${3:-$ROOT/CHANGELOG.md}" ;;
    *) usage ;;
esac

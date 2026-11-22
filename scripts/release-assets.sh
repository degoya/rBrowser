#!/usr/bin/env bash
#
# The release assets of release.yml (RB-090-04, after rCommander's script of the same name):
#
#   scripts/release-assets.sh sums <dir>                    # SHA256SUMS over the files in <dir>
#   scripts/release-assets.sh notes <version> [CHANGELOG]   # the version's CHANGELOG.md section
#   scripts/release-assets.sh complete <dir> <version>      # every package is in <dir>
#   scripts/release-assets.sh downloads <version>           # the release text on the packages
#
# `sums` lists every file of the folder but SHA256SUMS itself and its Sigstore bundle, by name, so
# `sha256sum -c SHA256SUMS` in the unpacked download checks them. `notes` prints the body of
# `## [<version>] - <date>`; a pre-release (`1.1.0-beta.2`) without a section of its own takes its
# release's (`1.1.0`). A version without either fails: a release says what it brings.
# `complete` refuses a folder without one of the release's packages - the Windows ZIP and, per
# macOS architecture, the ZIP and the DMG of scripts/package-macos.sh (RB-090-80), and the Linux
# tarball, deb and rpm of scripts/package-linux.sh (RB-097-05) - so a release
# never goes out with a platform missing. `downloads` is the release text's last part: which
# package is whose, and how macOS lets an app start that is signed ad hoc and not notarised.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
    echo "usage: scripts/release-assets.sh sums <dir> | notes <version> [CHANGELOG.md]" \
        "| complete <dir> <version> | downloads <version>" >&2
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

# The packages of release $1, by name.
packages() {
    local name
    for name in windows-x64.zip macos-aarch64.zip macos-aarch64.dmg macos-x86_64.zip \
        macos-x86_64.dmg linux-x64.tar.gz linux-x64.deb linux-x64.rpm; do
        printf 'rbrowser-%s-%s\n' "$1" "$name"
    done
}

complete() {
    local dir="$1" version="$2" name missing=() count=0
    [[ -d "$dir" ]] || { echo "!! $dir is not a folder" >&2; return 1; }
    while IFS= read -r name; do
        if [[ -s "$dir/$name" ]]; then count=$((count + 1)); else missing+=("$name"); fi
    done < <(packages "$version")
    if [[ ${#missing[@]} -gt 0 ]]; then
        echo "!! the release is incomplete; missing in $dir: ${missing[*]}" >&2
        return 1
    fi
    echo "    $count package(s) of rBrowser $version in $dir"
}

downloads() {
    local version="$1"
    cat <<TEXT

## Downloads

- **Windows** (x64): \`rbrowser-$version-windows-x64.zip\` - unpack it into a folder of your own
  and start \`rbrowser.exe\`; it is not code-signed yet, so SmartScreen asks once (More info, Run
  anyway).
- **macOS 13 or later**: \`rbrowser-$version-macos-aarch64.dmg\` for Apple Silicon,
  \`rbrowser-$version-macos-x86_64.dmg\` for Intel Macs, each also as a ZIP; drag rBrowser into
  Applications. The app is signed ad hoc, not notarised, so macOS refuses its first start: allow
  it under System Settings > Privacy & Security > Open Anyway (on macOS 14 and earlier a
  right-click on the app and Open does the same), or remove the quarantine flag once with
  \`xattr -dr com.apple.quarantine /Applications/rBrowser.app\`.
- **Linux** (x64): \`rbrowser-$version-linux-x64.deb\` or \`.rpm\` installs into /usr/lib/rbrowser with
  a menu entry and the Chromium sandbox set up; \`rbrowser-$version-linux-x64.tar.gz\` unpacks
  anywhere (\`./rbrowser\`; see its README.txt for the sandbox helper and the libraries).
- \`SHA256SUMS\` lists every file's checksum (\`sha256sum -c SHA256SUMS\`); its Sigstore signature
  is \`SHA256SUMS.sigstore.json\`.
TEXT
}

case "${1:-}" in
    sums) [[ $# -eq 2 ]] || usage; sums "$2" ;;
    notes) [[ $# -ge 2 && $# -le 3 ]] || usage; notes "$2" "${3:-$ROOT/CHANGELOG.md}" ;;
    complete) [[ $# -eq 3 ]] || usage; complete "$2" "$3" ;;
    downloads) [[ $# -eq 2 ]] || usage; downloads "$2" ;;
    *) usage ;;
esac

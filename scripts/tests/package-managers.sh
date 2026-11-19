#!/usr/bin/env bash
#
# scripts/package-managers.sh against a SHA256SUMS in the release's format (RB-090-13, RB-110-04): a release
# with the Windows ZIP only (phase 1) renders the Scoop and winget files and skips the Homebrew
# tap with a note; a release with every archive renders the cask and the formula too, each hash
# beside its own URL; the Scoop manifest is JSON that unpacks the ZIP's version folder, writes the
# install-kind marker and keeps no persist; the three winget manifests name the portable ZIP, its
# upper-case hash and the program in the version folder; a release without the Windows ZIP, or
# with half of a channel's archives, renders nothing; `--check` renders against a SHA256SUMS file.
# Adapted from rCommander's scripts/tests/package-managers.sh.
#
#   scripts/tests/package-managers.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

render() { "$ROOT/scripts/package-managers.sh" "$@"; }
# 64 times the character $1.
hash_of() { printf "$1%.0s" {1..64}; }
# The line after the one naming $2 in file $1: the sha256 under a url.
line_after() { awk -v needle="$2" 'found { print; exit } index($0, needle) { found = 1 }' "$1"; }
# One field of a JSON file, by a Python expression over `m`.
field() { python3 -I -c "import json, sys; m = json.load(open(sys.argv[1])); print($2)" "$1"; }
# The files of tree $1, sorted, on one line.
files_of() { (cd "$1" && find . -type f | sed 's#^\./##' | LC_ALL=C sort | tr '\n' ' ' | sed 's/ $//'); }

# Phase 1: the release's SHA256SUMS as scripts/release-assets.sh writes it, the Windows ZIP only.
windows_sums() { printf '%s  %s\n' "$(hash_of e)" "rbrowser-$1-windows-x64.zip"; }
# Phase 2: every archive.
all_sums() {
    printf '%s  %s\n' \
        "$(hash_of b)" "rbrowser-$1-linux-x64.tar.gz" \
        "$(hash_of c)" "rbrowser-$1-macos-aarch64.zip" \
        "$(hash_of d)" "rbrowser-$1-macos-x86_64.zip" \
        "$(hash_of e)" "rbrowser-$1-windows-x64.zip"
}
base="https://github.com/degoya/rBrowser/releases/download/v1.1.0"

# --- phase 1: Windows only ------------------------------------------------------------------
windows_sums 1.1.0 > "$SCRATCH/windows-sums"
run_status render v1.1.0 "$SCRATCH/windows-sums" "$SCRATCH/win"
expect_status "a Windows-only release renders" 0
expect "Scoop and winget, no Homebrew" \
    "scoop/README.md scoop/bucket/rbrowser.json winget/degoya.rBrowser.installer.yaml winget/degoya.rBrowser.locale.en-US.yaml winget/degoya.rBrowser.yaml" \
    "$(files_of "$SCRATCH/win")"
expect_output "the cask is skipped with a note" "skip the Homebrew cask: the release has no macOS archives"
expect_output "and the formula" "skip the Homebrew formula: the release has no Linux archive"

# --- phase 2: every archive -----------------------------------------------------------------
all_sums 1.1.0 > "$SCRATCH/SHA256SUMS"
run_status render v1.1.0 "$SCRATCH/SHA256SUMS" "$SCRATCH/out"
expect_status "a release with every archive renders" 0
out="$SCRATCH/out"
expect "the files as each repository holds them" \
    "homebrew/Casks/rbrowser.rb homebrew/Formula/rbrowser.rb homebrew/README.md scoop/README.md scoop/bucket/rbrowser.json winget/degoya.rBrowser.installer.yaml winget/degoya.rBrowser.locale.en-US.yaml winget/degoya.rBrowser.yaml" \
    "$(files_of "$out")"
expect "no placeholder is left anywhere" "0" \
    "$(cat "$out"/homebrew/*/* "$out"/scoop/bucket/* "$out"/winget/* "$out"/*/README.md | grep -c '@[A-Z0-9_]*@' || true)"
expect "the Windows files do not depend on the other archives" "" \
    "$(diff -r "$SCRATCH/win/scoop" "$out/scoop"; diff -r "$SCRATCH/win/winget" "$out/winget")"

# Homebrew: the cask for macOS, the formula for Linux.
cask="$out/homebrew/Casks/rbrowser.rb"
formula="$out/homebrew/Formula/rbrowser.rb"
expect "the cask's version, without the tag's v" '  version "1.1.0"' "$(grep '^  version ' "$cask")"
expect "cask, Apple Silicon: its archive and its hash" "    sha256 \"$(hash_of c)\"" \
    "$(line_after "$cask" "url \"$base/rbrowser-1.1.0-macos-aarch64.zip\"")"
expect "cask, Intel: its archive and its hash" "    sha256 \"$(hash_of d)\"" \
    "$(line_after "$cask" "url \"$base/rbrowser-1.1.0-macos-x86_64.zip\"")"
expect "the cask installs the bundle and its command" \
    'app "rBrowser.app" binary "#{appdir}/rBrowser.app/Contents/MacOS/rbrowser"' \
    "$(grep -E '^  (app|binary) ' "$cask" | sed 's/^  //' | tr '\n' ' ' | sed 's/ $//')"
expect "the cask explains Gatekeeper" "1" "$(grep -c 'signed ad hoc, not notarised' "$cask")"
# RB-090-80: the cask holds to the bundle - its minimum system (LSMinimumSystemVersion 13.0 is
# Ventura), the archive names scripts/package-macos.sh writes and the folders the app keeps.
expect "the cask asks for the bundle's minimum system" '  depends_on macos: ">= :ventura"' \
    "$(grep '^  depends_on macos' "$cask")"
expect "the bundle's minimum system is the one the cask names" "1" \
    "$(grep -c '^RB_MACOS_MINIMUM_SYSTEM="13.0"$' "$ROOT/scripts/lib/macos-bundle.sh")"
expect "package-macos.sh names its archives as the cask downloads them" "1" \
    "$(grep -c '^NAME="rbrowser-$version-macos-$arch"$' "$ROOT/scripts/package-macos.sh")"
expect "zap removes the settings and the state folder the app writes" \
    '"~/.config/rbrowser", "~/.local/state/rBrowser",' \
    "$(sed -n '/^  zap trash: \[$/,/^  \]$/p' "$cask" | grep '^    "' | sed 's/^ *//' | tr '\n' ' ' | sed 's/ $//')"
expect "formula, Linux x64: its archive and its hash" "  sha256 \"$(hash_of b)\"" \
    "$(line_after "$formula" "url \"$base/rbrowser-1.1.0-linux-x64.tar.gz\"")"
expect "the formula is Linux only" "1" "$(grep -c '^  depends_on :linux$' "$formula")"
expect "the formula names its tap" "1" "$(grep -c 'in the tap degoya/homebrew-rbrowser' "$formula")"
if command -v ruby > /dev/null; then
    run_status ruby -c "$formula"
    expect_status "the formula is Ruby" 0
    run_status ruby -c "$cask"
    expect_status "the cask is Ruby" 0
else
    echo "skip the formula and the cask are Ruby: no ruby on this machine"
fi

# Scoop.
scoop="$out/scoop/bucket/rbrowser.json"
expect "the manifest is JSON with the version" "1.1.0" "$(field "$scoop" 'm["version"]')"
expect "the manifest's archive" "$base/rbrowser-1.1.0-windows-x64.zip" \
    "$(field "$scoop" 'm["architecture"]["64bit"]["url"]')"
expect "the manifest's hash" "$(hash_of e)" "$(field "$scoop" 'm["architecture"]["64bit"]["hash"]')"
expect "it unpacks the zip's version folder" "rbrowser-1.1.0-windows-x64" \
    "$(field "$scoop" 'm["architecture"]["64bit"]["extract_dir"]')"
expect "the command is the console twin" "rbrowser.com" "$(field "$scoop" 'm["bin"]')"
expect "a Start menu entry for the GUI program" "rbrowser.exe rBrowser" "$(field "$scoop" '" ".join(m["shortcuts"][0])')"
expect "post_install writes the install-kind marker rb-update reads" "True" \
    "$(field "$scoop" '"install-kind" in m["post_install"][0] and "scoop" in m["post_install"][0]')"
expect "no persist: the data lives in the user's folders" "False" "$(field "$scoop" '"persist" in m')"
expect "checkver asks the repository's releases" "https://github.com/degoya/rBrowser" \
    "$(field "$scoop" 'm["checkver"]["github"]')"
expect "autoupdate keeps Scoop's own variables" \
    "https://github.com/degoya/rBrowser/releases/download/v\$version/rbrowser-\$version-windows-x64.zip rbrowser-\$version-windows-x64" \
    "$(field "$scoop" 'm["autoupdate"]["architecture"]["64bit"]["url"] + " " + m["autoupdate"]["architecture"]["64bit"]["extract_dir"]')"

# winget: three manifests of one version, the portable zip.
winget="$out/winget"
installer="$winget/degoya.rBrowser.installer.yaml"
expect "every manifest names the version" "3" "$(cat "$winget"/*.yaml | grep -c '^PackageVersion: 1.1.0$')"
expect "every manifest names the package" "3" \
    "$(cat "$winget"/*.yaml | grep -c '^PackageIdentifier: degoya.rBrowser$')"
expect "the installer is the Windows zip" \
    "  InstallerUrl: $base/rbrowser-1.1.0-windows-x64.zip" "$(grep '^  InstallerUrl: ' "$installer")"
expect "with its hash in upper case" "  InstallerSha256: $(hash_of E)" "$(grep '^  InstallerSha256: ' "$installer")"
expect "a portable inside a zip" "InstallerType: zip NestedInstallerType: portable" \
    "$(grep -E '^(Nested)?InstallerType: ' "$installer" | tr '\n' ' ' | sed 's/ $//')"
expect "the program in the zip's version folder" \
    '- RelativeFilePath: rbrowser-1.1.0-windows-x64\rbrowser.exe' "$(grep '^- RelativeFilePath: ' "$installer")"
expect "the folder goes on PATH, CEF stays beside the program" "ArchiveBinariesDependOnPath: true" \
    "$(grep '^ArchiveBinariesDependOnPath: ' "$installer")"
expect "no Scope on a portable package" "" "$(grep '^Scope: ' "$installer" || true)"
expect "the release notes of the tag" \
    "ReleaseNotesUrl: https://github.com/degoya/rBrowser/releases/tag/v1.1.0" \
    "$(grep '^ReleaseNotesUrl: ' "$winget/degoya.rBrowser.locale.en-US.yaml")"
if python3 -I -c 'import yaml' 2> /dev/null; then
    expect "the manifests are YAML of one schema version" "1.28.0 1.28.0 1.28.0" \
        "$(python3 -I -c 'import sys, yaml; print(" ".join(yaml.safe_load(open(f))["ManifestVersion"] for f in sys.argv[1:]))' \
            "$winget"/*.yaml)"
else
    echo "skip the manifests are YAML: no PyYAML on this machine"
fi

expect "the tap's README installs cask and formula from the tap" \
    "brew install --cask degoya/rbrowser/rbrowser brew install degoya/rbrowser/rbrowser" \
    "$(grep '^brew install' "$out/homebrew/README.md" | tr '\n' ' ' | sed 's/ $//')"
expect "the bucket's README adds the bucket" \
    "scoop bucket add rbrowser https://github.com/degoya/scoop-rbrowser" \
    "$(grep '^scoop bucket add' "$out/scoop/README.md")"

# RB-090-80: a release with the Windows ZIP and both macOS ZIPs, no Linux archive yet.
grep -v 'linux-x64' "$SCRATCH/SHA256SUMS" > "$SCRATCH/windows-macos"
run_status render 1.1.0 "$SCRATCH/windows-macos" "$SCRATCH/windows-macos-out"
expect_status "a release with Windows and macOS renders" 0
expect "the cask moves, the formula waits for its archive" \
    "homebrew/Casks/rbrowser.rb homebrew/README.md scoop/README.md scoop/bucket/rbrowser.json winget/degoya.rBrowser.installer.yaml winget/degoya.rBrowser.locale.en-US.yaml winget/degoya.rBrowser.yaml" \
    "$(files_of "$SCRATCH/windows-macos-out")"
expect_output "the formula is skipped with a note" "skip the Homebrew formula: the release has no Linux archive"
expect "the same cask as with every archive" "" \
    "$(diff "$cask" "$SCRATCH/windows-macos-out/homebrew/Casks/rbrowser.rb")"

# A fork and a local fixture.
all_sums 1.1.1-beta.1 > "$SCRATCH/beta-sums"
run_status render 1.1.1-beta.1 "$SCRATCH/beta-sums" "$SCRATCH/fork" --repository someone/rBrowser \
    --base-url "file:///tmp/fixture/"
expect_status "a pre-release version, another repository and a base URL" 0
expect "the archives come from the base URL, without its trailing slash" \
    '  url "file:///tmp/fixture/rbrowser-1.1.1-beta.1-linux-x64.tar.gz"' \
    "$(grep 'linux-x64' "$SCRATCH/fork/homebrew/Formula/rbrowser.rb")"
expect "the fork's bucket" "True" \
    "$(field "$SCRATCH/fork/scoop/bucket/rbrowser.json" '"someone/scoop-rbrowser" in m["##"]')"

# sha256sum's other spellings: ./, binary mode's *, upper case.
sed -e 's#  rbrowser-1.1.0-linux#  ./rbrowser-1.1.0-linux#' -e 's#  rbrowser-1.1.0-macos# *rbrowser-1.1.0-macos#' \
    -e 's#^e\{64\}#'"$(hash_of E)"'#' "$SCRATCH/SHA256SUMS" > "$SCRATCH/spellings"
run_status render 1.1.0 "$SCRATCH/spellings" "$SCRATCH/spellings-out"
expect_status "names with ./ or * are found" 0
expect "the same files either way" "" "$(diff -r "$out" "$SCRATCH/spellings-out")"

# Refusals: nothing is written.
grep -v 'windows-x64' "$SCRATCH/SHA256SUMS" > "$SCRATCH/no-windows"
run_status render 1.1.0 "$SCRATCH/no-windows" "$SCRATCH/no-windows-out"
expect_status "a release without the Windows ZIP is refused" 1
expect_output "and the archive is named" "no SHA-256 for rbrowser-1.1.0-windows-x64.zip"
expect_true "and nothing is written" '[[ ! -e "$SCRATCH/no-windows-out" ]]'
grep -v 'macos-x86_64' "$SCRATCH/SHA256SUMS" > "$SCRATCH/half-macos"
run_status render 1.1.0 "$SCRATCH/half-macos" "$SCRATCH/half-macos-out"
expect_status "a release with one macOS ZIP of two is refused" 1
expect_output "naming the missing one" "no SHA-256 for rbrowser-1.1.0-macos-x86_64.zip"
expect_true "and nothing is written" '[[ ! -e "$SCRATCH/half-macos-out" ]]'

run_status render 1.1 "$SCRATCH/SHA256SUMS" "$SCRATCH/bad-version"
expect_status "a version that is not X.Y.Z is refused" 1
run_status render 1.1.0 "$SCRATCH/SHA256SUMS" "$SCRATCH/bad-repo" --repository degoya
expect_status "a repository that is not OWNER/NAME is refused" 1
run_status render 1.1.0 "$SCRATCH/SHA256SUMS"
expect_status "too few arguments is a usage error" 2

# --check against the last release's SHA256SUMS.
run_status render --check --sums "$SCRATCH/windows-sums"
expect_status "--check renders against a Windows-only release" 0
expect_output "naming its version" "against the last release, 1.1.0: 5 files"
run_status render --check --sums "$SCRATCH/SHA256SUMS"
expect_status "--check renders against a full release" 0
expect_output "with every file" "against the last release, 1.1.0: 8 files"
run_status render --check --sums "$SCRATCH/half-macos"
expect_status "--check fails on half a channel" 1
run_status render --check --sums "$SCRATCH/no-windows"
expect_status "--check without the Windows zip cannot tell the version" 1
run_status render --check --sums "$SCRATCH/none"
expect_status "--check with a missing file is refused" 1
run_status render --check --unknown
expect_status "--check with an unknown option is a usage error" 2

finish_tests "package-managers.sh"

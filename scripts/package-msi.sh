#!/usr/bin/env bash
#
# Builds rBrowser's optional Windows installer (RB-099-01) from the unpacked package ZIP, with
# WiX 5 and packaging/windows/msi/rbrowser.wxs; adapted from rDownloader's scripts/package-msi.sh.
# The MSI holds exactly the package's files - listed by scripts/lib/package_msi.py into
# files.wxs - plus the marker install-kind (`msi`): installed per user, without admin rights, to
# %LOCALAPPDATA%\Programs\rBrowser with a Start menu entry; an uninstall leaves the user data
# (%APPDATA%\rBrowser, %LOCALAPPDATA%\rBrowser). Its own strings are packaging/windows/msi/
# en-US.wxl; WiX's dialogs come in the same culture.
#
# WiX runs on Windows: under Git Bash, after scripts/ci-tools.sh wix, which installs the WiX 5
# that scripts/lib/tool-versions.sh pins (RB_WIX_VERSION) and its UI extension at the same
# version. WiX 5 rather than 6 or later: from 6 on, WiX asks for its maintenance-fee EULA to be
# accepted. In WSL, --sources-only writes what WiX would be given (files.wxs, license.rtf,
# install-kind and wix-arguments.txt) without running it. scripts/package-windows.sh --msi calls
# this script with its staged folder; the CI's Windows runner (ci.yml, release.yml; RB-099-27)
# with the package ZIP, which is unpacked to a folder of its own first and has to hold exactly
# one package folder.
#
# Usage:
#   scripts/package-msi.sh <unpacked package folder | package ZIP> <out.msi>
#   scripts/package-msi.sh --sources-only <unpacked package folder | package ZIP> <out dir>
#
# The MSI's version is the three numbers of the version VERSION.txt names: MSI compares no more,
# and a pre-release suffix is not allowed in it (0.9.9-pre.2 is 0.9.9).
#
# Compiles nothing and takes no lock. Tests: scripts/tests/windows-paket-msi.sh.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/package.sh
source "$ROOT/scripts/lib/package.sh"
usage="usage: scripts/package-msi.sh [--sources-only] <unpacked package folder | package ZIP> <out.msi | out dir>"
sources_only=0
if [[ "${1:-}" == --sources-only ]]; then
    sources_only=1
    shift
fi
[[ $# -eq 2 && ( -d "$1" || ( -f "$1" && "$1" == *.zip ) ) ]] || { echo "$usage" >&2; exit 2; }
out="$2"
# Git Bash on a Windows runner may have python but no python3.
python="$(command -v python3 || command -v python)" || { echo "!! neither python3 nor python" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
if [[ -d "$1" ]]; then
    stage="$(cd "$1" && pwd)"
else
    # The package ZIP's one folder, rbrowser-<version>-windows-x64 (scripts/package-windows.sh);
    # a Windows python ends its line with a CR.
    stage="$(rb_package_unzip "$1" "$work/unpacked")" || exit 1
    stage="$(cd "${stage%$'\r'}" && pwd)"
fi

# A native Windows program takes Windows paths; under Git Bash cygpath makes them.
winpath() {
    if command -v cygpath > /dev/null; then cygpath -w "$1"; else printf '%s' "$1"; fi
}

for entry in rbrowser.exe VERSION.txt LICENSE; do
    [[ -f "$stage/$entry" ]] || { echo "!! $stage has no $entry: unpack the package ZIP there" >&2; exit 1; }
done
if [[ -e "$stage/install-kind" ]]; then
    echo "!! $stage carries an install-kind of its own; the MSI takes the package ZIP's folder" >&2
    exit 1
fi
# The portable marker or a data folder would make every installed copy portable.
rb_package_unportable "$stage"

version="$(sed -n '1s/^rBrowser //p' "$stage/VERSION.txt" | tr -d '\r')"
if [[ ! "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(-[0-9A-Za-z.]+)?$ ]]; then
    echo "!! $stage/VERSION.txt names no version ('$version')" >&2
    exit 1
fi
msi_version="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.${BASH_REMATCH[3]}"
# The limits of ProductVersion: major and minor below 256, build below 65536.
if ((BASH_REMATCH[1] > 255 || BASH_REMATCH[2] > 255 || BASH_REMATCH[3] > 65535)); then
    echo "!! $version does not fit an MSI ProductVersion" >&2
    exit 1
fi

"$python" -I "$ROOT/scripts/lib/package_msi.py" "$stage" "$work/files.wxs"

# The licence page WixUI_Minimal shows, from LICENSE: RTF with its three special characters
# escaped and every line a paragraph.
{
    printf '%s\n' '{\rtf1\ansi\deff0{\fonttbl{\f0\fmodern Consolas;}}\f0\fs16'
    sed -e 's/\r$//' -e 's/\\/\\\\/g' -e 's/{/\\{/g' -e 's/}/\\}/g' -e 's/$/\\par/' "$stage/LICENSE"
    printf '%s\n' '}'
} > "$work/license.rtf"
printf 'msi\n' > "$work/install-kind"

culture=en-US
arguments=(build -arch x64
    -ext WixToolset.UI.wixext
    -culture "$culture"
    -d "Version=$msi_version"
    -d "Stage=$(winpath "$stage")"
    -d "InstallKind=$(winpath "$work/install-kind")"
    -d "LicenseRtf=$(winpath "$work/license.rtf")"
    -d "Icon=$(winpath "$ROOT/crates/rbrowser/resources/rbrowser.ico")"
    -loc "$(winpath "$ROOT/packaging/windows/msi/$culture.wxl")"
    "$(winpath "$ROOT/packaging/windows/msi/rbrowser.wxs")" "$(winpath "$work/files.wxs")")

if [[ "$sources_only" == 1 ]]; then
    mkdir -p "$out"
    cp "$work/files.wxs" "$work/license.rtf" "$work/install-kind" "$out/"
    printf '%s\n' "${arguments[@]}" > "$out/wix-arguments.txt"
    echo "==> MSI sources in $out (rBrowser $version, MSI $msi_version, $culture)"
    exit 0
fi

if ! command -v wix > /dev/null; then
    echo "!! wix is not installed: WiX 5 runs on Windows (scripts/ci-tools.sh wix installs the pinned one);" >&2
    echo "   in WSL, --sources-only writes its sources" >&2
    exit 1
fi
mkdir -p "$(dirname "$out")"
rm -f "$out"
wix "${arguments[@]}" -o "$(winpath "$out")"
echo "==> $out (rBrowser $version, MSI $msi_version, $culture)"

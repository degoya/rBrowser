#!/usr/bin/env bash
#
# Builds the deb and the rpm package of Linux x64 from its release tarball (RB-097-05), with nfpm
# and packaging/linux/nfpm.yaml.in: exactly the tarball's files, installed in /usr/lib/rbrowser,
# the command linked into /usr/bin, Chromium's sandbox helper chrome-sandbox root's and setuid
# (04755), a menu entry (packaging/linux/rbrowser.desktop) with the app icon in the hicolor theme
# (assets/rBrowser-icon.svg and its renderings at 48, 128 and 256 px), and an install-kind marker
# (`deb`, `rpm`) that tells the update check apt or dnf updates it. The version is the one
# VERSION.txt names. Adapted from rCommander's scripts/package-deb-rpm.sh.
#
# Usage:
#   scripts/package-deb-rpm.sh <rbrowser-VERSION-linux-x64.tar.gz> <out-dir> [--deb] [--rpm]
#   scripts/package-deb-rpm.sh --render-only <tarball> <out-dir>   # the nfpm configs only
#
# Writes <out-dir>/rbrowser-VERSION-linux-x64.deb and .rpm (both without --deb or --rpm).
# Needs nfpm (scripts/release-channels.sh install-nfpm installs the pinned one;
# docs/development.md names it) and, for the icons, rsvg-convert (librsvg2-bin) or ImageMagick -
# neither with --render-only. Compiles nothing and takes no lock: scripts/package-linux.sh --deb
# --rpm calls it after the tarball.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/render-template.sh
source "$ROOT/scripts/lib/render-template.sh"
shopt -u patsub_replacement 2> /dev/null || true

usage() {
    echo "usage: scripts/package-deb-rpm.sh [--render-only] <rbrowser-VERSION-linux-x64.tar.gz> <out-dir> [--deb] [--rpm]" >&2
    exit 2
}

render_only=0
if [[ "${1:-}" == --render-only ]]; then
    render_only=1
    shift
fi
[[ $# -ge 2 ]] || usage
archive="$1"
out="$2"
shift 2
formats=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --deb) formats+=(deb); shift ;;
        --rpm) formats+=(rpm); shift ;;
        *) usage ;;
    esac
done
[[ "${#formats[@]}" -gt 0 ]] || formats=(deb rpm)

# nfpm names the architecture as Go does and writes each format's own name into the package.
name="$(basename "$archive")"
case "$name" in
    rbrowser-*-linux-x64.tar.gz) arch=x64 nfpm_arch=amd64 ;;
    *)
        echo "!! not a Linux release tarball: $archive (rbrowser-<version>-linux-x64.tar.gz)" >&2
        exit 2
        ;;
esac
[[ -f "$archive" ]] || { echo "!! $archive does not exist" >&2; exit 1; }
if [[ "$render_only" -eq 0 ]] && ! command -v nfpm > /dev/null; then
    echo "!! nfpm is not installed; see docs/development.md (Linux packages)" >&2
    exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/unpacked" "$work/helper" "$out"
tar -xzf "$archive" -C "$work/unpacked"
stage="$work/unpacked/${name%.tar.gz}"
for file in rbrowser libcef.so chrome-sandbox VERSION.txt LICENSE; do
    [[ -f "$stage/$file" ]] || {
        echo "!! $archive does not hold ${name%.tar.gz}/$file" >&2
        exit 1
    }
done
version="$(sed -n '1s/^rBrowser //p' "$stage/VERSION.txt")"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]]; then
    echo "!! $archive: VERSION.txt names no version ('$version')" >&2
    exit 1
fi
if [[ "$name" != "rbrowser-$version-linux-$arch.tar.gz" ]]; then
    echo "!! $archive is named for another version than its VERSION.txt ($version)" >&2
    exit 1
fi
if ! grep -q 'profile release$' "$stage/VERSION.txt"; then
    echo "note: $archive is a test package (VERSION.txt names no release profile); packaging it anyway" >&2
fi
# The helper gets its own entry (root's, setuid); the tree takes the rest of the folder.
mv "$stage/chrome-sandbox" "$work/helper/chrome-sandbox"

# The menu icon in the hicolor theme: the SVG as it is, and its renderings.
icons="$work/icons"
svg="$ROOT/assets/rBrowser-icon.svg"
png="$ROOT/assets/rBrowser-icon.png"
mkdir -p "$icons"
cp "$svg" "$icons/rbrowser.svg"
if [[ "$render_only" -eq 0 ]]; then
    for size in 48 128 256; do
        mkdir -p "$icons/$size"
        if command -v rsvg-convert > /dev/null; then
            rsvg-convert --width "$size" --height "$size" --output "$icons/$size/rbrowser.png" "$svg"
        elif command -v magick > /dev/null; then
            magick "$png" -resize "${size}x${size}" "$icons/$size/rbrowser.png"
        elif command -v convert > /dev/null; then
            convert "$png" -resize "${size}x${size}" "$icons/$size/rbrowser.png"
        else
            echo "!! the icons need rsvg-convert (librsvg2-bin) or ImageMagick" >&2
            exit 1
        fi
    done
fi

for kind in "${formats[@]}"; do
    printf '%s\n' "$kind" > "$work/install-kind.$kind"
    config="$work/nfpm-$kind.yaml"
    rb_render_template "$ROOT/packaging/linux/nfpm.yaml.in" "$config" \
        "VERSION=$version" "ARCH=$nfpm_arch" "STAGE=$stage" "HELPER=$work/helper/chrome-sandbox" \
        "KIND_FILE=$work/install-kind.$kind" "PACKAGING=$ROOT/packaging/linux" \
        "ICONS=$icons"
    if [[ "$render_only" -eq 1 ]]; then
        cp "$config" "$out/nfpm-$kind-$arch.yaml"
        echo "wrote $out/nfpm-$kind-$arch.yaml"
        continue
    fi
    target="$out/rbrowser-$version-linux-$arch.$kind"
    rm -f "$target"
    nfpm package --config "$config" --packager "$kind" --target "$target"
    echo "==> $target ($(wc -c < "$target") bytes, rbrowser $version)"
done

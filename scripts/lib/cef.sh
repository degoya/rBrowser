# shellcheck shell=bash
#
# The CEF runtime of the app (RB-010-04, ADR 0001): where the CEF distribution is cached, the check
# of its archive against our own SHA-256 pin (Decision 1), and the app folder in the sandboxed
# layout (Decision 6): CEF's bootstrap.exe as rbrowser.exe, rbrowser.dll beside it, the CEF
# runtime, the locales and the built shell (ui/dist as ui\).
#
#   source "$ROOT/scripts/lib/cef.sh"
#   export CEF_PATH="$(rb_cef_path)"               # the versioned cache the cef crate uses
#   rb_cef_verify "$CEF_PATH" [--if-present]        # the cached archive matches the pin
#   rb_stage_app "$build_dir" "$app_dir" [ui_dist]  # assembles (or updates) the app folder
#
# The macOS part (RB-090-15) is at the end: the macOS archives and their pins; the app bundle
# itself is scripts/lib/macos-bundle.sh.

# shellcheck source=scripts/lib/tool-versions.sh
source "$(dirname "${BASH_SOURCE[0]}")/tool-versions.sh"

# The CEF cache: $CEF_PATH when set, else ~/.cache/rbrowser/cef (shared by every worktree).
rb_cef_path() {
    printf '%s\n' "${CEF_PATH:-${XDG_CACHE_HOME:-$HOME/.cache}/rbrowser/cef}"
}

# Holds the archive in cache $1 to the pin. With --if-present a cache without the archive passes
# (the build downloads it; verify again afterwards).
rb_cef_verify() {
    local cache="$1" mode="${2:-}" archive actual
    archive="$cache/$RB_CEF_VERSION/$RB_CEF_ARCHIVE"
    if [[ ! -f "$archive" ]]; then
        [[ "$mode" == --if-present ]] && return 0
        echo "!! no CEF archive at $archive; a build with CEF_PATH=$cache downloads it" >&2
        return 1
    fi
    actual="$(sha256sum "$archive" | cut -d ' ' -f 1)"
    if [[ "$actual" != "$RB_CEF_ARCHIVE_SHA256" ]]; then
        echo "!! $archive has SHA-256 $actual; the pin (scripts/lib/tool-versions.sh) is $RB_CEF_ARCHIVE_SHA256" >&2
        echo "   Remove $cache/$RB_CEF_VERSION and build again; if the CDN changed the file, check it first." >&2
        return 1
    fi
}

# Assembles the app folder $2 from the build output $1 (target/<triple>/<profile>): bootstrap.exe
# as rbrowser.exe, rbrowser.dll (and its PDB when there), the CEF runtime (*.dll, *.pak, *.dat,
# *.bin, the SwiftShader ICD, locales\) and, when $3 holds a build of the shell, $3 as ui\.
# Files are copied only when newer, so a second run is quick.
rb_stage_app() {
    local build="$1" app="$2" ui="${3:-}" file name
    for name in bootstrap.exe rbrowser.dll libcef.dll; do
        if [[ ! -f "$build/$name" ]]; then
            echo "!! $build has no $name: build -p rbrowser for Windows first" >&2
            return 1
        fi
    done
    mkdir -p "$app"
    cp -u "$build/bootstrap.exe" "$app/rbrowser.exe"
    for file in "$build"/*.dll "$build"/*.pak "$build"/*.dat "$build"/*.bin \
        "$build/vk_swiftshader_icd.json" "$build/rbrowser.pdb"; do
        [[ -f "$file" ]] || continue
        cp -u "$file" "$app/"
    done
    if [[ -d "$build/locales" ]]; then
        mkdir -p "$app/locales"
        cp -ru "$build/locales/." "$app/locales/"
    fi
    if [[ -n "$ui" && -f "$ui/index.html" ]]; then
        rm -rf "$app/ui"
        cp -r "$ui" "$app/ui"
    else
        rm -rf "$app/ui"
    fi
}

# --- macOS (RB-090-15) -------------------------------------------------------------------------
#
#   rb_cef_macos_dir "$CEF_PATH" aarch64|x86_64          # the extracted distribution's folder
#   rb_cef_verify_macos "$CEF_PATH" aarch64|x86_64 [--if-present]

# The SHA-256 of file $1 with what the system has: sha256sum, or shasum on macOS.
rb_sha256() {
    if command -v sha256sum > /dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    else
        shasum -a 256 "$1" | cut -d ' ' -f 1
    fi
}

# The archive and its pin for the macOS architecture $1, as "<archive> <sha256>".
rb_cef_macos_pin() {
    case "$1" in
        aarch64) printf '%s %s\n' "$RB_CEF_ARCHIVE_MACOS_ARM64" "$RB_CEF_ARCHIVE_MACOS_ARM64_SHA256" ;;
        x86_64) printf '%s %s\n' "$RB_CEF_ARCHIVE_MACOS_X64" "$RB_CEF_ARCHIVE_MACOS_X64_SHA256" ;;
        *) echo "!! no CEF pin for the macOS architecture '$1' (aarch64 or x86_64)" >&2; return 2 ;;
    esac
}

# The folder the `cef` crate extracts the macOS distribution for architecture $2 into, in cache $1
# (cef-dll-sys: cef_<os>_<arch>); the framework lies at its top.
rb_cef_macos_dir() {
    printf '%s\n' "$1/$RB_CEF_VERSION/cef_macos_$2"
}

# Holds the macOS archive for architecture $2 in cache $1 to its pin, as rb_cef_verify does for
# Windows; --if-present lets a cache without the archive pass (the build downloads it).
rb_cef_verify_macos() {
    local cache="$1" arch="$2" mode="${3:-}" pin archive expected actual
    pin="$(rb_cef_macos_pin "$arch")" || return 2
    archive="$cache/$RB_CEF_VERSION/${pin% *}"
    expected="${pin##* }"
    if [[ ! -f "$archive" ]]; then
        [[ "$mode" == --if-present ]] && return 0
        echo "!! no CEF archive at $archive; a build with CEF_PATH=$cache downloads it" >&2
        return 1
    fi
    actual="$(rb_sha256 "$archive")"
    if [[ "$actual" != "$expected" ]]; then
        echo "!! $archive has SHA-256 $actual; the pin (scripts/lib/tool-versions.sh) is $expected" >&2
        echo "   Remove $cache/$RB_CEF_VERSION and build again; if the CDN changed the file, check it first." >&2
        return 1
    fi
}

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
# The macOS part (RB-090-15) follows: the macOS archives and their pins; the app bundle
# itself is scripts/lib/macos-bundle.sh. The Linux part (RB-097-05) is at the end: its archive,
# its pin and the app folder of the Linux package.

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
# The cef crate's build script copies the runtime into the build output, but only into the
# profile its run was shared with: a release-test build after a clippy run (the CI's Windows
# job) finds none there. The runtime then comes from the extracted distribution in the CEF cache
# (rb_cef_windows_dir), the files the build script would have copied.
# Files are copied only when newer, so a second run is quick.
rb_stage_app() {
    local build="$1" app="$2" ui="${3:-}" cef="$1" dist file name
    dist="$(rb_cef_windows_dir "$(rb_cef_path)")"
    if [[ ! -f "$build/bootstrap.exe" || ! -f "$build/libcef.dll" ]] \
        && [[ -f "$dist/bootstrap.exe" && -f "$dist/libcef.dll" ]]; then
        cef="$dist"
    fi
    if [[ ! -f "$build/rbrowser.dll" ]]; then
        echo "!! $build has no rbrowser.dll: build -p rbrowser for Windows first" >&2
        return 1
    fi
    for name in bootstrap.exe libcef.dll; do
        if [[ ! -f "$cef/$name" ]]; then
            echo "!! $build has no $name (nor $dist): build -p rbrowser for Windows first" >&2
            return 1
        fi
    done
    mkdir -p "$app"
    cp -u "$cef/bootstrap.exe" "$app/rbrowser.exe"
    for file in "$cef"/*.dll "$cef"/*.pak "$cef"/*.dat "$cef"/*.bin \
        "$cef/vk_swiftshader_icd.json" "$build/rbrowser.dll" "$build/rbrowser.pdb"; do
        [[ -f "$file" ]] || continue
        cp -u "$file" "$app/"
    done
    if [[ -d "$cef/locales" ]]; then
        mkdir -p "$app/locales"
        cp -ru "$cef/locales/." "$app/locales/"
    fi
    if [[ -n "$ui" && -f "$ui/index.html" ]]; then
        rm -rf "$app/ui"
        cp -r "$ui" "$app/ui"
    else
        rm -rf "$app/ui"
    fi
}

# The Windows distribution the cef crate extracted into cache $1: <cache>/<version>/cef_windows_x86_64.
rb_cef_windows_dir() {
    printf '%s\n' "$1/$RB_CEF_VERSION/cef_windows_x86_64"
}

# --- macOS (RB-090-15) -------------------------------------------------------------------------
#
#   rb_cef_macos_dir "$CEF_PATH" aarch64|x86_64          # the extracted distribution's folder
#   rb_cef_verify_macos "$CEF_PATH" aarch64|x86_64 [--if-present]

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

# --- Linux (RB-097-05) -------------------------------------------------------------------------
#
#   rb_cef_linux_dir "$CEF_PATH"                         # the extracted distribution's folder
#   rb_cef_verify_linux "$CEF_PATH" [--if-present]
#   rb_stage_app_linux "$build_dir" "$app_dir" [ui_dist] # the app folder of the Linux package

# The folder the `cef` crate extracts the Linux x64 distribution into, in cache $1.
rb_cef_linux_dir() {
    printf '%s\n' "$1/$RB_CEF_VERSION/cef_linux_x86_64"
}

# Holds the Linux archive in cache $1 to its pin, as rb_cef_verify does for Windows; --if-present
# lets a cache without the archive pass (the build downloads it).
rb_cef_verify_linux() {
    local cache="$1" mode="${2:-}" archive actual
    archive="$cache/$RB_CEF_VERSION/$RB_CEF_ARCHIVE_LINUX_X64"
    if [[ ! -f "$archive" ]]; then
        [[ "$mode" == --if-present ]] && return 0
        echo "!! no CEF archive at $archive; a build with CEF_PATH=$cache downloads it" >&2
        return 1
    fi
    actual="$(rb_sha256 "$archive")"
    if [[ "$actual" != "$RB_CEF_ARCHIVE_LINUX_X64_SHA256" ]]; then
        echo "!! $archive has SHA-256 $actual; the pin (scripts/lib/tool-versions.sh) is $RB_CEF_ARCHIVE_LINUX_X64_SHA256" >&2
        echo "   Remove $cache/$RB_CEF_VERSION/cef_linux_x86_64 and the archive and build again; if the CDN changed the file, check it first." >&2
        return 1
    fi
}

# Assembles the Linux app folder $2 from the build output $1 (target/<triple>/<profile>): the
# executable rbrowser-nosandbox as rbrowser (on Linux it runs with the sandbox: the kernel's user
# namespaces or the setuid chrome-sandbox, no bootstrap), the CEF runtime - libcef.so, ANGLE's
# libEGL.so and libGLESv2.so, chrome-sandbox, SwiftShader and its Vulkan loader, *.pak, *.dat,
# *.bin, the ICD file and locales/ - and, when $3 holds a build of the shell, $3 as ui/. Named
# one by one, not *.so: the build output holds libraries of ours. The runtime comes from the build
# output, where the cef crate copies it, else from the extracted distribution in the CEF cache
# (rb_cef_linux_dir). chrome-sandbox keeps mode 0755 here: only an installed package can make it
# root's and setuid (packaging/linux/nfpm.yaml.in). Files are copied only when newer.
rb_stage_app_linux() {
    local build="$1" app="$2" ui="${3:-}" cef="$1" dist file
    dist="$(rb_cef_linux_dir "$(rb_cef_path)")"
    if [[ ! -f "$build/libcef.so" && -f "$dist/libcef.so" ]]; then
        cef="$dist"
    fi
    if [[ ! -f "$build/rbrowser-nosandbox" ]]; then
        echo "!! $build has no rbrowser-nosandbox: build -p rbrowser for Linux first" >&2
        return 1
    fi
    if [[ ! -f "$cef/libcef.so" ]]; then
        echo "!! $build has no libcef.so (nor $dist): build -p rbrowser for Linux first" >&2
        return 1
    fi
    mkdir -p "$app"
    cp -u "$build/rbrowser-nosandbox" "$app/rbrowser"
    chmod 755 "$app/rbrowser"
    for file in "$cef"/libcef.so "$cef"/libEGL.so "$cef"/libGLESv2.so "$cef"/libvk_swiftshader.so \
        "$cef"/libvulkan.so.1 "$cef"/*.pak "$cef"/*.dat "$cef"/*.bin "$cef/vk_swiftshader_icd.json"; do
        [[ -f "$file" ]] || continue
        cp -u "$file" "$app/"
    done
    if [[ -f "$cef/chrome-sandbox" ]]; then
        cp -u "$cef/chrome-sandbox" "$app/chrome-sandbox"
        chmod 755 "$app/chrome-sandbox"
    fi
    if [[ -d "$cef/locales" ]]; then
        mkdir -p "$app/locales"
        cp -ru "$cef/locales/." "$app/locales/"
    fi
    rm -rf "$app/ui"
    if [[ -n "$ui" && -f "$ui/index.html" ]]; then
        cp -r "$ui" "$app/ui"
    fi
}

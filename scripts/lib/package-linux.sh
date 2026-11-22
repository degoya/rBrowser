# shellcheck shell=bash
#
# What the Linux package carries beyond the app folder of scripts/lib/cef.sh (RB-097-05): the
# pinned Node runtime, the stripped CEF library, and the checks a release tarball must pass.
# The Windows package's helpers (scripts/lib/package-stage.sh: the download cache, the front
# ends, the sidecar, crash_reporter.cfg) are shared; this file sources them.
#
#   source "$ROOT/scripts/lib/package-linux.sh"
#   rb_stage_node_linux "$ROOT" "$cache" "$STAGE"      # node/bin/node and node/LICENSE
#   rb_strip_linux "$STAGE"                            # libcef.so and rbrowser without symbols
#   rb_check_linux_stage "$STAGE"                      # what a tarball must hold
#
# Tests: scripts/tests/package-linux.sh (a stand-in Node tarball in the cache, no network).

# shellcheck source=scripts/lib/package-stage.sh
source "$(dirname "${BASH_SOURCE[0]}")/package-stage.sh"

# Stages the Node runtime of checkout $1 into $3/node (bin/node and its LICENSE, which names the
# licences of everything Node bundles): the official linux-x64 tarball of .nvmrc's version, held
# to RB_NODE_LINUX_X64_SHA256 and cached in $2. A pin for another version than .nvmrc's stops the
# run.
rb_stage_node_linux() {
    local root="$1" cache="$2" stage="$3" version name archive
    version="$(rb_node_version "$root")"
    if [[ -z "$version" || "$RB_NODE_LINUX_X64_VERSION" != "$version" ]]; then
        echo "!! the Linux pin is for node $RB_NODE_LINUX_X64_VERSION, .nvmrc says ${version:-nothing}:" >&2
        echo "   pin RB_NODE_LINUX_X64_VERSION and _SHA256 in scripts/lib/tool-versions.sh" >&2
        echo "   (https://nodejs.org/dist/v$version/SHASUMS256.txt, node-v$version-linux-x64.tar.xz)" >&2
        return 1
    fi
    name="node-v$version-linux-x64"
    archive="$cache/$name.tar.xz"
    rb_fetch_pinned "https://nodejs.org/dist/v$version/$name.tar.xz" "$archive" \
        "$RB_NODE_LINUX_X64_SHA256" || return 1
    mkdir -p "$stage/node/bin"
    tar -xJf "$archive" -C "$stage/node/bin" --strip-components=2 "$name/bin/node" || return 1
    tar -xJf "$archive" -C "$stage/node" --strip-components=1 "$name/LICENSE" || return 1
    chmod 755 "$stage/node/bin/node"
    echo "==> node $version staged (tarball SHA-256 $RB_NODE_LINUX_X64_SHA256)" >&2
}

# Takes the symbols out of the staged libcef.so (1.5 GB with them, under 300 MB without) and
# the program: a package carries no debug information. Needs strip (binutils).
rb_strip_linux() {
    local stage="$1" file
    command -v strip > /dev/null 2>&1 || { echo "!! strip (binutils) is not installed" >&2; return 1; }
    for file in "$stage/libcef.so" "$stage/rbrowser"; do
        [[ -f "$file" ]] || continue
        strip --strip-unneeded "$file"
    done
}

# Fails, naming each, when the stage $1 lacks what the Linux package needs to start: the program,
# the CEF runtime, the sandbox helper, the shell, the Node runtime, the sidecar, the notices.
rb_check_linux_stage() {
    local stage="$1" path missing=()
    for path in rbrowser libcef.so libEGL.so libGLESv2.so chrome-sandbox icudtl.dat \
        v8_context_snapshot.bin locales/en-US.pak ui/index.html node/bin/node node/LICENSE sidecar/dist/main.js \
        NOTICE LICENSE VERSION.txt licenses/chromium-credits.html crash_reporter.cfg; do
        [[ -e "$stage/$path" ]] || missing+=("$path")
    done
    if ((${#missing[@]})); then
        printf '!! the Linux stage lacks %s\n' "${missing[@]}" >&2
        return 1
    fi
    if [[ ! -x "$stage/rbrowser" || ! -x "$stage/node/bin/node" ]]; then
        echo "!! rbrowser and node/bin/node must be executable" >&2
        return 1
    fi
}

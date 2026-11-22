#!/usr/bin/env bash
#
# Builds the Linux x64 release natively (RB-097-05) and packs it as the portable archive
#   artifacts/linux/rbrowser-<version>-linux-x64.tar.gz
# and, with --deb and --rpm, as the distribution packages beside it (scripts/package-deb-rpm.sh),
# with
#   - rbrowser, the app (crates/rbrowser's rbrowser-nosandbox, which on Linux runs with the
#     sandbox), CEF's runtime beside it - libcef.so without its symbols, chrome-sandbox (mode 0755
#     here; the deb and rpm make it root's and setuid), *.pak, *.dat, *.bin, SwiftShader, locales/ -
#     and the shell, built here from ui/, as ui/ (scripts/lib/cef.sh; the CEF archive held to its
#     SHA-256 pin);
#   - crash_reporter.cfg (Crashpad keeps dumps local, no upload; rb_diag::crash_reporter_cfg);
#   - node/bin/node and node/LICENSE: the Node runtime of .nvmrc, its tarball held to its SHA-256
#     pin (scripts/lib/package-linux.sh);
#   - sidecar/ (RB-040-03): dist/main.js and dist/flow-worker.js built from sidecar/, its
#     package.json and the production node_modules (scripts/lib/package-stage.sh);
#   - plugins/ - the bundled plugins, built, signed and checked by scripts/build-plugins.sh;
#   - README.txt (start, the sandbox, the system's libraries), NOTICE (CEF, Chromium, Node,
#     playwright-core and their licences), licenses/chromium-credits.html, LICENSE, CHANGELOG.md
#     and VERSION.txt (version, commit, the bundled CEF and Node);
# and prints a size report. No rbrowser-wsl (native Linux watches with inotify itself), no
# rbrowser.com (the program is a console program already), no updater (apt, dnf or the release
# page update it). Downloads land in the build cache (~/.cache/rbrowser/package).
#
# Usage:
#   scripts/package-linux.sh                          # the release profile
#   scripts/package-linux.sh --profile release-test   # a test package, built much faster
#   scripts/package-linux.sh --plugins <dir>          # the signed packages in <dir>, not built here
#   scripts/package-linux.sh --allow-dirty            # a dirty tree, for a local test only
#   scripts/package-linux.sh --deb --rpm              # also the deb and the rpm (needs nfpm)
#
# Runs on Linux x64 only (a native build: CEF's libcef.so is linked). The version is held by
# scripts/set-version.sh --check first; a dirty tree, or a package of the same version from
# another commit or a dirty tree, stops the run before anything is built (scripts/lib/package.sh;
# RB_REPACK=1 replaces the package, --allow-dirty takes a dirty tree).
#
# Takes the build lock (scripts/lib/lock.sh) - never wrap it in flock. Parallelism:
# scripts/lib/jobs.sh (RB_JOBS).
set -euo pipefail

TARGET="x86_64-unknown-linux-gnu"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/jobs.sh
source "$ROOT/scripts/lib/jobs.sh"
# Sourced before the `cd`, because the lock library resolves this script's own path from $0.
# shellcheck source=lib/lock.sh
source "$ROOT/scripts/lib/lock.sh"
rb_take_lock "$@"
# shellcheck source=lib/package.sh
source "$ROOT/scripts/lib/package.sh"
# shellcheck source=lib/cef.sh
source "$ROOT/scripts/lib/cef.sh"
# shellcheck source=lib/package-linux.sh
source "$ROOT/scripts/lib/package-linux.sh"
cd "$ROOT"

profile=release
prebuilt=""
allow_dirty=0
distribution=()
usage="usage: scripts/package-linux.sh [--profile release|release-test] [--plugins <dir>] [--allow-dirty] [--deb] [--rpm]"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --profile)
            [[ $# -ge 2 ]] || { echo "$usage" >&2; exit 2; }
            profile="$2"
            shift 2
            ;;
        --plugins)
            [[ $# -ge 2 && -d "$2" ]] || { echo "$usage (an existing directory)" >&2; exit 2; }
            prebuilt="$(cd "$2" && pwd)"
            shift 2
            ;;
        --allow-dirty) allow_dirty=1; shift ;;
        --deb) distribution+=(--deb); shift ;;
        --rpm) distribution+=(--rpm); shift ;;
        *) echo "$usage" >&2; exit 2 ;;
    esac
done
case "$profile" in release | release-test) ;; *) echo "!! unknown profile '$profile'" >&2; exit 2 ;; esac
if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
    echo "!! scripts/package-linux.sh builds on Linux x86_64 only (this is $(uname -s) $(uname -m))" >&2
    exit 2
fi
if [[ "${#distribution[@]}" -gt 0 ]] && ! command -v nfpm > /dev/null; then
    echo "!! --deb and --rpm need nfpm; see docs/development.md (Linux packages)" >&2
    exit 1
fi

version="$(rb_package_version "$ROOT")"
OUT="$ROOT/artifacts/linux"
STAGE="$OUT/rbrowser-$version-linux-x64"
TARBALL="$OUT/rbrowser-$version-linux-x64.tar.gz"
echo "==> packaging rBrowser $version for $TARGET (profile: $profile, jobs: $JOBS) into $TARBALL"

# The commit and the time, exported so the binary and VERSION.txt name the same ones (build.rs
# takes them verbatim). An existing package of this version from another build stops the run.
commit="$(rb_package_commit)"
rb_package_clean "$commit" "$allow_dirty"
rb_package_guard "$TARBALL" "$(basename "$STAGE")/VERSION.txt" "$commit"
RB_BUILD_COMMIT="$commit"
RB_BUILD_TIME="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
export RB_BUILD_COMMIT RB_BUILD_TIME

# The bundled plugins.
plugins=()
if [[ -n "$prebuilt" || -x scripts/build-plugins.sh ]]; then
    plugin_list="$(rb_package_plugins "$ROOT" "$prebuilt")"
    while IFS= read -r package; do [[ -z "$package" ]] || plugins+=("$package"); done <<< "$plugin_list"
fi

# The front ends and the sidecar's install run with the node .nvmrc pins.
rb_use_pinned_node "$ROOT"

# The pinned download first, so a missing network or a changed file stops the run before the
# long build.
CACHE="$(rb_package_cache)"
NODE_STAGE="$OUT/node-stage"
rm -rf "$NODE_STAGE"
rb_stage_node_linux "$ROOT" "$CACHE" "$NODE_STAGE"

# The front ends, built from the tracked sources: never a stale dist/ (the sync agent and the
# page API are embedded by build.rs).
for dir in ui agent sidecar; do rb_build_frontend "$ROOT/$dir"; done
for built in ui/dist/index.html agent/dist/rb-agent.js agent/dist/rb-page-api.js sidecar/dist/main.js; do
    [[ -s "$built" ]] || { echo "!! $built is missing after the build" >&2; exit 1; }
done

CEF_PATH="$(rb_cef_path)"
export CEF_PATH
rb_cef_verify_linux "$CEF_PATH" --if-present
echo "==> building rBrowser (CEF in $CEF_PATH)"
CARGO_BUILD_JOBS="$JOBS" cargo build --locked --profile "$profile" --target "$TARGET" \
    -j "$JOBS" -p rbrowser --bin rbrowser-nosandbox
rb_cef_verify_linux "$CEF_PATH"

echo "==> assembling $STAGE"
rm -rf "$STAGE" "$TARBALL"
build="${CARGO_TARGET_DIR:-$ROOT/target}/$TARGET/$profile"
rb_stage_app_linux "$build" "$STAGE" "$ROOT/ui/dist"
# Built with the feature `self-update` (the default): the positive control of
# scripts/check-update-code.sh, before the symbols go.
echo "==> the app carries the self-update (scripts/check-update-code.sh --expect-present)"
scripts/check-update-code.sh --expect-present "$STAGE/rbrowser"
rb_strip_linux "$STAGE"
rb_write_crash_reporter_cfg packaging/windows/crash_reporter.cfg.in "$STAGE" "$version"
mv "$NODE_STAGE/node" "$STAGE/node"
rm -rf "$NODE_STAGE"
rb_stage_sidecar "$ROOT/sidecar" "$STAGE/sidecar"
mkdir -p "$STAGE/licenses"
credits="$(rb_cef_linux_dir "$CEF_PATH")/CREDITS.html"
[[ -f "$credits" ]] || { echo "!! no $credits: the Chromium licences cannot be shipped" >&2; exit 1; }
install -m 644 "$credits" "$STAGE/licenses/chromium-credits.html"
install -m 644 packaging/linux/README.txt packaging/linux/NOTICE LICENSE CHANGELOG.md "$STAGE/"
if ((${#plugins[@]})); then
    mkdir -p "$STAGE/plugins"
    install -m 644 "${plugins[@]}" "$STAGE/plugins/"
fi
cef_build="${RB_CEF_ARCHIVE_LINUX_X64#cef_binary_}"
printf '%s\n' "rBrowser $version" "commit $RB_BUILD_COMMIT" "built $RB_BUILD_TIME" \
    "target $TARGET, profile $profile" "cef ${cef_build%_linux64*}" \
    "node $(rb_node_version "$ROOT") (node SHA-256 $(sha256sum "$STAGE/node/bin/node" | cut -d ' ' -f 1))" \
    > "$STAGE/VERSION.txt"
rb_check_linux_stage "$STAGE"
# Never the portable mode's marker or data.
rb_package_unportable "$STAGE"

echo "==> writing $TARBALL"
tar -C "$OUT" --owner=0 --group=0 --numeric-owner -czf "$TARBALL" "$(basename "$STAGE")"
rb_package_sizes "$STAGE" "$TARBALL"
rm -rf "$STAGE"
if [[ "${#distribution[@]}" -gt 0 ]]; then
    echo "==> the distribution packages of $TARBALL"
    "$ROOT/scripts/package-deb-rpm.sh" "$TARBALL" "$OUT" "${distribution[@]}"
fi
echo "==> done"
ls -la "$TARBALL"

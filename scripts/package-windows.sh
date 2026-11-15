#!/usr/bin/env bash
#
# Builds the Windows release from WSL with cargo-xwin - or natively with cargo on Windows (Git
# Bash, later the GitHub runner of release.yml) - and packs it as the portable ZIP (RB-010-15):
#   artifacts/windows/rbrowser-<version>-windows-x64.zip
# with
#   - the app in the sandboxed layout of ADR 0001: CEF's bootstrap.exe as rbrowser.exe - given
#     rBrowser's icon and version resource with rcedit (scripts/lib/package-stage.sh) -,
#     rbrowser.dll, the CEF runtime and locales\, and the shell, built here from ui/, as ui\
#     (scripts/lib/cef.sh; the CEF archive held to its SHA-256 pin);
#   - rbrowser.com, the console twin (crates/rbrowser-cli, RB-010-11): a bare `rbrowser` in a
#     terminal finds it before the GUI rbrowser.exe, waits for a command and shows its output;
#   - crash_reporter.cfg (Crashpad keeps dumps local, no upload; rb_diag::crash_reporter_cfg);
#   - node\node.exe and node\LICENSE: the Node runtime of .nvmrc, its ZIP held to its SHA-256 pin;
#   - sidecar\ (RB-040-03): dist\main.js and dist\flow-worker.js built from sidecar/, its
#     package.json and the production node_modules\{playwright-core,playwright,@playwright\test}
#     with their LICENSE and NOTICE (scripts/lib/package-stage.sh);
#   - once plugins/ exists, plugins\ - the bundled plugins, built, signed and checked by
#     scripts/build-plugins.sh;
#   - README.txt (start, SmartScreen, portable mode, diagnostics), NOTICE (CEF, Chromium, Node,
#     playwright-core and their licences), licenses\chromium-credits.html, LICENSE, CHANGELOG.md
#     and VERSION.txt (version, commit, the bundled CEF and Node);
# and prints a size report. Downloads land in the build cache (~/.cache/rbrowser/package).
# The front ends ui/, agent/ (embedded by build.rs) and sidecar/ are built here from their lock
# files, without the packages' install scripts, by the node .nvmrc pins (fnm or nvm switch to it
# when another one is first on PATH). rbrowser-wsl, the static helper inside WSL (RB-030-04),
# is built for x86_64-unknown-linux-musl and lies beside rbrowser.exe, where the app starts it
# from (scripts/lib/wsl-helper.sh); WebKit is downloaded by the user (engines.update).
#
# Usage:
#   scripts/package-windows.sh                          # the release profile
#   scripts/package-windows.sh --profile release-test   # a test package, built much faster
#   scripts/package-windows.sh --plugins <dir>          # the signed packages in <dir> (relative
#                                                       # to the checkout), not built here
#   scripts/package-windows.sh --allow-dirty            # a dirty tree, for a local test only
#   scripts/package-windows.sh --sign                   # Authenticode-signs the staged binaries
#                                                       # (scripts/sign-windows.sh, RB-090-03);
#                                                       # RB_SIGN=1 in the environment asks the same
#
# The version is held by scripts/set-version.sh --check first; a dirty tree, or a package of the
# same version from another commit or a dirty tree, stops the run before anything is built
# (scripts/lib/package.sh; RB_REPACK=1 replaces the package, --allow-dirty takes a dirty tree).
# rcedit is a Windows program: in WSL it runs through interop.
#
# Takes the build lock (scripts/lib/lock.sh) - never wrap it in flock. Parallelism:
# scripts/lib/jobs.sh (RB_JOBS).
set -euo pipefail

TARGET="x86_64-pc-windows-msvc"
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
# shellcheck source=lib/package-stage.sh
source "$ROOT/scripts/lib/package-stage.sh"
# shellcheck source=lib/wsl-helper.sh
source "$ROOT/scripts/lib/wsl-helper.sh"
cd "$ROOT"

profile=release
prebuilt=""
allow_dirty=0
sign=0
usage="usage: scripts/package-windows.sh [--profile release|release-test] [--plugins <dir>] [--allow-dirty] [--sign]"
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
        --sign) sign=1; shift ;;
        *) echo "$usage" >&2; exit 2 ;;
    esac
done
case "$profile" in release | release-test) ;; *) echo "!! unknown profile '$profile'" >&2; exit 2 ;; esac

version="$(rb_package_version "$ROOT")"
OUT="$ROOT/artifacts/windows"
STAGE="$OUT/rbrowser-$version-windows-x64"
ZIP="$OUT/rbrowser-$version-windows-x64.zip"
echo "==> packaging rBrowser $version for $TARGET (profile: $profile, jobs: $JOBS) into $ZIP"
# Signing is off unless asked for; then its key, tool and timestamp server are held before the
# long build, and without them the run ends here (RB-090-03).
[[ "${RB_SIGN:-0}" != 1 ]] || sign=1
[[ "$sign" == 0 ]] || rb_package_sign_check "$ROOT"
# From here on no child inherits a signing secret: npm, build scripts and compilers run without
# them; only scripts/sign-windows.sh gets them back, after the build.
rb_package_hold_signing

# The commit and the time, exported so the binary and VERSION.txt name the same ones (build.rs
# takes them verbatim). An existing package of this version from another build stops the run.
commit="$(rb_package_commit)"
rb_package_clean "$commit" "$allow_dirty"
rb_package_guard "$ZIP" "$(basename "$STAGE")/VERSION.txt" "$commit"
RB_BUILD_COMMIT="$commit"
RB_BUILD_TIME="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
export RB_BUILD_COMMIT RB_BUILD_TIME

# The bundled plugins, once there are any (RB-010-03).
plugins=()
if [[ -n "$prebuilt" || -x scripts/build-plugins.sh ]]; then
    plugin_list="$(rb_package_plugins "$ROOT" "$prebuilt")"
    while IFS= read -r package; do [[ -z "$package" ]] || plugins+=("$package"); done <<< "$plugin_list"
fi

# The front ends and the sidecar's install run with the node .nvmrc pins, activated with fnm or
# nvm when another one is first on PATH; without it the run stops here, before anything is built.
rb_use_pinned_node "$ROOT"

# The pinned downloads first, so a missing network or a changed file stops the run before the
# long build: the Node runtime (staged later) and rcedit (rb_patch_exe).
CACHE="$(rb_package_cache)"
NODE_STAGE="$OUT/node-stage"
rm -rf "$NODE_STAGE"
rb_stage_node "$ROOT" "$CACHE" "$NODE_STAGE"
rb_fetch_pinned "https://github.com/electron/rcedit/releases/download/v$RB_RCEDIT_VERSION/rcedit-x64.exe" \
    "$CACHE/rcedit-x64.exe" "$RB_RCEDIT_SHA256"

# The front ends, built from the tracked sources: never a stale dist/. The sync agent and the
# page API are embedded by build.rs (RB-020-01, RB-020-08), so they are built before the app; a
# package without them would not sync and give the pages no window.__rbrowser.
for dir in ui agent sidecar; do rb_build_frontend "$ROOT/$dir"; done
for built in ui/dist/index.html agent/dist/rb-agent.js agent/dist/rb-page-api.js sidecar/dist/main.js; do
    [[ -s "$built" ]] || { echo "!! $built is missing after the build" >&2; exit 1; }
done

# cargo-xwin brings the MSVC libraries to WSL; on Windows itself cargo has them.
build=(xwin build)
case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) build=(build) ;; esac
CEF_PATH="$(rb_cef_path)"
export CEF_PATH
rb_cef_verify "$CEF_PATH" --if-present
echo "==> building rBrowser (cargo ${build[*]}, CEF in $CEF_PATH)"
CARGO_BUILD_JOBS="$JOBS" cargo "${build[@]}" --locked --profile "$profile" --target "$TARGET" \
    -j "$JOBS" -p rbrowser -p rbrowser-cli
rb_cef_verify "$CEF_PATH"
echo "==> building rbrowser-wsl ($RB_WSL_TARGET, static)"
wsl_helper="$(rb_wsl_helper_build "$profile" "$JOBS")"

echo "==> assembling $STAGE"
rm -rf "$STAGE" "$ZIP"
# The sandboxed layout (ADR 0001): bootstrap.exe as rbrowser.exe, rbrowser.dll, the CEF runtime,
# ui/dist as ui\ (scripts/lib/cef.sh).
rb_stage_app "${CARGO_TARGET_DIR:-$ROOT/target}/$TARGET/$profile" "$STAGE" "$ROOT/ui/dist"
# The console twin rbrowser.com, so a terminal waits for a command and shows its output.
rb_stage_console "${CARGO_TARGET_DIR:-$ROOT/target}/$TARGET/$profile" "$STAGE"
# The WSL helper beside rbrowser.exe: the app starts it from there through /mnt/<drive>/.
rb_stage_wsl_helper "$wsl_helper" "$STAGE"
# A test package's line tables are in the PDB (Cargo.toml, [profile.release-test]); beside the DLL
# a panic's backtrace finds it. Other packages ship none.
[[ "$profile" == release-test ]] || rm -f "$STAGE/rbrowser.pdb"
[[ -f "$STAGE/ui/index.html" ]] || { echo "!! the stage has no ui\\index.html" >&2; exit 1; }
# Built without the feature `self-update`: no binary of ours, nor rbrowser.pdb, carries update
# code or the update address (RB-090-02).
echo "==> no update code in the binaries (scripts/check-update-code.sh)"
scripts/check-update-code.sh "$STAGE/rbrowser.exe" "$STAGE/rbrowser.dll" "$STAGE/rbrowser.com"
echo "==> rbrowser.exe: icon and version resource (rcedit $RB_RCEDIT_VERSION)"
rb_patch_exe "$STAGE/rbrowser.exe" crates/rbrowser/resources/rbrowser.ico "$version" "$CACHE"
rb_write_crash_reporter_cfg packaging/windows/crash_reporter.cfg.in "$STAGE" "$version"
mv "$NODE_STAGE/node" "$STAGE/node"
rm -rf "$NODE_STAGE"
rb_stage_sidecar "$ROOT/sidecar" "$STAGE/sidecar"
mkdir -p "$STAGE/licenses"
credits="$CEF_PATH/$RB_CEF_VERSION/cef_windows_x86_64/CREDITS.html"
[[ -f "$credits" ]] || { echo "!! no $credits: the Chromium licences cannot be shipped" >&2; exit 1; }
install -m 644 "$credits" "$STAGE/licenses/chromium-credits.html"
install -m 644 packaging/windows/README.txt packaging/windows/NOTICE LICENSE CHANGELOG.md "$STAGE/"
if ((${#plugins[@]})); then
    mkdir -p "$STAGE/plugins"
    install -m 644 "${plugins[@]}" "$STAGE/plugins/"
fi
cef_build="${RB_CEF_ARCHIVE#cef_binary_}"
printf '%s\n' "rBrowser $version" "commit $RB_BUILD_COMMIT" "built $RB_BUILD_TIME" \
    "target $TARGET, profile $profile" "cef ${cef_build%_windows64*}" \
    "node $(rb_node_version "$ROOT") (node.exe SHA-256 $(sha256sum "$STAGE/node/node.exe" | cut -d ' ' -f 1))" \
    > "$STAGE/VERSION.txt"

# --sign: the staged binaries - ours and CEF's, not node\ - are signed and verified before the
# ZIP is written; a failure ends the run without a package (RB-090-03). release.yml instead signs
# the finished ZIP in a step of its own, so no certificate is on the runner while anything builds.
if [[ "$sign" == 1 ]]; then
    rb_package_with_signing "$ROOT/scripts/sign-windows.sh" "$STAGE"
fi
echo "==> writing $ZIP"
rb_package_zip "$OUT" "$(basename "$STAGE")" "$ZIP"
rb_package_sizes "$STAGE" "$ZIP"
rm -rf "$STAGE"
echo "==> done"
ls -la "$ZIP"

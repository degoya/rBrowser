#!/usr/bin/env bash
#
# scripts/lib/package-stage.sh without the network or Windows (RB-010-15): a fake `curl` serves
# files from a scratch folder, a fake rcedit records its arguments. A download is kept only when
# its SHA-256 is the pin, a cached file that is not stops the run; the Node runtime is taken only
# for .nvmrc's version and only node.exe and its LICENSE are staged; rcedit gets the icon and the
# version; the console twin is staged as rbrowser.com; crash_reporter.cfg gets the version and no
# ServerURL; the size report names each part; the sidecar is staged with both bundles (main.js,
# flow-worker.js) and every production package but Sentry's (fake npm).
# scripts/lib/package.sh's --sign check (scripts/sign-windows.sh --check) and the unzip of
# run-windows.sh --package are here too.
#
# Pure bash and python3, a second.
#
#   scripts/tests/package-stage.sh
# shellcheck disable=SC2016  # the snippets are single-quoted on purpose: a child bash runs them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

SERVER="$SCRATCH/server"
CACHE="$SCRATCH/cache"
BIN="$SCRATCH/bin"
mkdir -p "$SERVER" "$CACHE" "$BIN"
# curl ... -o <file> <url>: copies the url's last segment from $SERVER, fails like curl -f.
cat > "$BIN/curl" << 'SH'
#!/usr/bin/env bash
out="" url=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        -o) out="$2"; shift 2 ;;
        -*) shift ;;
        *) url="$1"; shift ;;
    esac
done
echo "$url" >> "$RB_FAKE_SERVER/requests"
cp "$RB_FAKE_SERVER/${url##*/}" "$out" 2> /dev/null || exit 22
SH
chmod +x "$BIN/curl"

sha() { sha256sum "$1" | cut -d ' ' -f 1; }

# Runs snippet $1 in a bash that sourced the library, with the fakes first on PATH; $2... are
# its positional parameters.
in_lib() {
    local snippet="$1"
    shift
    run_status env PATH="$BIN:$PATH" RB_FAKE_SERVER="$SERVER" bash -c \
        'set -euo pipefail; source "$RB_ROOT/scripts/lib/package.sh"; source "$RB_ROOT/scripts/lib/package-stage.sh"; '"$snippet" \
        _ "$@"
}
export RB_ROOT="$ROOT"

# --- rb_fetch_pinned: the download is held to its pin.
printf 'payload\n' > "$SERVER/tool.bin"
PIN="$(sha "$SERVER/tool.bin")"
in_lib 'rb_fetch_pinned "https://example.invalid/d/tool.bin" "$1/tool.bin" "$2"' "$CACHE" "$PIN"
expect_status "a download with the pinned SHA-256 is kept" 0
expect_true "in the cache" '[[ -f "$CACHE/tool.bin" && ! -e "$CACHE/tool.bin.part" ]]'

rm -f "$SERVER/tool.bin" "$SERVER/requests"
in_lib 'rb_fetch_pinned "https://example.invalid/d/tool.bin" "$1/tool.bin" "$2"' "$CACHE" "$PIN"
expect_status "a cached file with the pin is taken again" 0
expect_true "without a download" '[[ ! -e "$SERVER/requests" ]]'

printf 'tampered\n' > "$SERVER/other.bin"
in_lib 'rb_fetch_pinned "https://example.invalid/d/other.bin" "$1/other.bin" "$2"' "$CACHE" "$PIN"
expect_status "a download with another SHA-256 stops the run" 1
expect_output "naming the pin" "$PIN"
expect_output "and what came" "$(sha "$SERVER/other.bin")"
expect_true "and nothing is kept" '[[ ! -e "$CACHE/other.bin" && ! -e "$CACHE/other.bin.part" ]]'

printf 'changed\n' > "$CACHE/tool.bin"
in_lib 'rb_fetch_pinned "https://example.invalid/d/tool.bin" "$1/tool.bin" "$2"' "$CACHE" "$PIN"
expect_status "a cached file that is no longer the pin stops the run" 1
expect_output "and says what to do" "remove $CACHE/tool.bin"

in_lib 'rb_fetch_pinned "https://example.invalid/d/missing.bin" "$1/missing.bin" "$2"' "$CACHE" "$PIN"
expect_status "a failed download stops the run" 1
expect_output "and is named" "could not download https://example.invalid/d/missing.bin"

# --- rb_stage_node: the .nvmrc version, the pinned ZIP, node.exe and LICENSE only.
TREE="$SCRATCH/tree"
STAGE="$SCRATCH/stage"
mkdir -p "$TREE"
echo "v9.9.9" > "$TREE/.nvmrc"
python3 -I - "$SERVER/node-v9.9.9-win-x64.zip" << 'PY'
import sys
import zipfile

with zipfile.ZipFile(sys.argv[1], "w") as archive:
    archive.writestr("node-v9.9.9-win-x64/node.exe", "MZ node 9.9.9\n")
    archive.writestr("node-v9.9.9-win-x64/LICENSE", "Node.js is licensed for use as follows: MIT\n")
    archive.writestr("node-v9.9.9-win-x64/npm.cmd", "npm\n")
    archive.writestr("node-v9.9.9-win-x64/node_modules/npm/package.json", "{}\n")
PY
NODE_PIN="$(sha "$SERVER/node-v9.9.9-win-x64.zip")"
stage_node() {
    rm -rf "$STAGE"
    in_lib 'RB_NODE_WIN_X64_VERSION="$1"; RB_NODE_WIN_X64_SHA256="$2"; rb_stage_node "$3" "$4" "$5"' \
        "$1" "$2" "$TREE" "$CACHE" "$STAGE"
}

stage_node 9.9.9 "$NODE_PIN"
expect_status "the pinned Node runtime is staged" 0
expect_true "node.exe from the ZIP" 'grep -q "MZ node 9.9.9" "$STAGE/node/node.exe"'
expect_true "with its LICENSE" '[[ -f "$STAGE/node/LICENSE" ]]'
expect_true "and nothing else" '[[ "$(ls "$STAGE/node" | tr "\n" " ")" == "LICENSE node.exe " ]]'
expect_true "from nodejs.org" 'grep -qF "https://nodejs.org/dist/v9.9.9/node-v9.9.9-win-x64.zip" "$SERVER/requests"'
expect_output "the version is reported" "node 9.9.9"

stage_node 9.9.8 "$NODE_PIN"
expect_status "a pin for another version than .nvmrc stops the run" 1
expect_output "and names both" "the pin is for node 9.9.8, .nvmrc says 9.9.9"

rm -f "$CACHE/node-v9.9.9-win-x64.zip"
stage_node 9.9.9 "0000000000000000000000000000000000000000000000000000000000000000"
expect_status "a Node ZIP with another SHA-256 stops the run" 1
expect_true "and stages nothing" '[[ ! -e "$STAGE/node/node.exe" ]]'

# --- rb_patch_exe: rcedit, held to its pin, gets the icon and the version.
cat > "$CACHE/rcedit-x64.exe" << 'SH'
#!/usr/bin/env bash
printf '%s\n' "$@" > "$RB_FAKE_SERVER/rcedit-args"
[[ "${RB_FAKE_RCEDIT_FAIL:-0}" == 0 ]]
SH
chmod +x "$CACHE/rcedit-x64.exe"
RCEDIT_PIN="$(sha "$CACHE/rcedit-x64.exe")"
printf 'MZ bootstrap\n' > "$SCRATCH/rbrowser.exe"
printf 'icon\n' > "$SCRATCH/rbrowser.ico"
patch_exe() {
    in_lib 'RB_RCEDIT_SHA256="$1"; rb_windows_path() { printf "%s\n" "$1"; }; rb_patch_exe "$2" "$3" 1.2.3 "$4"' \
        "$1" "$SCRATCH/rbrowser.exe" "$SCRATCH/rbrowser.ico" "$CACHE"
}
patch_exe "$RCEDIT_PIN"
expect_status "rcedit patches rbrowser.exe" 0
ARGS="$(cat "$SERVER/rcedit-args" 2> /dev/null || true)"
expect "the exe comes first" "$SCRATCH/rbrowser.exe" "$(head -n 1 <<< "$ARGS")"
expect_true "with the icon" 'grep -qxF -- "--set-icon" <<< "$ARGS" && grep -qxF "$SCRATCH/rbrowser.ico" <<< "$ARGS"'
expect_true "the file and product version" 'grep -A1 -xF -- "--set-file-version" <<< "$ARGS" | grep -qx 1.2.3 && grep -A1 -xF -- "--set-product-version" <<< "$ARGS" | grep -qx 1.2.3'
expect_true "and the version strings of rbrowser.dll" 'grep -A1 -xF ProductName <<< "$ARGS" | grep -qx rBrowser && grep -A1 -xF OriginalFilename <<< "$ARGS" | grep -qx rbrowser.exe'

patch_exe "1111111111111111111111111111111111111111111111111111111111111111"
expect_status "an rcedit that is not the pin is never run" 1
RB_FAKE_RCEDIT_FAIL=1 patch_exe "$RCEDIT_PIN"
expect_status "a failing rcedit stops the run" 1
expect_output "and is named" "rcedit failed"

# --- rb_stage_console: the console twin rbrowser-console.exe ships as rbrowser.com (RB-010-11).
BUILD="$SCRATCH/build"
mkdir -p "$BUILD" "$STAGE"
in_lib 'rb_stage_console "$1" "$2"' "$BUILD" "$STAGE"
expect_status "a build without the console twin stops the run" 1
expect_output "and names the package to build" "-p rbrowser-cli"
printf 'MZ console\n' > "$BUILD/rbrowser-console.exe"
in_lib 'rb_stage_console "$1" "$2"' "$BUILD" "$STAGE"
expect_status "the console twin is staged" 0
expect_true "as rbrowser.com beside rbrowser.exe" 'grep -q "MZ console" "$STAGE/rbrowser.com"'
expect_true "and not under its build name" '[[ ! -e "$STAGE/rbrowser-console.exe" ]]'

# --- rb_stage_updater: rbrowser-updater.exe ships beside rbrowser.exe (RB-090-14).
in_lib 'rb_stage_updater "$1" "$2"' "$BUILD" "$STAGE"
expect_status "a build without the updater stops the run" 1
expect_output "and names the package to build" "-p rb-update"
printf 'MZ updater\n' > "$BUILD/rbrowser-updater.exe"
in_lib 'rb_stage_updater "$1" "$2"' "$BUILD" "$STAGE"
expect_status "the updater is staged" 0
expect_true "beside rbrowser.exe under its own name" 'grep -q "MZ updater" "$STAGE/rbrowser-updater.exe"'

# --- crash_reporter.cfg from the template rb-diag holds to crash_reporter_cfg.
mkdir -p "$STAGE"
in_lib 'rb_write_crash_reporter_cfg "$1" "$2" 1.2.3' "$ROOT/packaging/windows/crash_reporter.cfg.in" "$STAGE"
expect_status "crash_reporter.cfg is written" 0
expect_true "with the version" 'grep -qx "ProductVersion=1.2.3" "$STAGE/crash_reporter.cfg"'
expect_true "and AppName" 'grep -qx "AppName=rBrowser" "$STAGE/crash_reporter.cfg"'
expect_true "no placeholder left" '! grep -q "@VERSION@" "$STAGE/crash_reporter.cfg"'
expect_true "no upload" '! grep -q "^ServerURL" "$STAGE/crash_reporter.cfg"'

# --- the size report.
SIZES="$SCRATCH/sizes"
mkdir -p "$SIZES/locales" "$SIZES/node" "$SIZES/ui"
head -c 1048576 /dev/zero > "$SIZES/rbrowser.exe"
head -c 2097152 /dev/zero > "$SIZES/rbrowser.dll"
head -c 262144 /dev/zero > "$SIZES/rbrowser.com"
head -c 524288 /dev/zero > "$SIZES/rbrowser-wsl"
head -c 262144 /dev/zero > "$SIZES/rbrowser-updater.exe"
head -c 3145728 /dev/zero > "$SIZES/libcef.dll"
head -c 1048576 /dev/zero > "$SIZES/resources.pak"
head -c 524288 /dev/zero > "$SIZES/locales/en-US.pak"
head -c 1048576 /dev/zero > "$SIZES/node/node.exe"
printf 'x\n' > "$SIZES/ui/index.html"
printf 'notice\n' > "$SIZES/NOTICE"
head -c 1048576 /dev/zero > "$SCRATCH/sizes.zip"
in_lib 'rb_package_sizes "$1" "$2"' "$SIZES" "$SCRATCH/sizes.zip"
expect_status "the size report is printed" 0
expect_output "rbrowser.exe" "     1.0 MB  rbrowser.exe"
expect_output "rbrowser.dll" "     2.0 MB  rbrowser.dll"
expect_output "the console twin" "     0.2 MB  rbrowser.com"
expect_output "the WSL helper, not the CEF runtime" "     0.5 MB  rbrowser-wsl"
expect_output "the updater, not the CEF runtime" "     0.2 MB  rbrowser-updater.exe"
expect_output "the CEF runtime" "     4.0 MB  CEF runtime"
expect_output "the locales" "     0.5 MB  CEF locales"
expect_output "Node" "     1.0 MB  Node runtime"
expect_output "the shell" "     0.0 MB  shell UI"
expect_output "the total" "     9.5 MB  total, unpacked"
expect_output "and the ZIP" "     1.0 MB  ZIP"

# --- --sign asks scripts/sign-windows.sh --check before anything is built (RB-090-03).
printf 'pfx\n' > "$SCRATCH/some-key.pfx"
printf '#!/usr/bin/env bash\nexit 0\n' > "$BIN/fake-osslsigncode"
chmod +x "$BIN/fake-osslsigncode"
in_lib 'unset RB_SIGN_PFX RB_SIGN_PKCS11_MODULE; rb_package_sign_check "$RB_ROOT"'
expect_status "--sign without a key stops the run" 1
expect_output "with the note" "signing not configured"
expect_output "before the build" "Nothing was built"
in_lib 'RB_SIGN_PFX="$1" RB_SIGN_PFX_PASSWORD=some-secret RB_OSSLSIGNCODE="$2" RB_SIGN_BACKEND=osslsigncode rb_package_sign_check "$RB_ROOT"' \
    "$SCRATCH/some-key.pfx" "$BIN/fake-osslsigncode"
expect_status "--sign with a key and osslsigncode goes on" 0
expect_true "the password is never printed" '! grep -q some-secret <<< "$output"'
expect_true "nor the key's path" '! grep -q some-key <<< "$output"'

# --- rb_stage_sidecar: both bundles and every production dependency (RB-040-07: the flow worker
# needs @playwright/test, which needs playwright, which needs playwright-core; Lighthouse needs
# its tree and puppeteer-core) but what only Sentry needs (Lighthouse's error reporting, never
# switched on; the `sentry` package is under the FSL). A fake npm records its calls and, for
# `npm ci --omit=dev`, installs every production package of the lock file it finds, a `.bin`
# folder too; RB_FAKE_NPM_SKIP leaves one out.
cat > "$BIN/npm" << 'SH'
#!/usr/bin/env bash
echo "$(basename "$PWD"): $*" >> "$RB_FAKE_NPM_LOG"
if [[ "$1" == ci && " $* " == *" --omit=dev "* ]]; then
    python3 -I - package-lock.json "${RB_FAKE_NPM_SKIP:-}" << 'PY'
import json
import os
import sys

lock, skip = sys.argv[1], sys.argv[2]
with open(lock, encoding="utf-8") as handle:
    packages = json.load(handle)["packages"]
for path, entry in packages.items():
    if not path or entry.get("dev") or path == f"node_modules/{skip}":
        continue
    os.makedirs(path, exist_ok=True)
    with open(os.path.join(path, "package.json"), "w", encoding="utf-8") as out:
        json.dump({"version": entry["version"]}, out)
    for name in ("LICENSE", "NOTICE", "LICENSE.txt", "NOTICE.md", "ThirdPartyNotices.txt",
                 "index.js", "index.js.map", "index.d.ts", "README.md", "notes.txt"):
        with open(os.path.join(path, name), "w", encoding="utf-8") as out:
            out.write(name + "\n")
os.makedirs("node_modules/.bin", exist_ok=True)
open("node_modules/.bin/lighthouse", "w").close()
PY
fi
SH
chmod +x "$BIN/npm"
export RB_FAKE_NPM_LOG="$SCRATCH/npm.log"
SIDECAR_SRC="$SCRATCH/src/sidecar"
SIDECAR_STAGE="$SCRATCH/stage/sidecar"
mkdir -p "$SIDECAR_SRC"
printf '{"type":"module"}\n' > "$SIDECAR_SRC/package.json"
cat > "$SIDECAR_SRC/package-lock.json" << 'JSON'
{
  "lockfileVersion": 3,
  "packages": {
    "": {
      "dependencies": { "@playwright/test": "1.65.0", "lighthouse": "13.6.0",
                        "playwright-core": "1.65.0", "puppeteer-core": "25.17.0" },
      "devDependencies": { "vitest": "5.1.0" }
    },
    "node_modules/@playwright/test": { "version": "1.65.0", "dependencies": { "playwright": "1.65.0" } },
    "node_modules/playwright": { "version": "1.65.0", "dependencies": { "playwright-core": "1.65.0" } },
    "node_modules/playwright-core": { "version": "1.65.0" },
    "node_modules/lighthouse": { "version": "13.6.0",
      "dependencies": { "@sentry/node": "^11", "debug": "^4", "puppeteer-core": "^25", "yargs": "^18" } },
    "node_modules/lighthouse/node_modules/yargs": { "version": "18.2.0" },
    "node_modules/puppeteer-core": { "version": "25.17.0", "dependencies": { "debug": "^4" } },
    "node_modules/debug": { "version": "4.4.3" },
    "node_modules/@sentry/node": { "version": "11.8.0", "dependencies": { "sentry": "^0", "debug": "^4" } },
    "node_modules/sentry": { "version": "0.49.0", "dependencies": { "magic-string": "^0" } },
    "node_modules/magic-string": { "version": "0.30.21" },
    "node_modules/vitest": { "version": "5.1.0", "dev": true }
  }
}
JSON
in_lib 'rb_stage_sidecar "$1" "$2"' "$SIDECAR_SRC" "$SIDECAR_STAGE"
expect_status "a sidecar without its bundle is not staged" 1
expect_output "and the bundle is named" "dist/main.js is missing"
mkdir -p "$SIDECAR_SRC/dist"
printf 'export {};\n' > "$SIDECAR_SRC/dist/main.js"
in_lib 'rb_stage_sidecar "$1" "$2"' "$SIDECAR_SRC" "$SIDECAR_STAGE"
expect_status "a sidecar without its flow worker is not staged" 1
expect_output "and the worker is named" "dist/flow-worker.js is missing"
printf 'export {};\n' > "$SIDECAR_SRC/dist/flow-worker.js"
RB_FAKE_NPM_SKIP=@playwright/test in_lib 'rb_stage_sidecar "$1" "$2"' "$SIDECAR_SRC" "$SIDECAR_STAGE"
expect_status "a production install without @playwright/test is not staged" 1
expect_output "and the package is named" "has no @playwright/test"
RB_FAKE_NPM_SKIP=lighthouse in_lib 'rb_stage_sidecar "$1" "$2"' "$SIDECAR_SRC" "$SIDECAR_STAGE"
expect_status "nor one without lighthouse" 1
expect_output "named too" "has no lighthouse"
in_lib 'rb_sidecar_packages "$1"' "$SIDECAR_SRC/package-lock.json"
expect_status "the dry listing of what is staged reads the lock file alone" 0
expect_output "it names Lighthouse" "node_modules/lighthouse"
expect_true "and leaves out Sentry and what only Sentry needs" \
    '! grep -qE "sentry|magic-string|vitest" <<< "$output"'
rm -f "$RB_FAKE_NPM_LOG"
in_lib 'rb_stage_sidecar "$1" "$2"' "$SIDECAR_SRC" "$SIDECAR_STAGE"
expect_status "the sidecar is staged" 0
expect_true "with its bundle" '[[ -f "$SIDECAR_STAGE/dist/main.js" ]]'
expect_true "with the flow worker beside it" '[[ -f "$SIDECAR_STAGE/dist/flow-worker.js" ]]'
expect_true "with its package.json (an ES module)" '[[ -f "$SIDECAR_STAGE/package.json" ]]'
for pkg in playwright-core playwright @playwright/test lighthouse lighthouse/node_modules/yargs \
    puppeteer-core debug; do
    expect_true "with $pkg and its licence and NOTICE" \
        "[[ -f \"\$SIDECAR_STAGE/node_modules/$pkg/LICENSE\" && -f \"\$SIDECAR_STAGE/node_modules/$pkg/NOTICE\" ]]"
done
# The prune beyond the lock file (RB-080-25): source maps, type declarations and the packages'
# prose are not shipped; what names a licence or a notice stays, whatever its suffix.
for name in index.js.map index.d.ts README.md notes.txt; do
    expect_true "without $name in any package" \
        "[[ -z \"\$(find \"\$SIDECAR_STAGE/node_modules\" -name '$name' -print -quit)\" ]]"
done
for name in index.js LICENSE.txt NOTICE.md ThirdPartyNotices.txt; do
    expect_true "but with $name" "[[ -f \"\$SIDECAR_STAGE/node_modules/playwright-core/$name\" ]]"
done
for pkg in @sentry/node sentry magic-string vitest .bin; do
    expect_true "without $pkg" "[[ ! -e \"\$SIDECAR_STAGE/node_modules/$pkg\" ]]"
done
expect_true "the production install leaves out dev dependencies and install scripts" \
    'grep -qF "ci --omit=dev --ignore-scripts" "$RB_FAKE_NPM_LOG"'
expect_true "outside the source tree" '[[ ! -e "$SIDECAR_SRC/node_modules" ]]'
expect_true "and no placeholder README" '[[ ! -e "$SIDECAR_STAGE/README.txt" ]]'
in_lib 'rb_stage_sidecar "$1" "$2"' "$SIDECAR_SRC" "$SIDECAR_STAGE"
expect_status "a second stage over the first" 0
expect_true "nests no package in itself" '[[ ! -e "$SIDECAR_STAGE/node_modules/playwright/playwright" ]]'
expect_true "nor a node_modules in node_modules" '[[ ! -e "$SIDECAR_STAGE/node_modules/node_modules" ]]'

# --- rb_package_unzip (run-windows.sh --package).
python3 -I - "$SCRATCH/pkg.zip" << 'PY'
import sys
import zipfile

with zipfile.ZipFile(sys.argv[1], "w") as archive:
    archive.writestr("rbrowser-1.2.3-windows-x64/rbrowser.exe", "MZ\n")
    archive.writestr("rbrowser-1.2.3-windows-x64/locales/en-US.pak", "pak\n")
PY
mkdir -p "$SCRATCH/unpacked/rbrowser-1.2.3-windows-x64"
printf 'old\n' > "$SCRATCH/unpacked/rbrowser-1.2.3-windows-x64/stale.dll"
in_lib 'rb_package_unzip "$1" "$2"' "$SCRATCH/pkg.zip" "$SCRATCH/unpacked"
expect_status "the package is unpacked" 0
expect_output "naming its folder" "$SCRATCH/unpacked/rbrowser-1.2.3-windows-x64"
expect_true "with its files" '[[ -f "$SCRATCH/unpacked/rbrowser-1.2.3-windows-x64/locales/en-US.pak" ]]'
expect_true "and none of an older one" '[[ ! -e "$SCRATCH/unpacked/rbrowser-1.2.3-windows-x64/stale.dll" ]]'

python3 -I - "$SCRATCH/evil.zip" << 'PY'
import sys
import zipfile

with zipfile.ZipFile(sys.argv[1], "w") as archive:
    archive.writestr("rbrowser-1.2.3-windows-x64/rbrowser.exe", "MZ\n")
    archive.writestr("other/rbrowser.exe", "MZ\n")
PY
in_lib 'rb_package_unzip "$1" "$2"' "$SCRATCH/evil.zip" "$SCRATCH/unpacked2"
expect_status "a ZIP with more than one top folder is refused" 1

finish_tests package-stage

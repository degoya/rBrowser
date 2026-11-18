#!/usr/bin/env bash
#
# scripts/package-windows.sh without a build: copies in a scratch
# repository whose scripts/build-plugins.sh is a stand-in that notes it was reached and stops the
# run with exit 3, so a run that gets past the checks of scripts/lib/package.sh ends there. A
# package of the same version from another commit or a dirty tree, an empty version and a
# failing scripts/set-version.sh --check stop the run before it; RB_REPACK=1 replaces the package.
# A dirty tree stops the run unless --allow-dirty; --sign, and RB_SIGN=1, hold the signing
# configuration (scripts/sign-windows.sh --check) before anything is built: no key or no
# osslsigncode ends with "signing not configured" and leaves an existing package as it was
# (RB-010-15, RB-090-03); without them osslsigncode is never needed. A stage that carries the portable mode's marker `portable` or a
# data folder beside the program is never packaged. With a fake npm: each front end is built with
# `npm ci --ignore-scripts` (the sidecar's stage is tested in scripts/tests/package-stage.sh).
#
# Pure bash and python3, a second.
#
#   scripts/tests/package.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
TREE="$SCRATCH/repo"
REACHED="$SCRATCH/reached"

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

mkdir -p "$TREE/scripts/lib" "$TREE/artifacts/windows"
cp "$ROOT/scripts/package-windows.sh" "$TREE/scripts/"
cp "$ROOT/scripts/lib/jobs.sh" "$ROOT/scripts/lib/lock.sh" "$ROOT/scripts/lib/package.sh" \
    "$ROOT/scripts/lib/workspace-version.sh" "$ROOT/scripts/lib/cef.sh" \
    "$ROOT/scripts/lib/tool-versions.sh" "$ROOT/scripts/lib/package-stage.sh" \
    "$ROOT/scripts/lib/sign.sh" "$ROOT/scripts/lib/sign_plan.py" \
    "$ROOT/scripts/lib/wsl-helper.sh" "$TREE/scripts/lib/"
cp "$ROOT/scripts/sign-windows.sh" "$TREE/scripts/"
# The stand-in also notes which signing variables the build would inherit: none.
printf '%s\n' '#!/usr/bin/env bash' \
    'env | sed -n "s/^\(RB_SIGN_[A-Z_]*\|RB_UPDATE_SIGNING_KEY\)=.*/\1/p" > "$RB_FAKE_REACHED.env"' \
    'echo reached > "$RB_FAKE_REACHED"' 'exit 3' \
    > "$TREE/scripts/build-plugins.sh"
printf '%s\n' '#!/usr/bin/env bash' '[[ "${RB_FAKE_VERSION_CHECK:-ok}" == ok ]]' \
    > "$TREE/scripts/set-version.sh"
printf '%s\n' '[workspace]' '' '[workspace.package]' 'version = "1.2.3"' > "$TREE/Cargo.toml"
printf '%s\n' '/artifacts/' > "$TREE/.gitignore"
chmod +x "$TREE"/scripts/*.sh
git -C "$TREE" init -q
git -C "$TREE" add -A
git -C "$TREE" -c user.name=test -c user.email=test@example.invalid commit -qm tree
HEAD_COMMIT="$(git -C "$TREE" rev-parse --short=8 HEAD)"
ZIP="$TREE/artifacts/windows/rbrowser-1.2.3-windows-x64.zip"

# A Windows package whose VERSION.txt names commit $1.
zip_built_from() {
    python3 -I - "$ZIP" "$1" << 'PY'
import sys
import zipfile

with zipfile.ZipFile(sys.argv[1], "w") as package:
    package.writestr("rbrowser-1.2.3-windows-x64/VERSION.txt",
                     f"rBrowser 1.2.3\ncommit {sys.argv[2]}\nbuilt now\n")
PY
}

package() {
    local script="$1"
    shift
    rm -f "$REACHED" "$REACHED.env"
    run_status env RB_NO_LOCK=1 RB_MIN_FREE_MB=0 RB_NO_SCCACHE=1 RB_FAKE_REACHED="$REACHED" \
        "$TREE/scripts/package-$script.sh" "$@"
}
reached() { [[ -f "$REACHED" ]]; }

package windows
expect_status "no package yet: the build starts" 3
expect_output "the package is named by the version" "artifacts/windows/rbrowser-1.2.3-windows-x64.zip"

zip_built_from deadbeef
package windows
expect_status "the same version from another commit stops the run" 1
expect_output "and names both commits" "built from commit deadbeef; this build is $HEAD_COMMIT"
expect_true "before anything is built" '! reached'

RB_REPACK=1 package windows
expect_status "RB_REPACK=1 replaces it" 3

zip_built_from "$HEAD_COMMIT"
package windows
expect_status "the same commit builds again" 3

touch "$TREE/untracked.txt"
package windows --allow-dirty
expect_status "an untracked file makes the tree dirty; an existing package stops even --allow-dirty" 1
expect_output "named -dirty" "this build is $HEAD_COMMIT-dirty"
rm -f "$ZIP"
package windows
expect_status "a dirty tree stops the run without --allow-dirty" 1
expect_output "with a clear message" "the working tree is dirty ($HEAD_COMMIT-dirty)"
expect_output "naming the way out" "--allow-dirty"
expect_true "before anything is built" '! reached'
package windows --allow-dirty
expect_status "--allow-dirty builds a dirty tree" 3
expect_output "and says so" "built from a dirty tree"
rm -f "$TREE/untracked.txt"

zip_built_from "$HEAD_COMMIT"
before="$(sha256sum "$ZIP")"
package windows --sign
expect_status "--sign without a key stops the run" 1
expect_output "with the note" "signing not configured"
expect_true "before anything is built" '! reached'
expect "and the package is unchanged" "$before" "$(sha256sum "$ZIP")"
RB_SIGN=1 package windows
expect_status "RB_SIGN=1 asks for signing as --sign does" 1
expect_output "with the note" "signing not configured"
expect_true "before anything is built" '! reached'
expect "the package is unchanged again" "$before" "$(sha256sum "$ZIP")"
# A key, and an osslsigncode that records being called: the configuration is held before the
# build, nothing is signed before it; without --sign the tool is never needed.
KEYS="$SCRATCH/keys"
mkdir -p "$KEYS"
printf 'pfx\n' > "$KEYS/test.pfx"
printf 'secret-password' > "$KEYS/test.pass"
printf '%s\n' '#!/usr/bin/env bash' 'echo "$*" >> "$RB_FAKE_SIGNER_CALLS"' > "$KEYS/osslsigncode"
chmod +x "$KEYS/osslsigncode"
export RB_FAKE_SIGNER_CALLS="$SCRATCH/signer-calls"
RB_SIGN_BACKEND=osslsigncode RB_SIGN_PFX="$KEYS/test.pfx" RB_SIGN_PFX_PASSWORD_FILE="$KEYS/test.pass" \
    RB_OSSLSIGNCODE="$SCRATCH/nowhere/osslsigncode" package windows --sign
expect_status "--sign without osslsigncode stops the run" 1
expect_output "naming the tool" "osslsigncode is not installed"
expect_true "before anything is built" '! reached'
expect "and the package is unchanged" "$before" "$(sha256sum "$ZIP")"
expect_true "the password is never printed" '! grep -q secret-password <<< "$output"'
RB_SIGN_BACKEND=osslsigncode RB_SIGN_PFX="$KEYS/test.pfx" RB_SIGN_PFX_PASSWORD_FILE="$KEYS/test.pass" \
    RB_OSSLSIGNCODE="$KEYS/osslsigncode" package windows --sign
expect_status "--sign with a key and the tool goes on to the build" 3
expect_output "naming how it will sign" "signing with osslsigncode"
expect_true "nothing is signed before the build" '[[ ! -e "$RB_FAKE_SIGNER_CALLS" ]]'
expect_true "the password is never printed" '! grep -q secret-password <<< "$output"'
expect "the build inherits no signing variable" "" "$(cat "$REACHED.env")"
RB_SIGN_BACKEND=osslsigncode RB_SIGN_PFX="$KEYS/test.pfx" RB_SIGN_PFX_PASSWORD=secret-password \
    RB_UPDATE_SIGNING_KEY="$KEYS/update.key" RB_OSSLSIGNCODE="$KEYS/osslsigncode" package windows --sign
expect_status "a password in the environment goes on to the build too" 3
expect "and the build inherits neither it nor the update key" "" "$(cat "$REACHED.env")"
expect_true "the password is never printed" '! grep -q secret-password <<< "$output"'
RB_OSSLSIGNCODE="$SCRATCH/nowhere/osslsigncode" package windows
expect_status "without --sign no osslsigncode is needed" 3
expect_true "and no signer is asked" '! grep -q "signing" <<< "$output"'

zip_built_from "$HEAD_COMMIT-dirty"
package windows
expect_status "a package from a dirty tree is never taken for this build" 1

printf 'not a zip\n' > "$ZIP"
package windows
expect_status "a package without a readable VERSION.txt stops the run" 1
expect_output "and says so" "built from commit (not readable)"
rm -f "$ZIP"

RB_FAKE_VERSION_CHECK=fail package windows
expect_status "a failing set-version.sh --check stops the run" 1
expect_output "and is named" "scripts/set-version.sh --check failed"
expect_true "before anything is built" '! reached'

cp "$TREE/Cargo.toml" "$SCRATCH/Cargo.toml"
printf '%s\n' '[workspace]' > "$TREE/Cargo.toml"
package windows
expect_status "an empty version stops the run" 1
expect_output "and is named" "no [workspace.package] version"
expect_true "no package without a version is written" '[[ ! -e "$TREE/artifacts/windows/rbrowser--windows-x64.zip" ]]'
cp "$SCRATCH/Cargo.toml" "$TREE/Cargo.toml"

# The console twin (RB-010-11): built with the app and staged as rbrowser.com; its staging is
# tested in scripts/tests/package-stage.sh.
expect_true "the package builds the console twin with the app" \
    'grep -qF -- "-p rbrowser -p rbrowser-cli" "$ROOT/scripts/package-windows.sh"'
expect_true "and stages it as rbrowser.com" 'grep -q "^rb_stage_console " "$ROOT/scripts/package-windows.sh"'
# The updater (RB-090-14): built with the app, staged beside rbrowser.exe, and the app's DLL
# carries rb-update now (the positive control of scripts/check-update-code.sh).
expect_true "the package builds the updater with the app" \
    'grep -qF -- "-p rbrowser -p rbrowser-cli -p rb-update" "$ROOT/scripts/package-windows.sh"'
expect_true "and stages it beside rbrowser.exe" 'grep -q "^rb_stage_updater " "$ROOT/scripts/package-windows.sh"'
expect_true "the DLL is held to carry the self-update" \
    'grep -qF "check-update-code.sh --expect-present" "$ROOT/scripts/package-windows.sh"'
# The WSL helper (RB-030-04): built for musl and staged beside rbrowser.exe.
expect_true "the WSL helper is built" 'grep -q "rb_wsl_helper_build \"\$profile\"" "$ROOT/scripts/package-windows.sh"'
expect_true "and staged beside rbrowser.exe" 'grep -q "^rb_stage_wsl_helper " "$ROOT/scripts/package-windows.sh"'

# The portable mode: rb_package_zip refuses a stage with the marker `portable` or with data
# beside the program, and packs one without.
STAGES="$SCRATCH/stages"
zip_stage() {
    rm -f "$STAGES/stage.zip"
    run_status bash -c 'source "$1/scripts/lib/package.sh" && rb_package_zip "$2" stage "$3"' \
        _ "$ROOT" "$STAGES" "$STAGES/stage.zip"
}
mkdir -p "$STAGES/stage/plugins"
printf 'program\n' > "$STAGES/stage/rbrowser.exe"
printf 'package\n' > "$STAGES/stage/plugins/data"
printf 'package\n' > "$STAGES/stage/plugins/portable"
zip_stage
expect_status "a stage without marker or data beside the program is zipped" 0
expect_true "the zip is written" '[[ -f "$STAGES/stage.zip" ]]'

: > "$STAGES/stage/portable"
zip_stage
expect_status "the portable marker beside the program stops the zip" 1
expect_output "and is named" "stage/portable"
expect_true "no zip is written" '[[ ! -e "$STAGES/stage.zip" ]]'
rm -f "$STAGES/stage/portable"

mkdir -p "$STAGES/stage/data"
zip_stage
expect_status "a data folder beside the program stops the zip" 1
expect_output "and is named" "stage/data"
rm -rf "$STAGES/stage/data"

mkdir -p "$STAGES/stage/data"
mv "$STAGES/stage/rbrowser.exe" "$STAGES/stage/rbrowser"
run_status bash -c 'source "$1/scripts/lib/package.sh" && rb_package_unportable "$2"' \
    _ "$ROOT" "$STAGES/stage"
expect_status "data beside a Unix rbrowser is refused too" 1

# The bundled plugins: a test-only plugin (marker TEST-ONLY) is never packaged (RB-060-12).
PLUGINS_TREE="$SCRATCH/plugins-tree"
PREBUILT="$SCRATCH/prebuilt"
mkdir -p "$PLUGINS_TREE/scripts" "$PLUGINS_TREE/plugins/alpha" "$PLUGINS_TREE/plugins/hello-panel" \
    "$PREBUILT"
printf '%s\n' '#!/usr/bin/env bash' 'printf "alpha\nhello-panel\n"' \
    > "$PLUGINS_TREE/scripts/build-plugins.sh"
chmod +x "$PLUGINS_TREE/scripts/build-plugins.sh"
for name in alpha hello-panel; do
    printf 'version = "0.1.0"\n' > "$PLUGINS_TREE/plugins/$name/manifest.toml"
    : > "$PREBUILT/$name-0.1.0.rbplugin"
done
: > "$PLUGINS_TREE/plugins/hello-panel/TEST-ONLY"
run_status bash -c 'source "$1/scripts/lib/package.sh" && rb_package_plugins "$2" "$3"' \
    _ "$ROOT" "$PLUGINS_TREE" "$PREBUILT"
expect_status "the plugins are listed" 0
expect "a TEST-ONLY plugin is left out of the package" "$PREBUILT/alpha-0.1.0.rbplugin" "$output"

# The front ends, with a fake npm that records its calls.
FAKE_BIN="$SCRATCH/bin"
mkdir -p "$FAKE_BIN"
cat > "$FAKE_BIN/npm" << 'SH'
#!/usr/bin/env bash
echo "$(basename "$PWD"): $*" >> "$RB_FAKE_NPM_LOG"
SH
chmod +x "$FAKE_BIN/npm"
NPM_LOG="$SCRATCH/npm.log"
in_stage_lib() {
    rm -f "$NPM_LOG"
    run_status env PATH="$FAKE_BIN:$PATH" RB_FAKE_NPM_LOG="$NPM_LOG" bash -c \
        'source "$1/scripts/lib/package-stage.sh" && eval "$2"' _ "$ROOT" "$1"
}

FRONT="$SCRATCH/front/agent"
mkdir -p "$FRONT"
in_stage_lib "rb_build_frontend '$FRONT'"
expect_status "a front end builds" 0
expect "without install scripts, then its build" \
    "agent: ci --ignore-scripts --no-audit --no-fund|agent: run build" "$(paste -sd '|' "$NPM_LOG")"

finish_tests package

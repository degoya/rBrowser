#!/usr/bin/env bash
#
# scripts/sign-update.sh (RB-090-02) without a build: copies it into a scratch repository and
# hands it a stand-in rb-pack (RB_PACK) that records its arguments. Without RB_UPDATE_SIGNING_KEY,
# on the wrong channel for the version, without the ZIP of scripts/package-windows.sh and with a
# ZIP of another version nothing is written and rb-pack is never reached; with all of them it
# builds from the ZIP with the version's CHANGELOG section as notes and verifies the result
# against the public key beside the private one. The key's content never reaches the output, and
# RB_UPDATE_SIGNING_KEY reaches no child process (a build script would read it).
#
# Pure bash and python3, a second.
#
#   scripts/tests/sign-update.sh
# shellcheck disable=SC2016,SC2034  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
TREE="$SCRATCH/repo"
CALLS="$SCRATCH/calls"

mkdir -p "$TREE/scripts/lib" "$TREE/artifacts/windows" "$SCRATCH/keys"
cp "$ROOT/scripts/sign-update.sh" "$ROOT/scripts/release-assets.sh" "$TREE/scripts/"
cp "$ROOT/scripts/lib/workspace-version.sh" "$TREE/scripts/lib/"
printf '%s\n' '[workspace]' '' '[workspace.package]' 'version = "1.2.0"' > "$TREE/Cargo.toml"
printf '%s\n' '# Changelog' '' '## [1.2.0] - 2027-01-01' '' '### Added' '' '- Self-update.' \
    > "$TREE/CHANGELOG.md"
# The stand-in: one line per call with its arguments, and the manifest file for `build`.
cat > "$SCRATCH/rb-pack" <<'PACK'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$RB_FAKE_CALLS"
printf '%s\n' "${RB_UPDATE_SIGNING_KEY:-unset}" >> "$RB_FAKE_CALLS.env"
if [[ "$1" == build ]]; then
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --out) mkdir -p "$(dirname "$2")" && echo '{}' > "$2"; shift ;;
            --notes) cp "$2" "$RB_FAKE_CALLS.notes"; shift ;;
        esac
        shift
    done
fi
PACK
chmod +x "$SCRATCH/rb-pack"
KEY="$SCRATCH/keys/rbrowser-update.key"
# No key at all: the stand-in never reads it. A marker to look for in the output.
echo "stand-in-key c2VjcmV0LWtleS1tYXRlcmlhbA" > "$KEY"
echo "cHVibGlj" > "$SCRATCH/keys/rbrowser-update.pub"

zip_of() {
    python3 -I - "$1" "$2" <<'PY'
import sys, zipfile
with zipfile.ZipFile(sys.argv[1], "w") as archive:
    archive.writestr("rbrowser-1.2.0-windows-x64/VERSION.txt", f"rBrowser {sys.argv[2]}\ncommit abc\n")
    archive.writestr("rbrowser-1.2.0-windows-x64/rbrowser.exe", "program")
PY
}

sign() {
    rm -f "$CALLS" "$CALLS.env"
    run_status env RB_PACK="$SCRATCH/rb-pack" RB_FAKE_CALLS="$CALLS" "$@" \
        "$TREE/scripts/sign-update.sh" "${channel:-stable}"
}

channel=stable sign env -u RB_UPDATE_SIGNING_KEY
expect_status "no key, no run" 1
expect_output "it says which variable" "RB_UPDATE_SIGNING_KEY"
expect_true "rb-pack is never reached" '[[ ! -e "$CALLS" ]]'

channel=beta sign RB_UPDATE_SIGNING_KEY="$KEY"
expect_status "a plain version is no beta" 1
expect_true "rb-pack is never reached on the wrong channel" '[[ ! -e "$CALLS" ]]'

channel=stable sign RB_UPDATE_SIGNING_KEY="$KEY"
expect_status "no ZIP, no manifest" 1
expect_output "it names the package script" "package-windows.sh"

zip_of "$TREE/artifacts/windows/rbrowser-1.2.0-windows-x64.zip" 1.1.0
channel=stable sign RB_UPDATE_SIGNING_KEY="$KEY"
expect_status "a ZIP of another version is refused" 1
expect_true "nothing is written" '[[ ! -e "$TREE/artifacts/update" ]]'

zip_of "$TREE/artifacts/windows/rbrowser-1.2.0-windows-x64.zip" 1.2.0
channel=stable sign RB_UPDATE_SIGNING_KEY="$KEY"
expect_status "the release's ZIP is signed" 0
expect_true "the manifest of the channel is written" \
    '[[ -f "$TREE/artifacts/update/rbrowser-update-stable.json" ]]'
build="$(sed -n 1p "$CALLS")"
verify="$(sed -n 2p "$CALLS")"
expect_true "build gets the channel, the version and the key's path" \
    '[[ "$build" == "build --channel stable --version 1.2.0 "* && "$build" == *"--key $KEY"* ]]'
expect_true "the base is the version's GitHub release" \
    '[[ "$build" == *"--base-url https://github.com/degoya/rBrowser/releases/download/v1.2.0/"* ]]'
expect "the notes are the CHANGELOG section" "$(printf '### Added\n\n- Self-update.')" \
    "$(cat "$CALLS.notes")"
expect_true "verify checks the ZIP against the public key beside the key" \
    '[[ "$verify" == "verify $TREE/artifacts/update/rbrowser-update-stable.json --assets "* && "$verify" == *"--public-key $SCRATCH/keys/rbrowser-update.pub" ]]'
expect_true "the key's content never reaches the output" '[[ "$output" != *c2VjcmV0* ]]'
expect "no child inherits the key's variable: rb-pack gets the path as --key" \
    "$(printf 'unset\nunset')" "$(cat "$CALLS.env")"

finish_tests sign-update

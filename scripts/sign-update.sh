#!/usr/bin/env bash
#
# Builds and signs the update manifest of the packaged release for one channel (RB-090-02):
# takes the ZIP of scripts/package-windows.sh (RB-090-01) from artifacts/windows/, insists on the
# workspace version in the ZIP's VERSION.txt and on the version's channel (a pre-release is beta),
# takes the notes from the version's CHANGELOG.md section (scripts/release-assets.sh notes),
# signs with rb-pack, verifies the round trip and writes
# artifacts/update/rbrowser-update-<channel>.json. Uploading is release.yml's step, switched on by
# the repository variable RB_SELF_UPDATE (RB-110-05); self-update stays off until then.
#
# Usage:
#   scripts/sign-update.sh <stable|beta>
#
# Environment:
#   RB_UPDATE_SIGNING_KEY  the path of the PKCS#8 PEM private key (`rb-pack keygen`); required.
#                          Its place is the owner's (docs/security/update-signing.md) and never
#                          named here; the key is read by rb-pack only and never printed.
#   RB_UPDATE_PUBLIC_KEY   the base64 public key, or a file holding it, the round trip verifies
#                          against while this build's root is still the placeholder; default the
#                          `.pub` beside the key, when there is one.
#   RB_UPDATE_BASE_URL     the https:// folder the ZIP is published under (default the GitHub
#                          release of the version, https://github.com/degoya/rBrowser/...).
#   RB_PACK                a prebuilt rb-pack (release.yml builds it in a step of its own, so no
#                          build runs while the key is on the runner); else built here, under the
#                          build lock.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -z "${RB_PACK:-}" ]]; then
    # shellcheck source=lib/lock.sh
    source "$ROOT/scripts/lib/lock.sh"
    rb_take_lock "$@"
fi
cd "$ROOT"
# shellcheck source=lib/workspace-version.sh
source "$ROOT/scripts/lib/workspace-version.sh"

usage() {
    echo "usage: scripts/sign-update.sh <stable|beta>" >&2
    exit 2
}

[[ $# -eq 1 ]] || usage
CHANNEL="$1"
case "$CHANNEL" in stable | beta) ;; *) usage ;; esac

KEY="${RB_UPDATE_SIGNING_KEY:-}"
# The path stays in this shell; no child - the rb-pack build's scripts above all - inherits it.
# rb-pack gets it as --key.
[[ -z "${RB_UPDATE_SIGNING_KEY+set}" ]] || export -n RB_UPDATE_SIGNING_KEY
if [[ -z "$KEY" || ! -f "$KEY" ]]; then
    echo "!! no update signing key: set RB_UPDATE_SIGNING_KEY to the key's path" >&2
    echo "   (rb-pack keygen; docs/security/update-signing.md); nothing was written" >&2
    exit 1
fi
PUBLIC="${RB_UPDATE_PUBLIC_KEY:-${KEY%.key}.pub}"
OUT_DIR="$ROOT/artifacts/update"
OUT="$OUT_DIR/rbrowser-update-$CHANNEL.json"
VERSION="$(rb_workspace_version < Cargo.toml)"
[[ -n "$VERSION" ]] || {
    echo "!! no workspace version in Cargo.toml" >&2
    exit 1
}

# The version's beta-ness decides the channel; a mismatch stops here.
if [[ "$VERSION" == *-* && "$CHANNEL" != beta ]] || [[ "$VERSION" != *-* && "$CHANNEL" != stable ]]; then
    echo "!! $VERSION does not belong on the $CHANNEL channel; nothing was written" >&2
    exit 1
fi

ZIP="$ROOT/artifacts/windows/rbrowser-$VERSION-windows-x64.zip"
[[ -f "$ZIP" ]] || {
    echo "!! $ZIP is missing (scripts/package-windows.sh); nothing was written" >&2
    exit 1
}
version_file="$(python3 -I -c '
import sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as archive:
    names = [name for name in archive.namelist() if name.endswith("/VERSION.txt")]
    print(archive.read(names[0]).decode() if names else "")
' "$ZIP")"
if ! head -n 1 <<< "$version_file" | grep -qx "rBrowser $VERSION"; then
    echo "!! the ZIP's VERSION.txt is not rBrowser $VERSION; nothing was written" >&2
    exit 1
fi

pack() {
    if [[ -n "${RB_PACK:-}" ]]; then
        "$RB_PACK" "$@"
    else
        cargo build --release --locked -p rb-update --bin rb-pack -q
        "$ROOT/target/release/rb-pack" "$@"
    fi
}

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
ln -s "$ZIP" "$STAGE/$(basename "$ZIP")"
notes_args=()
if "$ROOT/scripts/release-assets.sh" notes "$VERSION" > "$STAGE/notes.txt" 2> /dev/null; then
    notes_args=(--notes "$STAGE/notes.txt")
else
    echo "warning: CHANGELOG.md has no section for $VERSION; the manifest carries no notes" >&2
fi
base_url="${RB_UPDATE_BASE_URL:-https://github.com/degoya/rBrowser/releases/download/v$VERSION/}"

mkdir -p "$OUT_DIR"
pack build \
    --channel "$CHANNEL" \
    --version "$VERSION" \
    --assets "$STAGE" \
    --base-url "$base_url" \
    --key "$KEY" \
    "${notes_args[@]}" \
    --out "$OUT"
verify_args=()
[[ -e "$PUBLIC" || -n "${RB_UPDATE_PUBLIC_KEY:-}" ]] && verify_args=(--public-key "$PUBLIC")
pack verify "$OUT" --assets "$STAGE" "${verify_args[@]}"
echo "==> $OUT"

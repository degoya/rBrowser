#!/usr/bin/env bash
#
# rb_stage_node_macos (scripts/lib/package-stage.sh, RB-097-01) without the network or a Mac: a
# fake `curl` serves stand-in nodejs.org tarballs from a scratch folder. The runtime is taken only
# for .nvmrc's version, from the tarball of the bundle's architecture (darwin-arm64 for aarch64,
# darwin-x64 for x86_64), held to that architecture's pin; only bin/node (executable) and LICENSE
# are staged, into <Resources>/node, where rb-sidecar looks for them.
#
#   scripts/tests/macos-node.sh
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
TREE="$SCRATCH/tree"
RESOURCES="$SCRATCH/rBrowser.app/Contents/Resources"
mkdir -p "$SERVER" "$CACHE" "$BIN" "$TREE"
echo "v9.9.9" > "$TREE/.nvmrc"
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

# A stand-in for the official tarball $1 (top folder $2): bin/node, LICENSE and what is not staged.
# With $3 = no-node, the tarball lacks bin/node.
tarball() {
    python3 -I - "$1" "$2" "${3:-}" << 'PY'
import io
import sys
import tarfile

target, top, mode = sys.argv[1], sys.argv[2], sys.argv[3]
members = {"LICENSE": b"Node.js is licensed for use as follows: MIT\n",
           "lib/node_modules/npm/package.json": b"{}\n",
           "include/node/node.h": b"/* node */\n"}
if mode != "no-node":
    members["bin/node"] = f"node for {top}\n".encode()
with tarfile.open(target, "w:gz") as archive:
    for name, data in members.items():
        info = tarfile.TarInfo(f"{top}/{name}")
        info.size = len(data)
        info.mode = 0o755 if name == "bin/node" else 0o644
        archive.addfile(info, io.BytesIO(data))
PY
}
sha() { sha256sum "$1" | cut -d ' ' -f 1; }

tarball "$SERVER/node-v9.9.9-darwin-arm64.tar.gz" node-v9.9.9-darwin-arm64
tarball "$SERVER/node-v9.9.9-darwin-x64.tar.gz" node-v9.9.9-darwin-x64
ARM_PIN="$(sha "$SERVER/node-v9.9.9-darwin-arm64.tar.gz")"
X64_PIN="$(sha "$SERVER/node-v9.9.9-darwin-x64.tar.gz")"

# Stages for architecture $1 with the pinned version $2 and the pins $3 (arm64) and $4 (x64).
stage() {
    rm -rf "$RESOURCES" "$SERVER/requests"
    run_status env PATH="$BIN:$PATH" RB_FAKE_SERVER="$SERVER" RB_ROOT="$ROOT" bash -c \
        'set -euo pipefail; source "$RB_ROOT/scripts/lib/package-stage.sh"
         RB_NODE_DARWIN_VERSION="$2"; RB_NODE_DARWIN_ARM64_SHA256="$3"; RB_NODE_DARWIN_X64_SHA256="$4"
         rb_stage_node_macos "$5" "$6" "$1" "$7"' \
        _ "$1" "$2" "$3" "$4" "$TREE" "$CACHE" "$RESOURCES"
}

# --- Apple Silicon: the darwin-arm64 tarball, bin/node and LICENSE only.
stage aarch64 9.9.9 "$ARM_PIN" "$X64_PIN"
expect_status "the pinned runtime for Apple Silicon is staged" 0
expect "from the darwin-arm64 tarball" "node for node-v9.9.9-darwin-arm64" "$(cat "$RESOURCES/node/bin/node")"
expect_true "executable" '[[ -x "$RESOURCES/node/bin/node" ]]'
expect_true "with its LICENSE" 'grep -q "MIT" "$RESOURCES/node/LICENSE"'
expect_true "and nothing else of the tarball" \
    '[[ "$(cd "$RESOURCES/node" && find . -type f | LC_ALL=C sort | tr "\n" " ")" == "./LICENSE ./bin/node " ]]'
expect_true "downloaded from nodejs.org" \
    'grep -qxF "https://nodejs.org/dist/v9.9.9/node-v9.9.9-darwin-arm64.tar.gz" "$SERVER/requests"'
expect_output "the pin is named" "$ARM_PIN"

# --- Intel: the darwin-x64 tarball, held to its own pin.
stage x86_64 9.9.9 "$ARM_PIN" "$X64_PIN"
expect_status "the pinned runtime for Intel is staged" 0
expect "from the darwin-x64 tarball" "node for node-v9.9.9-darwin-x64" "$(cat "$RESOURCES/node/bin/node")"

stage x86_64 9.9.9 "$ARM_PIN" "$ARM_PIN"
expect_status "the Intel tarball held to another pin stops the run" 1
expect_output "naming what came" "$X64_PIN"
expect_true "and staging nothing" '[[ ! -e "$RESOURCES/node" ]]'

# --- a pin for another version than .nvmrc's, an unknown architecture, a placeholder.
stage aarch64 9.9.8 "$ARM_PIN" "$X64_PIN"
expect_status "a pin for another Node version stops the run" 1
expect_output "naming .nvmrc's version" ".nvmrc says 9.9.9"
expect_output "and where the pins come from" "https://nodejs.org/dist/v9.9.9/SHASUMS256.txt"
expect_true "before any download" '[[ ! -e "$SERVER/requests" ]]'

stage riscv64 9.9.9 "$ARM_PIN" "$X64_PIN"
expect_status "an unknown architecture stops the run" 1
expect_output "and is named" "riscv64"

stage aarch64 9.9.9 "(placeholder)" "$X64_PIN"
expect_status "a placeholder pin stops the run" 1
expect_output "and is named" "RB_NODE_DARWIN_ARM64_SHA256"
expect_true "before any download" '[[ ! -e "$SERVER/requests" ]]'

# --- a tarball without bin/node (cached under the name the pin holds).
rm -f "$CACHE"/*.tar.gz
tarball "$SERVER/node-v9.9.9-darwin-arm64.tar.gz" node-v9.9.9-darwin-arm64 no-node
stage aarch64 9.9.9 "$(sha "$SERVER/node-v9.9.9-darwin-arm64.tar.gz")" "$X64_PIN"
expect_status "a tarball without bin/node stops the run" 1
expect_output "and names the member" "node-v9.9.9-darwin-arm64/bin/node"

finish_tests macos-node

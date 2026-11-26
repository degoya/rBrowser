#!/usr/bin/env bash
#
# The MSI on the public CI's Windows runner (RB-099-27), without Windows and without WiX:
# - `scripts/ci-tools.sh wix` installs WiX at RB_WIX_VERSION with `dotnet tool install` and adds
#   the UI extension at the same version (a fake dotnet and a fake wix log their arguments), puts
#   the .NET tools folder on GITHUB_PATH when it is set, skips the install when the pin is already
#   there and fails when another WiX answers;
# - scripts/tool-versions.sh lists the WiX pin and --check refuses a workflow, a document or a
#   script naming the UI extension or `dotnet tool install ... wix` at another version, and an
#   AGENTS.md without its row;
# - scripts/package-msi.sh takes the package ZIP itself, unpacked to a folder of its own, and
#   refuses a ZIP that is not one package folder;
# - ci.yml builds the MSI from the cross-built ZIP on windows-2025, installs it, starts it and
#   removes it again, and uploads it; release.yml builds it after the signing step, from the
#   (signed) ZIP, so the MSI carries what the ZIP carries.
#
# Pure bash and python3, a second.
#
#   scripts/tests/msi-ci.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
# shellcheck source=../lib/tool-versions.sh
source "$ROOT/scripts/lib/tool-versions.sh"

expect_true "WiX is pinned to a 5.x release" '[[ "${RB_WIX_VERSION:-}" =~ ^5\.[0-9]+\.[0-9]+$ ]]'

# --- scripts/ci-tools.sh wix, with a fake dotnet that installs a fake wix ---
FAKE="$SCRATCH/fake-bin"
HOME_DIR="$SCRATCH/home"
LOG="$SCRATCH/calls.log"
mkdir -p "$FAKE" "$HOME_DIR"
cat > "$FAKE/dotnet" << 'SH'
#!/usr/bin/env bash
echo "dotnet $*" >> "$RB_FAKE_LOG"
tools="$HOME/.dotnet/tools"
mkdir -p "$tools"
version="${RB_FAKE_WIX_ANSWERS:-}"
if [[ -z "$version" ]]; then
    while [[ $# -gt 0 ]]; do
        if [[ "$1" == --version ]]; then version="$2"; fi
        shift
    done
fi
printf '%s\n' '#!/usr/bin/env bash' "if [[ \"\$1\" == --version ]]; then echo '$version+0123abcd'; exit 0; fi" \
    'echo "wix $*" >> "$RB_FAKE_LOG"' > "$tools/wix"
chmod +x "$tools/wix"
SH
chmod +x "$FAKE/dotnet"
wix_tools() {
    run_status env -u USERPROFILE HOME="$HOME_DIR" PATH="$FAKE:/usr/bin:/bin" RB_FAKE_LOG="$LOG" \
        "$@" "$ROOT/scripts/ci-tools.sh" wix
}

: > "$SCRATCH/github-path"
wix_tools env GITHUB_PATH="$SCRATCH/github-path"
expect_status "ci-tools.sh wix installs WiX" 0
expect_true "with dotnet tool install at the pin" \
    'grep -qxF "dotnet tool install --global wix --version $RB_WIX_VERSION" "$LOG"'
expect_true "and the UI extension at the same version" \
    'grep -qxF "wix extension add --global WixToolset.UI.wixext/$RB_WIX_VERSION" "$LOG"'
expect_true "and no other extension (rbrowser.wxs uses only the UI one)" \
    '[[ "$(grep -c "^wix extension add" "$LOG")" -eq 1 ]]'
expect_true "the .NET tools folder goes on GITHUB_PATH" \
    'grep -qxF "$HOME_DIR/.dotnet/tools" "$SCRATCH/github-path"'
expect_output "naming the version" "wix $RB_WIX_VERSION"

: > "$LOG"
wix_tools env -u GITHUB_PATH
expect_status "a second run without GITHUB_PATH (a workstation) passes" 0
expect_true "and installs nothing again" '! grep -q "^dotnet tool install" "$LOG"'
expect_true "but adds the extension" 'grep -qxF "wix extension add --global WixToolset.UI.wixext/$RB_WIX_VERSION" "$LOG"'

rm -rf "$HOME_DIR/.dotnet"
wix_tools env -u GITHUB_PATH RB_FAKE_WIX_ANSWERS=6.0.2
expect_status "another WiX answering fails" 1
expect_output "naming both versions" "6.0.2"

run_status env PATH="$FAKE:/usr/bin:/bin" "$ROOT/scripts/ci-tools.sh" wix extra
expect_status "wix takes no arguments" 2

# --- scripts/tool-versions.sh knows the pin and holds its copies ---
run_status "$ROOT/scripts/tool-versions.sh"
expect_status "the listing runs" 0
expect_true "and names WiX at its pin" 'grep -qE "^wix +$RB_WIX_VERSION$" <<< "$output"'

TREE="$SCRATCH/tree"
mkdir -p "$TREE"
(cd "$ROOT" && git ls-files -z | xargs -0 cp --parents -t "$TREE")
# The working copies of the files this job changes, tracked or not yet.
# AGENTS.md is not in the public export (scripts/public-exclude.txt).
[[ -f "$ROOT/AGENTS.md" ]] && cp "$ROOT/AGENTS.md" "$TREE/AGENTS.md"
cp "$ROOT/scripts/lib/tool-versions.sh" "$TREE/scripts/lib/tool-versions.sh"
run_status "$ROOT/scripts/tool-versions.sh" --check --root "$TREE"
expect_status "the tree agrees with the WiX pin" 0

mkdir -p "$TREE/.github/workflows"
printf '%s\n' 'jobs:' '  a:' '    steps:' '      - run: wix extension add --global WixToolset.UI.wixext/0.0.1' \
    > "$TREE/.github/workflows/stray.yml"
run_status "$ROOT/scripts/tool-versions.sh" --check --root "$TREE"
expect_status "a workflow adding the extension at another version fails" 1
expect_output "naming WiX" "wix 0.0.1"
rm "$TREE/.github/workflows/stray.yml"

mkdir -p "$TREE/docs"
printf '%s\n' 'Install: `dotnet tool install --global wix --version 4.0.6`.' > "$TREE/docs/stray-wix.md"
run_status "$ROOT/scripts/tool-versions.sh" --check --root "$TREE"
expect_status "a document installing another WiX fails" 1
expect_output "naming WiX in the document" "wix 4.0.6"
rm "$TREE/docs/stray-wix.md"

if [[ -f "$TREE/AGENTS.md" ]]; then
    sed -i '/^| wix |/d' "$TREE/AGENTS.md"
    run_status "$ROOT/scripts/tool-versions.sh" --check --root "$TREE"
    expect_status "AGENTS.md without the WiX row fails" 1
else
    echo "skip the AGENTS.md row: no AGENTS.md in this tree (the public export)"
fi

# --- scripts/package-msi.sh takes the package ZIP ---
NAME="rbrowser-1.2.3-windows-x64"
STAGE="$SCRATCH/zipped/$NAME"
mkdir -p "$STAGE/locales"
printf 'exe\n' > "$STAGE/rbrowser.exe"
printf '%s\n' "rBrowser 1.2.3" "commit 1a2b3c4d" > "$STAGE/VERSION.txt"
printf 'licence\n' > "$STAGE/LICENSE"
printf 'pak\n' > "$STAGE/locales/en-US.pak"
zip_of() {
    python3 -I - "$1" "$2" << 'PY'
import os
import sys
import zipfile

base, out = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(out, "w") as archive:
    for folder, _, names in os.walk(base):
        for name in names:
            path = os.path.join(folder, name)
            archive.write(path, os.path.relpath(path, os.path.dirname(base)))
PY
}
zip_of "$STAGE" "$SCRATCH/$NAME.zip"
run_status "$ROOT/scripts/package-msi.sh" --sources-only "$SCRATCH/$NAME.zip" "$SCRATCH/from-zip"
expect_status "the sources of a package ZIP are written" 0
expect_true "naming every file of the package" \
    '[[ "$(grep -c "<File " "$SCRATCH/from-zip/files.wxs")" -eq 4 ]]'
expect_true "from the package folder inside the ZIP" \
    'grep -qE "^Stage=.*/$NAME$" "$SCRATCH/from-zip/wix-arguments.txt"'
expect_true "the MSI version is the package's" 'grep -qx "Version=1.2.3" "$SCRATCH/from-zip/wix-arguments.txt"'

python3 -I - "$SCRATCH/two-folders.zip" << 'PY'
import sys
import zipfile

with zipfile.ZipFile(sys.argv[1], "w") as archive:
    archive.writestr("a/rbrowser.exe", "exe")
    archive.writestr("b/rbrowser.exe", "exe")
PY
run_status "$ROOT/scripts/package-msi.sh" --sources-only "$SCRATCH/two-folders.zip" "$SCRATCH/none"
expect_status "a ZIP that is not one package folder is refused" 1
expect_output "and named so" "one top folder"
printf 'not a zip\n' > "$SCRATCH/broken.zip"
run_status "$ROOT/scripts/package-msi.sh" --sources-only "$SCRATCH/broken.zip" "$SCRATCH/none"
expect_status "a broken ZIP is refused" 1

# --- the workflows ---
run_status python3 -I - "$ROOT/.github/workflows/ci.yml" "$ROOT/.github/workflows/release.yml" << 'PY'
import re
import sys


def job(path, name):
    """The lines of job `name` in workflow `path`."""
    lines = open(path, encoding="utf-8").read().splitlines()
    start = lines.index(f"  {name}:")
    body = []
    for line in lines[start + 1:]:
        if re.match(r"^  [A-Za-z0-9_-]+:$", line) or re.match(r"^\S", line):
            break
        body.append(line)
    return body


def first(body, needle):
    """The index of the first line holding `needle`, or -1."""
    return next((i for i, line in enumerate(body) if needle in line), -1)


errors = []
ci = job(sys.argv[1], "windows-msi")
text = "\n".join(ci)
for needle in ("runs-on: windows-2025", "needs: windows-cross", "name: windows-x64-release-test",
               "scripts/ci-tools.sh wix", "scripts/package-msi.sh", "msiexec", "/qn",
               "rbrowser.com", "--version", "/x", "Test-Path", "*.msi", "if-no-files-found: error"):
    if needle not in text:
        errors.append(f"ci.yml windows-msi: no '{needle}'")
order = [first(ci, n) for n in ("scripts/ci-tools.sh wix", "scripts/package-msi.sh", "msiexec")]
if -1 not in order and order != sorted(order):
    errors.append(f"ci.yml windows-msi: WiX, the MSI and its smoke out of order {order}")
release = job(sys.argv[2], "windows")
steps = {n: first(release, n) for n in ("scripts/ci-tools.sh wix", "name: Sign the package",
                                        "scripts/package-msi.sh", "artifacts/windows/*.msi")}
if -1 in steps.values():
    errors.append(f"release.yml windows: a step is missing {steps}")
elif not (steps["scripts/ci-tools.sh wix"] < steps["name: Sign the package"]
          < steps["scripts/package-msi.sh"] < steps["artifacts/windows/*.msi"]):
    errors.append(f"release.yml windows: WiX before signing, the MSI after it, then the upload {steps}")
print("\n".join(errors))
sys.exit(1 if errors else 0)
PY
expect_status "ci.yml builds, smokes and uploads the MSI; release.yml builds it after signing" 0

finish_tests "msi-ci"

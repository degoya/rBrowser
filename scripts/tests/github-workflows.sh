#!/usr/bin/env bash
#
# The rules of the GitHub workflows (RB-090-04), checked by scripts/lib/check_workflows.py: every
# workflow has `permissions: {}` and a `concurrency` group at the top, every job names its own
# permissions, `write` only where the allowlist says, named runner images rather than `-latest`;
# no step that reads a secret comes before a build step of its job (a build runs the code of every
# dependency, which must not find a key or certificate on the runner); dependabot.yml names every
# npm directory that has a package.json, besides cargo and github-actions. First against fixtures,
# each breaking one rule, then against this checkout.
#
#   scripts/tests/github-workflows.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CHECK=(python3 -I "$ROOT/scripts/lib/check_workflows.py")
# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# A fixture tree that keeps every rule; each case below breaks one in a copy.
good="$SCRATCH/good"
mkdir -p "$good/.github/workflows" "$good/ui" "$good/plugins/demo"
echo '{}' > "$good/ui/package.json"
echo '{}' > "$good/plugins/demo/package.json"
cat > "$good/.github/workflows/ci.yml" <<'EOF'
name: CI
on:
  push:
permissions: {}
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
jobs:
  checks:
    runs-on: ubuntu-24.04
    permissions:
      contents: read
    steps:
      - run: scripts/check.sh --preflight
  windows:
    runs-on: windows-2025
    permissions:
      contents: read
    steps:
      - run: scripts/check.sh --rust
EOF
cat > "$good/.github/workflows/release.yml" <<'EOF'
name: Release
on:
  push:
    tags: ["v*"]
permissions: {}
concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: false
jobs:
  windows:
    runs-on: windows-2025
    permissions:
      contents: read
    steps:
      - name: Build
        run: scripts/package-windows.sh --plugins artifacts/plugins
      - name: Sign
        env:
          RB_SIGN_PFX_BASE64: ${{ secrets.RB_SIGN_PFX_BASE64 }}
        run: |
          base64 -d <<< "${RB_SIGN_PFX_BASE64}" > "${RUNNER_TEMP}/codesign.pfx"
  publish:
    runs-on: ubuntu-24.04
    permissions:
      contents: write
      id-token: write
    steps:
      - run: scripts/release-assets.sh sums release-assets
EOF
cat > "$good/.github/dependabot.yml" <<'EOF'
version: 2
updates:
  - package-ecosystem: github-actions
    directory: "/"
  - package-ecosystem: cargo
    directory: "/"
  - package-ecosystem: npm
    directories:
      - "/ui"
      - "/agent"
      - "/plugins/*"
EOF

run_status "${CHECK[@]}" --root "$good"
expect_status "a tree that keeps every rule passes" 0
expect_output "the workflows are counted" "2 workflow(s), 4 job(s)"

# Copies tree $good to $SCRATCH/$1 and prints its path.
case_tree() {
    rm -rf "${SCRATCH:?}/$1"
    cp -r "$good" "$SCRATCH/$1"
    echo "$SCRATCH/$1"
}

tree="$(case_tree read-default)"
sed -i 's/^permissions: {}$/permissions:\n  contents: read/' "$tree/.github/workflows/ci.yml"
run_status "${CHECK[@]}" --root "$tree"
expect_status "top-level permissions other than {} fail" 1
expect_output "the top-level rule is named" "ci.yml: no top-level 'permissions: {}'"

tree="$(case_tree no-job-permissions)"
python3 -I - "$tree/.github/workflows/ci.yml" <<'EOF'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("    runs-on: windows-2025\n    permissions:\n      contents: read\n", "    runs-on: windows-2025\n")
open(path, "w", encoding="utf-8").write(text)
EOF
run_status "${CHECK[@]}" --root "$tree"
expect_status "a job without its own permissions fails" 1
expect_output "the job is named" "ci.yml: job 'windows' names no permissions"

tree="$(case_tree stray-write)"
sed -i '0,/      contents: read/s//      contents: write/' "$tree/.github/workflows/ci.yml"
run_status "${CHECK[@]}" --root "$tree"
expect_status "a write permission outside the allowlist fails" 1
expect_output "the write is named" "ci.yml: job 'checks' asks for contents: write"

tree="$(case_tree write-all)"
sed -i 's/^    permissions:$/    permissions: write-all/; /^      contents: read$/d' "$tree/.github/workflows/ci.yml"
run_status "${CHECK[@]}" --root "$tree"
expect_status "write-all fails" 1
expect_output "write-all is named" "ci.yml: job 'checks' asks for write-all"

tree="$(case_tree latest)"
sed -i 's/runs-on: ubuntu-24.04/runs-on: ubuntu-latest/' "$tree/.github/workflows/ci.yml"
run_status "${CHECK[@]}" --root "$tree"
expect_status "a -latest runner image fails" 1
expect_output "the image is named" "ci.yml: job 'checks' runs on ubuntu-latest"

tree="$(case_tree no-concurrency)"
sed -i '/^concurrency:$/,/^  cancel-in-progress/d' "$tree/.github/workflows/ci.yml"
run_status "${CHECK[@]}" --root "$tree"
expect_status "a workflow without a concurrency group fails" 1
expect_output "the concurrency rule is named" "ci.yml: no top-level 'concurrency'"

tree="$(case_tree secret-before-build)"
python3 -I - "$tree/.github/workflows/release.yml" <<'EOF'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
build = "      - name: Build\n        run: scripts/package-windows.sh --plugins artifacts/plugins\n"
text = text.replace(build, "")
text = text.replace("  publish:\n", build.replace("- name: Build", "- name: Build again") + "  publish:\n")
open(path, "w", encoding="utf-8").write(text)
EOF
run_status "${CHECK[@]}" --root "$tree"
expect_status "a step that reads a secret before a build step fails" 1
expect_output "the job and both steps are named" \
    "release.yml: job 'windows' reads a secret in step 1 (Sign) before the build in step 2 (Build again)"

tree="$(case_tree npm-dir)"
mkdir -p "$tree/sidecar"
echo '{}' > "$tree/sidecar/package.json"
run_status "${CHECK[@]}" --root "$tree"
expect_status "a package.json dependabot does not name fails" 1
expect_output "the directory is named" "dependabot.yml: npm directory /sidecar is not listed"

tree="$(case_tree no-cargo)"
sed -i '/package-ecosystem: cargo/,+1d' "$tree/.github/dependabot.yml"
run_status "${CHECK[@]}" --root "$tree"
expect_status "a dependabot.yml without cargo fails" 1
expect_output "the ecosystem is named" "dependabot.yml: no 'cargo' entry"

run_status "${CHECK[@]}" --root "$ROOT"
expect_status "this checkout's workflows and dependabot.yml keep the rules" 0

finish_tests github-workflows

#!/usr/bin/env bash
#
# scripts/lib/licenses.py (the worker of scripts/licenses.sh, RB-010-18, RB-099-12) against a
# fixture: two Rust roots (cargo metadata + cargo tree, the crates' sources beside them), the
# shell's npm lock file with a production, a dev and an optional package, and the sidecar's with
# Lighthouse and the error reporting the package leaves out (RB_SIDECAR_EXCLUDED). It lists a
# crate a tree names with its licence and every other locked crate as not shipped, the production
# npm package with its licence and the dev, optional and left-out ones as not shipped, and writes
# THIRD-PARTY-LICENSES.txt with the full texts and copyright lines of what ships - an identical
# text once, the standard text where a package ships none; --check then passes and finds either
# file stale; a package without a licence stops it until an override names one; a package with
# no licence file and no standard text stops it; an override for no package stops it.
#
# Python only, compiles nothing: check.sh --preflight runs it.
#
#   scripts/tests/licenses.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
WORKER="$ROOT/scripts/lib/licenses.py"
LIST=ui/src/about/licenses/third-party.json
TEXTS=packaging/THIRD-PARTY-LICENSES.txt

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

repo="$SCRATCH/repo"
inputs="$SCRATCH/inputs"
src="$SCRATCH/registry"
mkdir -p "$repo/ui/src/about/licenses/texts" "$repo/scripts/lib" "$inputs" \
    "$src/serde-1.0.0" "$src/itoa-1.0.0" "$src/bitflags-2.0.0" "$src/zstd-0.13.0" "$src/syn-2.0.0" \
    "$src/blank-1.0.0"
cat > "$inputs/app.metadata.json" << EOF
{ "packages": [
  { "name": "rbrowser", "version": "0.1.0", "source": null, "license": "GPL-3.0-or-later",
    "manifest_path": "/x/crates/rbrowser/Cargo.toml", "authors": [] },
  { "name": "serde", "version": "1.0.0", "source": "registry", "license": "MIT OR Apache-2.0",
    "manifest_path": "$src/serde-1.0.0/Cargo.toml", "authors": ["Serde Authors"] },
  { "name": "itoa", "version": "1.0.0", "source": "registry", "license": "MIT OR Apache-2.0",
    "manifest_path": "$src/itoa-1.0.0/Cargo.toml", "authors": [] },
  { "name": "zstd", "version": "0.13.0", "source": "registry", "license": "MIT",
    "manifest_path": "$src/zstd-0.13.0/Cargo.toml", "license_file": null, "authors": [] },
  { "name": "blank", "version": "1.0.0", "source": "registry", "license": "MIT",
    "manifest_path": "$src/blank-1.0.0/Cargo.toml", "authors": ["Blank Author"] },
  { "name": "syn", "version": "2.0.0", "source": "registry", "license": "MIT OR Apache-2.0",
    "manifest_path": "$src/syn-2.0.0/Cargo.toml", "authors": [] }
] }
EOF
printf '%s\n' 'rbrowser v0.1.0 (/x/crates/rbrowser)' 'serde v1.0.0' 'serde v1.0.0 (*)' 'itoa v1.0.0' \
    'zstd v0.13.0' 'blank v1.0.0' > "$inputs/app.tree.txt"
cat > "$inputs/plugin-a.metadata.json" << EOF
{ "packages": [ { "name": "bitflags", "version": "2.0.0", "source": "registry", "license": "",
  "manifest_path": "$src/bitflags-2.0.0/Cargo.toml", "authors": ["Bit Flagger <b@example.org>"] } ] }
EOF
echo 'bitflags v2.0.0' > "$inputs/plugin-a.tree.txt"
printf 'MIT License\n\nCopyright (c) 2014 The Serde Authors\n\nPermission is hereby granted (serde).\n' \
    > "$src/serde-1.0.0/LICENSE-MIT"
printf 'Apache License\nVersion 2.0, January 2004\n\nTERMS AND CONDITIONS (the one Apache text)\n' \
    > "$src/serde-1.0.0/LICENSE-APACHE"
# The same text with other line ends and indentation: one section for both.
printf '   Apache License\r\n   Version 2.0, January 2004\r\n\r\n   TERMS AND CONDITIONS (the one Apache text)\r\n' \
    > "$src/itoa-1.0.0/LICENSE-APACHE"
# A licence file with nothing in it counts as none.
printf '  \n\n \n' > "$src/blank-1.0.0/LICENSE"
printf 'Copyright (c) 2016 Alexandre Bury\n\nPermission is hereby granted (zstd).\n' > "$src/zstd-0.13.0/LICENSE"
printf 'The MIT standard text.\n\n{copyright}\n\nPermission is hereby granted (standard).\n' \
    > "$repo/ui/src/about/licenses/texts/MIT.txt"
printf 'The Apache-2.0 standard text.\n' > "$repo/ui/src/about/licenses/texts/Apache-2.0.txt"
echo 'RB_SIDECAR_EXCLUDED=(@sentry/ sentry)' > "$repo/scripts/lib/package-stage.sh"
cat > "$repo/ui/package-lock.json" << 'EOF'
{ "lockfileVersion": 3, "packages": {
  "": { "name": "shell" },
  "node_modules/vue": { "version": "3.5.0", "license": "MIT" },
  "node_modules/vite": { "version": "8.0.0", "license": "MIT", "dev": true },
  "node_modules/fsevents": { "version": "2.3.3", "license": "MIT", "optional": true }
} }
EOF
mkdir -p "$repo/ui/node_modules/vue" "$repo/sidecar/node_modules/lighthouse" "$repo/sidecar/node_modules/shared"
printf 'The MIT License (MIT)\n\nCopyright (c) 2018-present, Yuxi (Evan) You\n\nPermission (vue).\n' \
    > "$repo/ui/node_modules/vue/LICENSE"
printf 'Apache License\nVersion 2.0, January 2004\n\nTERMS AND CONDITIONS (the one Apache text)\n' \
    > "$repo/sidecar/node_modules/lighthouse/LICENSE"
printf 'ISC License\n\nCopyright (c) Shared Author\n' > "$repo/sidecar/node_modules/shared/LICENSE.md"
echo '{ "name": "vue", "version": "3.5.0" }' > "$repo/ui/node_modules/vue/package.json"
echo '{ "name": "lighthouse", "version": "13.0.0" }' > "$repo/sidecar/node_modules/lighthouse/package.json"
echo '{ "name": "shared", "version": "1.0.0" }' > "$repo/sidecar/node_modules/shared/package.json"
cat > "$repo/sidecar/package-lock.json" << 'EOF'
{ "lockfileVersion": 3, "packages": {
  "": { "name": "sidecar", "dependencies": { "lighthouse": "13.0.0" } },
  "node_modules/lighthouse": { "version": "13.0.0", "license": "Apache-2.0",
    "dependencies": { "@sentry/node": "11.0.0", "sentry": "0.49.0", "shared": "1.0.0" } },
  "node_modules/@sentry/node": { "version": "11.0.0", "license": "MIT", "dependencies": { "only-sentry": "1.0.0" } },
  "node_modules/sentry": { "version": "0.49.0", "license": "FSL-1.1-Apache-2.0" },
  "node_modules/only-sentry": { "version": "1.0.0", "license": "MIT" },
  "node_modules/shared": { "version": "1.0.0", "license": "ISC" }
} }
EOF

run_status python3 -B "$WORKER" "$repo" "$inputs"
expect_status "a crate without a licence and without an override stops the run" 2
expect_output "and names it" "bitflags@2.0.0: declares no licence"

cat > "$repo/ui/src/about/licenses/overrides.json" << 'EOF'
{ "rust": { "bitflags@2.0.0": { "license": "MIT OR Apache-2.0", "source": "its LICENSE files" } }, "npm": {} }
EOF
run_status python3 -B "$WORKER" "$repo" "$inputs"
expect_status "with the override the list is written" 0
# shellcheck disable=SC2034  # read by the conditions expect_true evaluates
list="$(cat "$repo/$LIST")"
expect_true "a crate the tree names is listed with its licence" \
    '[[ "$list" == *"{\"name\": \"serde\", \"version\": \"1.0.0\", \"license\": \"MIT OR Apache-2.0\"}"* ]]'
expect_true "the override gives the licence" '[[ "$list" == *"\"bitflags\", \"version\": \"2.0.0\", \"license\": \"MIT OR Apache-2.0\""* ]]'
expect_true "a locked crate no tree names is not shipped" '[[ "$list" == *"\"syn@2.0.0\""* ]]'
expect_true "the workspace member is not listed" '[[ "$list" != *"rbrowser"* ]]'
expect_true "the production npm package is listed" '[[ "$list" == *"{\"name\": \"vue\", \"version\": \"3.5.0\", \"license\": \"MIT\"}"* ]]'
expect_true "dev and optional npm packages are not shipped" \
    '[[ "$list" == *"\"vite@8.0.0\""* && "$list" == *"\"fsevents@2.3.3\""* ]]'
expect_true "the sidecar's Lighthouse ships, with what it needs beside the error reporting" \
    '[[ "$list" == *"{\"name\": \"lighthouse\", \"version\": \"13.0.0\""* && "$list" == *"{\"name\": \"shared\""* ]]'
expect_true "the packages RB_SIDECAR_EXCLUDED leaves out, and what only they need, are not shipped" \
    '[[ "$list" == *"\"@sentry/node@11.0.0\""* && "$list" == *"\"sentry@0.49.0\""* && "$list" == *"\"only-sentry@1.0.0\""* && "$list" != *"{\"name\": \"sentry\""* && "$list" != *"{\"name\": \"@sentry/node\""* ]]'

# shellcheck disable=SC2034  # read by the conditions expect_true evaluates
texts="$(cat "$repo/$TEXTS")"
expect_true "THIRD-PARTY-LICENSES.txt indexes every shipped package with its licence" \
    '[[ "$texts" == *"  serde 1.0.0 (MIT OR Apache-2.0): ["* && "$texts" == *"  vue 3.5.0 (MIT): ["* && "$texts" == *"  zstd 0.13.0 (MIT): ["* ]]'
expect_true "and leaves out what does not ship" \
    '[[ "$texts" != *"syn 2.0.0"* && "$texts" != *"vite 8.0.0"* && "$texts" != *"sentry"* && "$texts" != *"rbrowser 0.1.0"* ]]'
expect_true "a crate's licence files, with the copyright line" \
    '[[ "$texts" == *"Copyright (c) 2014 The Serde Authors"* && "$texts" == *"Copyright (c) 2016 Alexandre Bury"* ]]'
expect_true "an npm package's licence file, with the copyright line" \
    '[[ "$texts" == *"Copyright (c) 2018-present, Yuxi (Evan) You"* && "$texts" == *"Copyright (c) Shared Author"* ]]'
expect "a text several packages share is written once" 1 \
    "$(grep -c "TERMS AND CONDITIONS (the one Apache text)" "$repo/$TEXTS")"
expect_true "and names them all" \
    '[[ "$texts" == *"Used by: itoa 1.0.0, serde 1.0.0, lighthouse 13.0.0"* ]]'
expect_true "a package whose licence file is empty gets the standard text too, said so" \
    '[[ "$texts" == *"  blank 1.0.0 (MIT): ["*"] - ships no licence file; the standard text"* && "$texts" == *"Copyright (c) Blank Author"* ]]'
expect_true "a package without a licence file gets the standard text, said so, with its authors" \
    '[[ "$texts" == *"  bitflags 2.0.0 (MIT OR Apache-2.0): ["*"] - ships no licence file; the standard text"* && "$texts" == *"Copyright (c) Bit Flagger <b@example.org>"* && "$texts" == *"The Apache-2.0 standard text."* ]]'

run_status python3 -B "$WORKER" "$repo" "$inputs" --check
expect_status "--check passes on current files" 0

echo "edited by hand" >> "$repo/$TEXTS"
run_status python3 -B "$WORKER" "$repo" "$inputs" --check
expect_status "--check finds THIRD-PARTY-LICENSES.txt stale" 1
expect_output "and names it" "$TEXTS is stale"
run_status python3 -B "$WORKER" "$repo" "$inputs"
expect_status "a run writes it again" 0

sed -i 's/"3.5.0"/"3.5.1"/' "$repo/ui/package-lock.json"
mv "$repo/ui/node_modules/vue" "$SCRATCH/vue"
run_status python3 -B "$WORKER" "$repo" "$inputs"
expect_status "a shipped npm package that is not installed stops the run" 2
expect_output "and says what to install" "vue@3.5.1: not installed in ui/node_modules"
mv "$SCRATCH/vue" "$repo/ui/node_modules/vue"
run_status python3 -B "$WORKER" "$repo" "$inputs"
expect_status "an installed npm package of another version than the lock's stops the run" 2
expect_output "and says what to install" "vue@3.5.1: ui/node_modules has 3.5.0 - run npm ci in ui/"
sed -i 's/"3.5.0"/"3.5.1"/' "$repo/ui/node_modules/vue/package.json"
run_status python3 -B "$WORKER" "$repo" "$inputs" --check
expect_status "--check finds the list stale after a lock file changed" 1
sed -i 's/"3.5.1"/"3.5.0"/' "$repo/ui/package-lock.json" "$repo/ui/node_modules/vue/package.json"

# The negative case of the preflight: a new runtime dependency without any licence text.
sed -i 's/"zstd", "version": "0.13.0", "source": "registry", "license": "MIT"/"zstd", "version": "0.13.0", "source": "registry", "license": "BSD-3-Clause"/' \
    "$inputs/app.metadata.json"
rm "$src/zstd-0.13.0/LICENSE"
run_status python3 -B "$WORKER" "$repo" "$inputs"
expect_status "a shipped crate with no licence file and no standard text stops the run" 2
expect_output "and names it" "zstd@0.13.0: ships no licence file and BSD-3-Clause has no standard text"

cat > "$repo/ui/src/about/licenses/overrides.json" << 'EOF'
{ "rust": { "bitflags@2.0.0": { "license": "MIT" } }, "npm": { "gone@1.0.0": { "license": "MIT" } } }
EOF
run_status python3 -B "$WORKER" "$repo" "$inputs"
expect_status "an override for no package stops the run" 2
expect_output "and names it" "gone@1.0.0: an override for no locked package"

run_status python3 -B "$WORKER" "$repo" "$SCRATCH/empty"
expect_status "no Rust root is an error" 2

finish_tests licenses

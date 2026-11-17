#!/usr/bin/env bash
#
# scripts/advisories.sh (RB-090-04) against stub `cargo` and `npm` on PATH: it asks cargo-deny for
# the advisories and runs `npm audit --omit=dev` in every npm package with a package-lock.json,
# keeps going after a finding, and fails when any of them failed.
#
#   scripts/tests/advisories.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# A tree with the script and three packages (the plugins' workspace root among them): two with
# a lock file, one without.
tree="$SCRATCH/tree"
mkdir -p "$tree/scripts" "$tree/ui" "$tree/sidecar" "$tree/plugins" "$tree/bin"
cp "$ROOT/scripts/advisories.sh" "$tree/scripts/"
for dir in ui plugins; do echo '{}' > "$tree/$dir/package.json"; echo '{}' > "$tree/$dir/package-lock.json"; done
echo '{}' > "$tree/sidecar/package.json"
calls="$SCRATCH/calls"
cat > "$tree/bin/cargo" <<EOF
#!/usr/bin/env bash
echo "cargo \$*" >> "$calls"
exit "\${STUB_CARGO_EXIT:-0}"
EOF
cat > "$tree/bin/npm" <<EOF
#!/usr/bin/env bash
echo "npm \$* in \$(basename "\$PWD")" >> "$calls"
[[ "\$(basename "\$PWD")" != "\${STUB_NPM_FAIL_IN:-}" ]]
EOF
chmod +x "$tree/bin/cargo" "$tree/bin/npm"

run_status env PATH="$tree/bin:$PATH" bash "$tree/scripts/advisories.sh"
expect_status "no findings pass" 0
expect_output "cargo-deny is asked for the advisories" "cargo deny check advisories"
expect "npm audit runs in each locked package" "2" "$(grep -c '^npm audit --omit=dev in ' "$calls")"
expect "a package without a lock file is skipped" "0" "$(grep -c 'in sidecar$' "$calls" || true)"
expect_output "the skip is named" "sidecar"

: > "$calls"
run_status env PATH="$tree/bin:$PATH" STUB_NPM_FAIL_IN=ui bash "$tree/scripts/advisories.sh"
expect_status "a finding in one package fails" 1
expect "the other package is still audited" "1" "$(grep -c 'in plugins$' "$calls")"
expect_output "the failing package is named" "ui"

: > "$calls"
run_status env PATH="$tree/bin:$PATH" STUB_CARGO_EXIT=1 bash "$tree/scripts/advisories.sh"
expect_status "a Rust advisory fails" 1
expect "npm still runs after it" "2" "$(grep -c '^npm audit' "$calls")"

finish_tests advisories

#!/usr/bin/env bash
#
# scripts/check-settings-read.sh (scripts/lib/settings_unread.py, RB-094-30) against small
# repositories in a temp directory: a core settings key that no code reads fails with the key
# named; a key read in Rust (a field access), in the shell UI (the dotted key as a string) or by its
# camelCase name passes; a read only in a test or in the key tables of the annotations does not
# count; plugin keys are not its business. The repository itself passes too.
#
# Python only, compiles nothing: check.sh --preflight runs it.
#
#   scripts/tests/settings-read.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
CHECK="$ROOT/scripts/check-settings-read.sh"

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"
# shellcheck source=lib/settings-fixture.sh
source "$ROOT/scripts/tests/lib/settings-fixture.sh"

# Writes the code that reads every core key of the fixture into $1.
reading_code() {
    local dir="$1"
    mkdir -p "$dir/crates/demo/src" "$dir/ui/src"
    cat > "$dir/crates/demo/src/lib.rs" << 'RS'
pub fn apply(config: &Config) -> bool {
    config.sync.scroll && config.sync.navigation && !config.start_url.is_empty()
}
pub fn certificates(network: &Network) -> bool {
    network.ignore_certificate_errors
}
RS
    cat > "$dir/ui/src/proxy.ts" << 'TS'
export const PROXY_KEY = 'network.proxy';
TS
}

run_status "$CHECK" "$ROOT"
expect_status "the repository passes" 0

fixture="$(settings_fixture "$SCRATCH/none")"
run_status "$CHECK" "$fixture"
expect_status "keys nothing reads fail" 1
expect_output "an unread key is named" "sync.scroll: no code reads this key"
expect_output "the shell's keys count too" "network.proxy: no code reads this key"

fixture="$(settings_fixture "$SCRATCH/good")"
reading_code "$fixture"
run_status "$CHECK" "$fixture"
expect_status "every key read passes" 0
expect_output "the summary counts the core keys" "5 core keys"

fixture="$(settings_fixture "$SCRATCH/test-only")"
reading_code "$fixture"
sed -i 's/ && config.sync.navigation//' "$fixture/crates/demo/src/lib.rs"
printf '%s\n' 'fn reads() { let _ = config.sync.navigation; }' > "$fixture/crates/demo/src/lib_tests.rs"
mkdir -p "$fixture/crates/rb-config/src/annotate"
printf '%s\n' 'key("sync.navigation", Category::Sync, "navigation"),' \
    > "$fixture/crates/rb-config/src/annotate/user_keys.rs"
run_status "$CHECK" "$fixture"
expect_status "a read in a test or a key table does not count" 1
expect_output "the key is named" "sync.navigation: no code reads this key"

fixture="$(settings_fixture "$SCRATCH/camel")"
reading_code "$fixture"
sed -i 's/ && config.sync.navigation//' "$fixture/crates/demo/src/lib.rs"
printf '%s\n' 'export const on = (sync: { navigation: boolean }) => sync.navigation;' \
    > "$fixture/ui/src/sync.ts"
run_status "$CHECK" "$fixture"
expect_status "a read in the shell UI by its name passes" 0

finish_tests "settings read"

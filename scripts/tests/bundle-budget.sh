#!/usr/bin/env bash
#
# scripts/bundle-budget.sh (scripts/lib/bundle_budget.py) against a fake ui/dist: two pages that
# share a chunk, each with its entry, its preloaded chunks and its stylesheet. Within budget it
# passes and prints every page's size; a page over its JS or CSS budget fails and names the page
# and its largest file; a page without a budget, a budget for a page the build lacks, a missing
# dist and a malformed budget file fail too. Last, the repository's own budget file reads.
#
# Python only, compiles nothing: check.sh --preflight runs it.
#
#   scripts/tests/bundle-budget.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
BUDGET="$ROOT/scripts/bundle-budget.sh"

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

dist="$SCRATCH/dist"
mkdir -p "$dist/assets"
# A file of $2 bytes at dist/assets/$1.
asset() { head -c "$2" /dev/zero | tr '\0' 'x' > "$dist/assets/$1"; }
asset main-A1.js 1000
asset status-bar-B2.js 300
asset bootstrap-C3.js 5000
asset bootstrap-D4.css 2000
asset lazy-E5.js 90000
cat > "$dist/index.html" << 'EOF'
<!doctype html><html><head>
<script type="module" crossorigin src="./assets/main-A1.js"></script>
<link rel="modulepreload" crossorigin href="./assets/bootstrap-C3.js">
<link rel="stylesheet" crossorigin href="./assets/bootstrap-D4.css">
</head><body><div id="app"></div></body></html>
EOF
cat > "$dist/status-bar.html" << 'EOF'
<!doctype html><html><head>
<script type="module" crossorigin src="./assets/status-bar-B2.js"></script>
<link rel="modulepreload" crossorigin href="./assets/bootstrap-C3.js">
<link rel="stylesheet" crossorigin href="./assets/bootstrap-D4.css">
<link rel="icon" href="./assets/logo.svg">
</head><body></body></html>
EOF
budget() {
    printf '{"pages": {"index.html": {"js": %s, "css": %s}, "status-bar.html": {"js": 5300, "css": 2000}}}\n' \
        "$1" "$2" > "$SCRATCH/budget.json"
}

budget 6000 2000
run_status bash "$BUDGET" --dist "$dist" --budget "$SCRATCH/budget.json"
expect_status "within budget passes" 0
expect_output "a page's JS: its entry and its preloaded chunks, not a lazy one" "index.html: JS 6000 of 6000 bytes"
expect_output "a page's CSS" "CSS 2000 of 2000 bytes"
expect_output "the shared chunk counts for every page" "status-bar.html: JS 5300 of 5300 bytes"

budget 5999 2000
run_status bash "$BUDGET" --dist "$dist" --budget "$SCRATCH/budget.json"
expect_status "a page over its JS budget fails" 1
expect_output "the failure names the page and the overrun" "index.html: JS 6000 bytes is over its budget of 5999"
expect_output "the failure names the largest file" "assets/bootstrap-C3.js 5000"

budget 6000 1999
run_status bash "$BUDGET" --dist "$dist" --budget "$SCRATCH/budget.json"
expect_status "a page over its CSS budget fails" 1
expect_output "the CSS failure names the stylesheet" "index.html: CSS 2000 bytes is over its budget of 1999 (assets/bootstrap-D4.css 2000)"

printf '{"pages": {"index.html": {"js": 6000, "css": 2000}}}\n' > "$SCRATCH/budget.json"
run_status bash "$BUDGET" --dist "$dist" --budget "$SCRATCH/budget.json"
expect_status "a page without a budget fails" 1
expect_output "it names the page" "status-bar.html: no budget"

printf '{"pages": {"index.html": {"js": 6000, "css": 2000}, "status-bar.html": {"js": 5300, "css": 2000}, "gone.html": {"js": 1, "css": 1}}}\n' \
    > "$SCRATCH/budget.json"
run_status bash "$BUDGET" --dist "$dist" --budget "$SCRATCH/budget.json"
expect_status "a budget for a page the build lacks fails" 1
expect_output "it names that page" "gone.html: a budget, but the build has no such page"

printf '{"pages": {"index.html": {"js": "big"}}}\n' > "$SCRATCH/budget.json"
run_status bash "$BUDGET" --dist "$dist" --budget "$SCRATCH/budget.json"
expect_status "a malformed budget fails" 1
expect_output "it says what is wrong" "index.html: js and css must be byte counts"

budget 6000 2000
rm "$dist/assets/bootstrap-C3.js"
run_status bash "$BUDGET" --dist "$dist" --budget "$SCRATCH/budget.json"
expect_status "a page naming a missing file fails" 1
expect_output "it names the file" "assets/bootstrap-C3.js is missing"

run_status bash "$BUDGET" --dist "$SCRATCH/none" --budget "$SCRATCH/budget.json"
expect_status "no build fails" 1
expect_output "it says to build first" "no build at"

run_status python3 -c 'import json, sys; pages = json.load(open(sys.argv[1]))["pages"]; assert pages and all(set(v) == {"js", "css"} for v in pages.values())' \
    "$ROOT/ui/bundle-budget.json"
expect_status "the repository's budget file reads" 0

finish_tests "bundle-budget"

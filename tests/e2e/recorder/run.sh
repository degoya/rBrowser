#!/usr/bin/env bash
# K12 (RB-050-02): the recorder's export of tests/e2e/recorder/login.export.json - the files in
# generated/, held to the plugin's output by `cargo test` in plugins/recorder-export - is
# type-checked with `tsc --noEmit` (flow and spec), then the spec runs with `npx playwright test`
# against the fixture site (tests/fixtures/site/recorder.html) in every breakpoint project.
#
#   tests/e2e/recorder/run.sh [--typecheck-only] [-- <playwright test args>]
#
# Needs Node (.nvmrc) and network on the first run (npm ci, the Playwright Chromium build).
# RB_E2E_PORT picks the fixture server's port (default 8765); RB_SECRET_PASSWORD is the fixture's
# password (any value: the page accepts every non-empty one).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
typecheck_only=0
if [[ "${1:-}" == --typecheck-only ]]; then
    typecheck_only=1
    shift
fi
[[ "${1:-}" == -- ]] && shift

cd "$here"
if [[ ! -d node_modules ]]; then
    npm ci
fi
echo "==> tsc --noEmit (flow and spec)"
npx tsc --noEmit -p tsconfig.json
if [[ $typecheck_only -eq 1 ]]; then
    exit 0
fi

npx playwright install chromium
port="${RB_E2E_PORT:-8765}"
python3 -m http.server "$port" --bind 127.0.0.1 --directory "$root/tests/fixtures/site" \
    > /dev/null 2>&1 &
server=$!
trap 'kill "$server" 2> /dev/null || true' EXIT
for _ in $(seq 1 50); do
    if python3 -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:$port/recorder.html')" \
        2> /dev/null; then
        break
    fi
    sleep 0.1
done

echo "==> npx playwright test (every breakpoint project)"
RB_BASE_URL="http://127.0.0.1:$port" \
    RB_SECRET_PASSWORD="${RB_SECRET_PASSWORD:-fixture-only}" \
    npx playwright test -c generated/playwright.config.ts "$@"

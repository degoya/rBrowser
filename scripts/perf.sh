#!/usr/bin/env bash
#
# The performance budget K3 (RB-020-09, docs/perf-budget.md): with 8 views, 2 of them at DPR 3,
# the active view scrolls at 50 fps or more and all rBrowser processes together stay below
# 3 GB. Each run scrolls one view with real wheel input over the CDP proxy and counts its
# frames (scripts/lib/perf-measure.mjs), then samples the private memory of every rbrowser
# process from the Windows process list; scripts/lib/perf_report.py writes the result to
# artifacts/perf/<date>.json, prints the summary and holds it to the budget (the runs no more
# than 10 % apart).
#
# K1 and K11 (RB-090-32): after the runs, scripts/lib/perf-k1.mjs clicks the targets of
# latency.html in the measured view (--clicks, default 100; 0 skips it) and reads when each
# other view logged the replayed click - K1 is the p95 at most 150 ms. --counters names the
# answer of the MCP tool app_perf saved after the measurement: the replays into Firefox and
# WebKit (K11, p95 at most 300 ms), the hub's own replay times and the start-up time.
#
# Before a measurement (the Windows host, docs/perf-budget.md "How to measure"):
#   - the CDP proxy on (Settings > Automation > CDP proxy), its endpoint in the environment:
#     RBROWSER_CDP_ENDPOINT=<file with the answer of the MCP tool cdp_endpoint>, or
#     RBROWSER_CDP_URL and RBROWSER_CDP_TOKEN;
#   - rBrowser running on tests/fixtures/project-perf with the fixture server
#     (--start does both: the server in WSL, rBrowser through scripts/run-windows.sh --release);
#   - no other rBrowser running (the memory is summed by process name).
#
# Usage:
#   scripts/perf.sh [--views 8] [--dpr3 2] [--runs 3] [--seconds 10] [--view 0] [--start]
#                   [--clicks 100] [--counters APP_PERF.json] [--out DIR]
#   scripts/perf.sh --runs 0 --clicks 0 --counters APP_PERF.json   # K11 and start-up alone
#   scripts/perf.sh --compare [RESULT]   # the newest (or named) result against the baseline
#   scripts/perf.sh --dry-run ...        # print the steps, run nothing
#   scripts/perf.sh --fake DIR ...       # runs from DIR/run-<n>.json and DIR/memory-<n>, the
#                                        # click run from DIR/k1.json when it is there
#                                        # (the script test; no CDP, no Windows)
#
# Exit: 0 budget met, 1 budget missed, 2 usage or a run that failed.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="$ROOT/tests/fixtures/project-perf"
SERVER="$ROOT/tests/fixtures/server/post-counter.py"
SERVER_PORT=18766

views=8
dpr3=2
runs=3
seconds=10
view=0
clicks=100
counters=""
start=false
dry_run=false
fake=""
compare=false
compare_file=""
out="$ROOT/artifacts/perf"

usage() {
    sed -n '/^# Usage:/,/^# Exit:/p' "$0" | sed 's/^# \{0,1\}//' >&2
    exit 2
}

number() {
    [[ "$2" =~ ^[0-9]+$ ]] || { echo "perf.sh: $1 takes a number" >&2; exit 2; }
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --views) number "$1" "${2:-}"; views="$2"; shift 2 ;;
        --dpr3) number "$1" "${2:-}"; dpr3="$2"; shift 2 ;;
        --runs) number "$1" "${2:-}"; runs="$2"; shift 2 ;;
        --seconds) number "$1" "${2:-}"; seconds="$2"; shift 2 ;;
        --view) number "$1" "${2:-}"; view="$2"; shift 2 ;;
        --clicks) number "$1" "${2:-}"; clicks="$2"; shift 2 ;;
        --counters) [[ -f "${2:-}" ]] || { echo "perf.sh: --counters takes the saved app_perf answer, a file" >&2; exit 2; }
            counters="$2"; shift 2 ;;
        --out) [[ -n "${2:-}" ]] || usage; out="$2"; shift 2 ;;
        --fake) [[ -d "${2:-}" ]] || { echo "perf.sh: --fake takes a folder" >&2; exit 2; }
            fake="$2"; shift 2 ;;
        --start) start=true; shift ;;
        --dry-run) dry_run=true; shift ;;
        --compare)
            compare=true
            if [[ -n "${2:-}" && "${2:0:2}" != "--" ]]; then compare_file="$2"; shift; fi
            shift ;;
        -h|--help) usage ;;
        *) echo "perf.sh: unknown argument $1" >&2; usage ;;
    esac
done
if [[ "$seconds" -lt 1 ]]; then
    echo "perf.sh: --seconds is at least 1" >&2
    exit 2
fi
# --runs 0: no scroll run, K3 not measured - the click run and the counters alone.
if [[ "$runs" -lt 1 && "$clicks" -lt 1 && -z "$counters" ]]; then
    echo "perf.sh: nothing to measure - --runs 0 needs --clicks or --counters" >&2
    exit 2
fi

if $compare; then
    if [[ -z "$compare_file" && -d "$out" ]]; then
        compare_file="$(find "$out" -maxdepth 1 -name '*.json' | sort | tail -n 1)"
    fi
    if [[ -z "$compare_file" ]]; then
        echo "perf.sh: hint: no measurement under $out yet (scripts/perf.sh on Windows)"
        exit 0
    fi
    exec python3 "$ROOT/scripts/lib/perf_report.py" compare "$compare_file" "$ROOT/docs/perf-budget.md"
fi

# The private memory of every rbrowser process: "<processes> <bytes>". RB_PERF_MEMORY_CMD
# replaces the PowerShell call (tests, another host).
sample_memory() {
    if [[ -n "${RB_PERF_MEMORY_CMD:-}" ]]; then
        bash -c "$RB_PERF_MEMORY_CMD"
        return
    fi
    # shellcheck disable=SC2016  # PowerShell's variables, not the shell's
    powershell.exe -NoProfile -NonInteractive -Command \
        '$p = @(Get-Process -Name rbrowser,rbrowser-nosandbox -ErrorAction SilentlyContinue); "{0} {1}" -f $p.Count, [int64]($p | Measure-Object PrivateMemorySize64 -Sum).Sum' \
        | tr -d '\r'
}

stamp="$(date +%Y-%m-%dT%H-%M-%S)"
work="$(mktemp -d)"
server_pid=""
# shellcheck disable=SC2317,SC2329  # called by the trap (SC2317 before shellcheck 0.11)
cleanup() {
    if [[ -n "$server_pid" ]]; then kill "$server_pid" 2>/dev/null || true; fi
    rm -rf "$work"
}
trap cleanup EXIT

if $dry_run; then
    echo "perf.sh: dry run - $views views ($dpr3 at DPR 3), $runs run(s) of $seconds s, view $view"
    if $start; then
        echo "  start: python3 ${SERVER#"$ROOT/"} $SERVER_PORT"
        echo "  start: scripts/run-windows.sh --release -- --project ${FIXTURE#"$ROOT/"}"
    fi
    for ((n = 1; n <= runs; n++)); do
        echo "  run $n: node scripts/lib/perf-measure.mjs --seconds $seconds --view $view, then the memory sample"
    done
    if [[ "$clicks" -gt 0 ]]; then
        echo "  K1: node scripts/lib/perf-k1.mjs --clicks $clicks --view $view (latency.html in every view)"
    fi
    if [[ -n "$counters" ]]; then
        echo "  K11 and start-up: $counters (the app_perf answer)"
    fi
    echo "  report: ${out#"$ROOT/"}/$stamp.json"
    exit 0
fi

if [[ -z "$fake" ]]; then
    if [[ -z "${RBROWSER_CDP_URL:-}${RBROWSER_CDP_ENDPOINT:-}" ]]; then
        echo "perf.sh: no CDP endpoint - set RBROWSER_CDP_ENDPOINT (the answer of cdp_endpoint) or RBROWSER_CDP_URL" >&2
        exit 2
    fi
    command -v node > /dev/null || { echo "perf.sh: node is needed (the version of .nvmrc)" >&2; exit 2; }
fi

if $start && [[ -z "$fake" ]]; then
    python3 "$SERVER" "$SERVER_PORT" > "$work/server.log" 2>&1 &
    server_pid=$!
    "$ROOT/scripts/run-windows.sh" --release -- --project "$FIXTURE"
    echo "perf.sh: waiting 20 s for the views to load"
    sleep 20
fi

mkdir -p "$out"
report_args=()
for ((n = 1; n <= runs; n++)); do
    run_file="$work/run-$n.json"
    if [[ -n "$fake" ]]; then
        cp "$fake/run-$n.json" "$run_file" 2> /dev/null \
            || { echo "perf.sh: $fake/run-$n.json is missing" >&2; exit 2; }
        memory="$(cat "$fake/memory-$n" 2> /dev/null || true)"
    else
        echo "perf.sh: run $n of $runs ($seconds s)"
        if ! node "$ROOT/scripts/lib/perf-measure.mjs" --seconds "$seconds" --view "$view" --out "$run_file"; then
            echo "perf.sh: run $n failed" >&2
            exit 2
        fi
        memory="$(sample_memory)"
    fi
    report_args+=(--run "$run_file" "$memory")
done

k1_file="$work/k1.json"
if [[ -n "$fake" ]]; then
    if [[ "$clicks" -gt 0 && -f "$fake/k1.json" ]]; then
        cp "$fake/k1.json" "$k1_file"
        report_args+=(--k1 "$k1_file")
    fi
elif [[ "$clicks" -gt 0 ]]; then
    echo "perf.sh: K1, $clicks click(s) on latency.html"
    if ! node "$ROOT/scripts/lib/perf-k1.mjs" --clicks "$clicks" --view "$view" --out "$k1_file"; then
        echo "perf.sh: the click run failed" >&2
        exit 2
    fi
    report_args+=(--k1 "$k1_file")
fi
if [[ -n "$counters" ]]; then
    report_args+=(--counters "$counters")
fi

set +e
python3 "$ROOT/scripts/lib/perf_report.py" combine --views "$views" --dpr3 "$dpr3" \
    --out "$out/$stamp.json" "${report_args[@]}"
status=$?
set -e
exit "$status"

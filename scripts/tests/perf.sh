#!/usr/bin/env bash
#
# scripts/perf.sh (RB-020-09, K3) without Windows and without CDP: --fake hands it the runs and
# memory samples a measurement would give, so the JSON it writes, its summary and its exit code
# are checked here; --dry-run, the usage errors, the baseline comparison of
# scripts/lib/perf_report.py and the frame statistics of scripts/lib/perf-measure.mjs (when a
# node is on PATH) too.
#
#   scripts/tests/perf.sh
# shellcheck disable=SC2016  # single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

GIB=$((1024 * 1024 * 1024))

# A run in folder $1 as number $2 with fps $3 and $4 views (the first $5 of them at DPR 3).
fake_run() {
    python3 - "$@" << 'PY'
import json, sys
folder, number, fps, count, dpr3 = sys.argv[1], sys.argv[2], float(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5])
views = [
    {"index": i, "url": "http://127.0.0.1:18766/scroll.html", "width": 400, "dpr": 3 if i < dpr3 else 1,
     "cpuSeconds": 0.5, "cpuShare": 0.05}
    for i in range(count)
]
run = {"seconds": 10, "measuredView": 0, "fps": fps, "p95FrameMs": 18.2, "frames": int(fps * 10), "views": views}
with open(f"{folder}/run-{number}.json", "w", encoding="utf-8") as handle:
    json.dump(run, handle)
PY
}

# Folder $1 with three runs at fps $2 $3 $4, 8 views (2 at DPR 3) and memory $5 bytes each.
fake_dir() {
    mkdir -p "$1"
    local n=1
    for fps in "$2" "$3" "$4"; do
        fake_run "$1" "$n" "$fps" 8 2
        echo "17 $5" > "$1/memory-$n"
        n=$((n + 1))
    done
}

json_field() {
    python3 -c 'import json,sys; d=json.load(open(sys.argv[1]))
for key in sys.argv[2].split("."): d = d[int(key)] if isinstance(d, list) else d[key]
print(json.dumps(d))' "$1" "$2"
}

only_json() {
    find "$1" -maxdepth 1 -name '*.json' | head -n 1
}

# The budget met: exit 0, the JSON in --out, the summary.
fake_dir "$SCRATCH/good" 58 60 59 $((2 * GIB))
run_status "$ROOT/scripts/perf.sh" --fake "$SCRATCH/good" --out "$SCRATCH/out-good"
expect_status "a run within the budget exits 0" 0
expect_output "the summary says K3 is met" "K3 met"
result="$(only_json "$SCRATCH/out-good")"
expect_true "the result is written under --out" '[[ -f "$result" ]]'
expect "the result names the job" '"RB-020-09"' "$(json_field "$result" job)"
expect "three runs are kept" "3" "$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["runs"]))' "$result")"
expect "the mean fps" "59.0" "$(json_field "$result" fps.mean)"
expect "the memory in MB" "2048" "$(json_field "$result" memoryMb.max)"
expect "the views seen" '{"views": 8, "dpr3": 2}' "$(json_field "$result" seen)"
expect "every check passes" "true" "$(json_field "$result" ok)"
expect "the processes of a run" "17" "$(json_field "$result" runs.0.processes)"

# Too slow.
fake_dir "$SCRATCH/slow" 45 46 45 $((2 * GIB))
run_status "$ROOT/scripts/perf.sh" --fake "$SCRATCH/slow" --out "$SCRATCH/out-slow"
expect_status "fps below 50 exits 1" 1
expect_output "the summary names the fps" "K3 missed: fps"

# Too much memory.
fake_dir "$SCRATCH/heavy" 58 60 59 $((3 * GIB + 1))
run_status "$ROOT/scripts/perf.sh" --fake "$SCRATCH/heavy" --out "$SCRATCH/out-heavy"
expect_status "3 GB or more exits 1" 1
expect_output "the summary names the memory" "K3 missed: memory"

# Not reproducible: the runs 15 % apart.
fake_dir "$SCRATCH/shaky" 52 60 60 $((2 * GIB))
run_status "$ROOT/scripts/perf.sh" --fake "$SCRATCH/shaky" --out "$SCRATCH/out-shaky"
expect_status "runs more than 10 % apart exit 1" 1
expect_output "the summary names the spread" "reproducible"

# Other views than asked for.
run_status "$ROOT/scripts/perf.sh" --fake "$SCRATCH/good" --views 6 --out "$SCRATCH/out-six"
expect_status "a grid other than asked exits 1" 1
expect_output "the summary names the views" "K3 missed: views"

# A memory sample that does not read is a failed run.
fake_dir "$SCRATCH/broken" 58 60 59 $((2 * GIB))
echo "no numbers" > "$SCRATCH/broken/memory-2"
run_status "$ROOT/scripts/perf.sh" --fake "$SCRATCH/broken" --out "$SCRATCH/out-broken"
expect_status "a broken memory sample exits 2" 2

# Fewer runs than asked for in the fake folder.
run_status "$ROOT/scripts/perf.sh" --fake "$SCRATCH/good" --runs 4 --out "$SCRATCH/out-four"
expect_status "a missing run exits 2" 2

# The dry run runs nothing.
run_status "$ROOT/scripts/perf.sh" --dry-run --start --runs 2 --out "$SCRATCH/out-dry"
expect_status "the dry run exits 0" 0
expect_output "the dry run names the measuring script" "perf-measure.mjs --seconds 10 --view 0"
expect_output "the dry run names the fixture" "--project tests/fixtures/project-perf"
expect_true "the dry run writes nothing" '[[ ! -d "$SCRATCH/out-dry" ]]'

# Usage errors.
run_status "$ROOT/scripts/perf.sh" --runs many
expect_status "a word for a number exits 2" 2
run_status "$ROOT/scripts/perf.sh" --frobnicate
expect_status "an unknown argument exits 2" 2
run_status env -u RBROWSER_CDP_URL -u RBROWSER_CDP_ENDPOINT "$ROOT/scripts/perf.sh" --out "$SCRATCH/out-none"
expect_status "no endpoint exits 2" 2
expect_output "no endpoint names the variables" "RBROWSER_CDP_ENDPOINT"

# The comparison with the baseline is a hint, never a failure.
cat > "$SCRATCH/budget.md" << 'MD'
# Budget

```json perf-baseline
{ "fps": 70, "memoryMb": 1500 }
```
MD
run_status python3 "$ROOT/scripts/lib/perf_report.py" compare "$result" "$SCRATCH/budget.md"
expect_status "a regression is no failure" 0
expect_output "an fps regression is named" "below the baseline 70"
expect_output "a memory regression is named" "above the baseline 1500 MB"
printf '```json perf-baseline\n{ "fps": 59, "memoryMb": 2048 }\n```\n' > "$SCRATCH/same.md"
run_status python3 "$ROOT/scripts/lib/perf_report.py" compare "$result" "$SCRATCH/same.md"
expect "no regression, no hint" "" "$output"
run_status "$ROOT/scripts/perf.sh" --compare --out "$SCRATCH/empty"
expect_status "nothing to compare is no failure" 0
expect_output "nothing to compare says so" "no measurement"
run_status "$ROOT/scripts/perf.sh" --compare "$result"
expect_status "the checked-in baseline compares" 0

# The frame statistics of the measuring script.
if command -v node > /dev/null; then
    stats="$(cd "$ROOT" && node --input-type=module -e '
import { frameStats } from "./scripts/lib/perf-measure.mjs";
const frames = Array.from({ length: 61 }, (_, i) => i * 1000 / 60);
console.log(JSON.stringify(frameStats(frames)), JSON.stringify(frameStats([5])));')"
    expect "60 frames in a second are 60 fps" '{"fps":60,"p95FrameMs":16.7,"frames":61} {"fps":0,"p95FrameMs":0,"frames":1}' "$stats"
else
    ok "node is not on PATH: the frame statistics are checked where it is"
fi

finish_tests "perf"

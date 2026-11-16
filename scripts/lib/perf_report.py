#!/usr/bin/env python3
"""The report of scripts/perf.sh (RB-020-09, K3): the runs of scripts/lib/perf-measure.mjs and
the memory samples of each run become one JSON file and a summary.

    perf_report.py combine --views 8 --dpr3 2 --out FILE --run RUN.json MEMORY [--run ...]
    perf_report.py compare RESULT.json docs/perf-budget.md

`combine` writes FILE and prints the summary; it exits 1 when the budget is missed (fps below
50, memory at or above 3 GB, the runs more than 10 % apart, or the views not what was asked)
and 2 on bad input. MEMORY is "<processes> <bytes>" as perf.sh samples it.

`compare` holds a result against the baseline of docs/perf-budget.md (the fenced block
`json perf-baseline`) and prints a hint for a regression above 10 %; it never fails a run
(check.sh --full calls it without Windows).
"""

import argparse
import datetime
import json
import re
import sys

sys.dont_write_bytecode = True

BUDGET_FPS = 50.0
BUDGET_MEMORY_MB = 3072
MAX_SPREAD_PCT = 10.0
REGRESSION_PCT = 10.0
MIB = 1024 * 1024
BASELINE = re.compile(r"```json perf-baseline\n(.*?)\n```", re.S)


class InputError(Exception):
    """A run or memory sample that does not read."""


def read_run(path):
    try:
        with open(path, encoding="utf-8") as handle:
            run = json.load(handle)
    except (OSError, ValueError) as error:
        raise InputError(f"{path}: {error}") from error
    if not isinstance(run, dict) or not isinstance(run.get("fps"), (int, float)):
        raise InputError(f"{path}: no fps")
    if not isinstance(run.get("views"), list):
        raise InputError(f"{path}: no views")
    return run


def parse_memory(text):
    parts = text.split()
    if len(parts) != 2 or not all(part.isdigit() for part in parts):
        raise InputError(f"memory sample {text!r} is not '<processes> <bytes>'")
    return int(parts[0]), int(parts[1])


def spread_pct(values):
    """(max - min) / mean in percent; 0 for fewer than two values or a zero mean."""
    if len(values) < 2:
        return 0.0
    mean = sum(values) / len(values)
    return 0.0 if mean == 0 else round((max(values) - min(values)) / mean * 100, 1)


def combine(runs, views, dpr3, now):
    """The report of `runs`: [(run dict, (processes, bytes))]."""
    rows = []
    for run, (processes, private_bytes) in runs:
        rows.append(
            {
                "fps": run["fps"],
                "p95FrameMs": run.get("p95FrameMs", 0),
                "seconds": run.get("seconds", 0),
                "memoryMb": round(private_bytes / MIB),
                "processes": processes,
                "views": run["views"],
            }
        )
    fps = [row["fps"] for row in rows]
    memory = [row["memoryMb"] for row in rows]
    seen_views = len(rows[0]["views"]) if rows else 0
    seen_dpr3 = sum(1 for view in rows[0]["views"] if view.get("dpr") == 3) if rows else 0
    fps_summary = {
        "mean": round(sum(fps) / len(fps), 1) if fps else 0,
        "min": min(fps, default=0),
        "max": max(fps, default=0),
        "spreadPct": spread_pct(fps),
    }
    checks = {
        "views": seen_views == views and seen_dpr3 == dpr3,
        "fps": bool(fps) and min(fps) >= BUDGET_FPS,
        "memory": bool(memory) and max(memory) < BUDGET_MEMORY_MB,
        "reproducible": fps_summary["spreadPct"] < MAX_SPREAD_PCT,
    }
    return {
        "schema": 1,
        "job": "RB-020-09",
        "date": now.strftime("%Y-%m-%dT%H:%M:%S"),
        "asked": {"views": views, "dpr3": dpr3},
        "seen": {"views": seen_views, "dpr3": seen_dpr3},
        "budget": {"fps": BUDGET_FPS, "memoryMb": BUDGET_MEMORY_MB, "spreadPct": MAX_SPREAD_PCT},
        "runs": rows,
        "fps": fps_summary,
        "memoryMb": {"max": max(memory, default=0)},
        "checks": checks,
        "ok": all(checks.values()),
    }


def summary(report):
    fps = report["fps"]
    lines = [
        f"views {report['seen']['views']} (DPR 3: {report['seen']['dpr3']}), "
        f"{len(report['runs'])} run(s)",
        f"fps of the active view: mean {fps['mean']}, min {fps['min']}, max {fps['max']}, "
        f"spread {fps['spreadPct']} % (budget >= {report['budget']['fps']:g}, < "
        f"{report['budget']['spreadPct']:g} %)",
        f"memory of all processes: max {report['memoryMb']['max']} MB "
        f"(budget < {report['budget']['memoryMb']} MB)",
    ]
    failed = [name for name, passed in report["checks"].items() if not passed]
    lines.append("K3 met" if report["ok"] else f"K3 missed: {', '.join(failed)}")
    return "\n".join(lines)


def baseline_of(markdown):
    match = BASELINE.search(markdown)
    if match is None:
        return None
    try:
        baseline = json.loads(match.group(1))
    except ValueError:
        return None
    return baseline if isinstance(baseline, dict) else None


def regressions(report, baseline):
    """The hints for `report` against `baseline` ({"fps": n, "memoryMb": n}; 0 is unmeasured)."""
    hints = []
    base_fps = baseline.get("fps") or 0
    base_memory = baseline.get("memoryMb") or 0
    fps = report["fps"]["mean"]
    memory = report["memoryMb"]["max"]
    if base_fps and fps < base_fps * (1 - REGRESSION_PCT / 100):
        hints.append(f"fps {fps} is more than {REGRESSION_PCT:g} % below the baseline {base_fps}")
    if base_memory and memory > base_memory * (1 + REGRESSION_PCT / 100):
        hints.append(
            f"memory {memory} MB is more than {REGRESSION_PCT:g} % above the baseline {base_memory} MB"
        )
    return hints


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    joined = commands.add_parser("combine")
    joined.add_argument("--views", type=int, required=True)
    joined.add_argument("--dpr3", type=int, required=True)
    joined.add_argument("--out", required=True)
    joined.add_argument("--run", nargs=2, action="append", required=True, metavar=("RUN", "MEMORY"))
    compared = commands.add_parser("compare")
    compared.add_argument("result")
    compared.add_argument("budget")
    options = parser.parse_args(argv)
    try:
        if options.command == "combine":
            runs = [(read_run(path), parse_memory(memory)) for path, memory in options.run]
            report = combine(runs, options.views, options.dpr3, datetime.datetime.now())
            with open(options.out, "w", encoding="utf-8") as handle:
                json.dump(report, handle, indent=2)
                handle.write("\n")
            print(summary(report))
            print(f"written: {options.out}")
            return 0 if report["ok"] else 1
        with open(options.result, encoding="utf-8") as handle:
            report = json.load(handle)
        with open(options.budget, encoding="utf-8") as handle:
            baseline = baseline_of(handle.read())
    except InputError as error:
        print(f"perf_report: {error}", file=sys.stderr)
        return 2
    except (OSError, ValueError) as error:
        print(f"perf_report: hint: no comparison ({error})")
        return 0
    if baseline is None:
        print("perf_report: hint: docs/perf-budget.md has no perf-baseline block")
        return 0
    try:
        hints = regressions(report, baseline)
    except (KeyError, TypeError, AttributeError):
        print(f"perf_report: hint: {options.result} is no perf.sh result")
        return 0
    for hint in hints:
        print(f"perf_report: hint: {hint}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

#!/usr/bin/env python3
"""The report of scripts/perf.sh (RB-020-09, K3; RB-090-32, K1, K11, start-up): the runs of
scripts/lib/perf-measure.mjs and the memory samples of each run, the click run of
scripts/lib/perf-k1.mjs and the answer of the MCP tool app_perf become one JSON file and a
summary.

    perf_report.py combine --views 8 --dpr3 2 --out FILE --run RUN.json MEMORY [--run ...]
                           [--k1 K1.json] [--counters APP_PERF.json]
    perf_report.py compare RESULT.json docs/perf-budget.md

`combine` writes FILE and prints the summary; it exits 1 when the budget is missed (fps below
50, memory at or above 3 GB, the runs more than 10 % apart, or the views not what was asked;
the click latency's p95 above 150 ms - K1; the p95 of the replays into Firefox or WebKit above
300 ms - K11) and 2 on bad input. MEMORY is "<processes> <bytes>" as perf.sh samples it.

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
BUDGET_K1_MS = 150.0
BUDGET_K11_MS = 300.0
K11_ENGINES = ("firefox", "webkit")
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


def read_json(path, what):
    try:
        with open(path, encoding="utf-8") as handle:
            value = json.load(handle)
    except (OSError, ValueError) as error:
        raise InputError(f"{what} {path}: {error}") from error
    if not isinstance(value, dict):
        raise InputError(f"{what} {path}: not a JSON object")
    return value

def p95_of(counters, name):
    """The p95 (ms) of counter `name` in an app_perf answer; None without samples."""
    for counter in counters.get("counters") or []:
        if isinstance(counter, dict) and counter.get("name") == name and counter.get("count"):
            value = counter.get("p95Ms")
            return value if isinstance(value, (int, float)) else None
    return None

def number(value):
    """A figure as the summary prints it: whole numbers without a point."""
    return f"{value:.0f}" if float(value).is_integer() else f"{value:.1f}"

def latency(k1, counters):
    """The K1, K11 and start-up parts of the report and their checks."""
    parts, checks = {}, {}
    if k1 is not None:
        overall = k1.get("overall")
        if not isinstance(overall, dict) or not isinstance(overall.get("p95Ms"), (int, float)):
            raise InputError("the click run has no overall p95Ms")
        parts["k1"] = {
            "clicks": k1.get("clicks", 0),
            **{key: overall.get(key, 0) for key in ("pairs", "missed", "p50Ms", "p95Ms", "maxMs")},
            "views": k1.get("views", []),
        }
        checks["k1"] = overall["p95Ms"] <= BUDGET_K1_MS
    if counters is not None:
        hub = p95_of(counters, "sync.replay.chromium")
        if hub is not None:
            parts.setdefault("k1", {})["hubP95Ms"] = hub
        engines = {engine: p95_of(counters, f"sync.replay.{engine}") for engine in K11_ENGINES}
        measured = [value for value in engines.values() if value is not None]
        parts["k11"] = {
            "p95Ms": max(measured) if measured else None,
            **{f"{engine}P95Ms": value for engine, value in engines.items()},
            "navigationP95Ms": p95_of(counters, "sync.navigation"),
        }
        if measured:
            checks["k11"] = max(measured) <= BUDGET_K11_MS
        parts["coldStartMs"] = p95_of(counters, "startup.firstView")
    return parts, checks

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


def combine(runs, views, dpr3, now, k1=None, counters=None):
    """The report of `runs`: [(run dict, (processes, bytes))], with the click run `k1` and the
    app_perf answer `counters` when given."""
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
    # Without a scroll run (perf.sh --runs 0) K3 is not measured, not missed.
    checks = {
        "views": seen_views == views and seen_dpr3 == dpr3,
        "fps": bool(fps) and min(fps) >= BUDGET_FPS,
        "memory": bool(memory) and max(memory) < BUDGET_MEMORY_MB,
        "reproducible": fps_summary["spreadPct"] < MAX_SPREAD_PCT,
    } if rows else {}
    parts, latency_checks = latency(k1, counters)
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
        **parts,
        "latencyChecks": latency_checks,
        "budgetLatencyMs": {"k1P95": BUDGET_K1_MS, "k11P95": BUDGET_K11_MS},
        "ok": all(checks.values()) and all(latency_checks.values()),
    }


def summary(report):
    fps = report["fps"]
    if not report["runs"]:
        return "\n".join(["K3 not measured (no scroll run)", *latency_summary(report)])
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
    lines.append(f"K3 missed: {', '.join(failed)}" if failed else "K3 met")
    lines.extend(latency_summary(report))
    return "\n".join(lines)

def latency_summary(report):
    lines = []
    checks = report.get("latencyChecks", {})
    k1 = report.get("k1", {})
    if "k1" in checks:
        lines.append(
            f"click latency (K1): p50 {number(k1['p50Ms'])} ms, p95 {number(k1['p95Ms'])} ms over "
            f"{k1['pairs']} view click(s), {k1['missed']} missed (budget p95 <= {BUDGET_K1_MS:g} ms)"
        )
        lines.append("K1 met" if checks["k1"] else f"K1 missed: p95 {number(k1['p95Ms'])} ms")
    if "hubP95Ms" in k1:
        lines.append(f"sync hub to Chromium replay: p95 {number(k1['hubP95Ms'])} ms")
    k11 = report.get("k11")
    if k11 is not None:
        engines = ", ".join(
            f"{engine} {number(k11[f'{engine}P95Ms'])}"
            for engine in K11_ENGINES
            if k11.get(f"{engine}P95Ms") is not None
        )
        if "k11" in checks:
            lines.append(
                f"replay into Firefox and WebKit (K11): p95 {number(k11['p95Ms'])} ms ({engines}; "
                f"budget p95 <= {BUDGET_K11_MS:g} ms)"
            )
            lines.append("K11 met" if checks["k11"] else f"K11 missed: p95 {number(k11['p95Ms'])} ms")
        else:
            lines.append("K11: no Firefox or WebKit replay in the app_perf answer")
        if k11.get("navigationP95Ms") is not None:
            lines.append(f"navigation followed (every engine): p95 {number(k11['navigationP95Ms'])} ms")
    if report.get("coldStartMs") is not None:
        lines.append(f"start-up to the first view loaded: {number(report['coldStartMs'])} ms")
    return lines


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
    if not report.get("runs"):
        # A result of the counters alone (perf.sh --runs 0) has no fps or memory to compare.
        return hints
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
    joined.add_argument("--run", nargs=2, action="append", default=[], metavar=("RUN", "MEMORY"))
    joined.add_argument("--k1", help="the result of scripts/lib/perf-k1.mjs")
    joined.add_argument("--counters", help="the answer of the MCP tool app_perf")
    compared = commands.add_parser("compare")
    compared.add_argument("result")
    compared.add_argument("budget")
    options = parser.parse_args(argv)
    try:
        if options.command == "combine":
            runs = [(read_run(path), parse_memory(memory)) for path, memory in options.run]
            k1 = read_json(options.k1, "the click run") if options.k1 else None
            counters = read_json(options.counters, "the app_perf answer") if options.counters else None
            if not runs and k1 is None and counters is None:
                raise InputError("nothing to report: no run, no click run, no app_perf answer")
            report = combine(
                runs, options.views, options.dpr3, datetime.datetime.now(), k1=k1, counters=counters
            )
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

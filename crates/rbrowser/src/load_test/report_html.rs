//! The report as a page of its own (RB-096-07): `.rbrowser/out/load-test/<id>.html`, readable
//! without rBrowser - the run's facts, the totals, failures by kind, each step's share and the
//! requests per second and p95 over time as an inline SVG. No script, no outside resource;
//! every text from the report escaped ([`rb_plugin_types::html::escape`]).

use std::fmt::Write as _;

use rb_plugin_types::html::escape;
use serde_json::Value;

const STYLE: &str = "body{font:14px system-ui,sans-serif;margin:2rem;color:#1f2328;background:#fff}\
h1{font-size:1.4rem}table{border-collapse:collapse;margin:1rem 0}td,th{border:1px solid #d0d7de;\
padding:.3rem .6rem;text-align:left}th{background:#f6f8fa}.note{color:#59636e}\
@media (prefers-color-scheme:dark){body{color:#e6edf3;background:#0d1117}th{background:#161b22}\
td,th{border-color:#30363d}.note{color:#9198a1}}";

fn text(value: &Value) -> String {
    match value {
        Value::String(text) => escape(text),
        Value::Null => String::new(),
        Value::Number(number) => number.as_f64().map_or_else(
            || number.to_string(),
            |n| format!("{}", (n * 10.0).round() / 10.0),
        ),
        other => escape(&other.to_string()),
    }
}

fn row(out: &mut String, label: &str, value: &str) {
    let _ = write!(out, "<tr><th>{}</th><td>{value}</td></tr>", escape(label));
}

/// The polyline of `values` (one per second) in a box `width` by `height`, scaled to `top`.
fn line(values: &[f64], width: f64, height: f64, top: f64) -> String {
    let count = values.len().max(2) as f64 - 1.0;
    values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = i as f64 / count * width;
            let y = height - (v / top).clamp(0.0, 1.0) * height;
            format!("{x:.1},{y:.1}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn chart(series: &[Value]) -> String {
    let rps: Vec<f64> = series
        .iter()
        .map(|p| p["requests"].as_f64().unwrap_or(0.0))
        .collect();
    let p95: Vec<f64> = series
        .iter()
        .map(|p| p["p95"].as_f64().unwrap_or(0.0))
        .collect();
    if rps.is_empty() {
        return String::new();
    }
    let top = |values: &[f64]| values.iter().copied().fold(1.0_f64, f64::max);
    let (width, height) = (720.0, 160.0);
    format!(
        "<svg viewBox=\"0 0 {width} {height}\" width=\"100%\" role=\"img\" aria-label=\"Requests per second and p95 over time\">\
<polyline fill=\"none\" stroke=\"#0969da\" stroke-width=\"2\" points=\"{}\"/>\
<polyline fill=\"none\" stroke=\"#bf3989\" stroke-width=\"2\" stroke-dasharray=\"4 3\" points=\"{}\"/></svg>\
<p class=\"note\">Solid: requests per second (up to {}). Dashed: p95 in ms (up to {}).</p>",
        line(&rps, width, height, top(&rps)),
        line(&p95, width, height, top(&p95)),
        top(&rps).round(),
        top(&p95).round(),
    )
}

/// The page of `report`.
#[must_use]
pub fn render(report: &Value) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Load test: {}</title><style>{STYLE}</style></head><body>",
        text(&report["name"])
    );
    let _ = write!(out, "<h1>Load test: {}</h1>", text(&report["name"]));
    out.push_str(
        "<p class=\"note\">Run by rBrowser against a system you own or may test. Every request named itself \
         with the User-Agent \"rBrowser load test\".</p><table>",
    );
    let profile = &report["profile"];
    let totals = &report["totals"];
    let latency = &totals["latency"];
    let targets: Vec<String> = report["targets"]
        .as_array()
        .map(|list| list.iter().map(|t| text(&t["host"])).collect())
        .unwrap_or_default();
    row(&mut out, "Scenario", &text(&report["scenario"]));
    row(&mut out, "Started", &text(&report["startedAt"]));
    row(&mut out, "Ended", &text(&report["end"]));
    if let Some(rule) = report["abort"]["rule"].as_str() {
        let reason = format!(
            "{} {} over {}",
            escape(rule),
            text(&report["abort"]["value"]),
            text(&report["abort"]["threshold"])
        );
        row(&mut out, "Abort rule", &reason);
    }
    row(&mut out, "Targets", &targets.join(", "));
    row(&mut out, "Users", &text(&profile["users"]));
    let ramp = format!(
        "{} s up, {} s hold, {} s down",
        text(&profile["rampUpS"]),
        text(&profile["holdS"]),
        text(&profile["rampDownS"])
    );
    row(&mut out, "Profile", &ramp);
    row(&mut out, "Rate limit (req/s)", &text(&report["rpsLimit"]));
    row(&mut out, "Duration (s)", &text(&report["elapsedS"]));
    row(&mut out, "Requests", &text(&totals["requests"]));
    row(&mut out, "Requests per second", &text(&totals["rps"]));
    row(&mut out, "Failures", &text(&totals["errors"]));
    row(&mut out, "Failure rate", &text(&totals["errorRate"]));
    for key in ["p50", "p95", "p99", "min", "max", "mean"] {
        row(
            &mut out,
            &format!("Latency {key} (ms)"),
            &text(&latency[key]),
        );
    }
    out.push_str("</table>");
    if let Some(errors) = report["errors"].as_object().filter(|e| !e.is_empty()) {
        out.push_str("<h2>Failures by kind</h2><table>");
        for (kind, count) in errors {
            row(&mut out, kind, &text(count));
        }
        out.push_str("</table>");
    }
    if let Some(steps) = report["steps"].as_array() {
        out.push_str("<h2>Steps</h2><table><tr><th>Request</th><th>Requests</th><th>Failures</th><th>Mean (ms)</th></tr>");
        for step in steps {
            let _ = write!(
                out,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                text(&step["request"]),
                text(&step["requests"]),
                text(&step["errors"]),
                text(&step["meanMs"])
            );
        }
        out.push_str("</table>");
    }
    if let Some(series) = report["series"].as_array() {
        out.push_str("<h2>Over time</h2>");
        out.push_str(&chart(series));
    }
    out.push_str("</body></html>\n");
    out
}

#[cfg(test)]
#[path = "report_html_tests.rs"]
mod tests;

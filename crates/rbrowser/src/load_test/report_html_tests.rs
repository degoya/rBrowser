use serde_json::json;

use super::*;

#[test]
fn the_page_escapes_every_text_and_draws_the_series() {
    let report = json!({
        "name": "<script>alert(1)</script>",
        "scenario": "a.json",
        "startedAt": "2026-11-21T14:05:01Z",
        "end": "aborted",
        "abort": { "rule": "p95Ms", "value": 2500.0, "threshold": 2000.0 },
        "targets": [{ "host": "localhost", "standing": "local" }],
        "profile": { "users": 50, "rampUpS": 10, "holdS": 40, "rampDownS": 10 },
        "rpsLimit": 200.0,
        "elapsedS": 61.23,
        "totals": { "requests": 1000, "errors": 3, "errorRate": 0.003, "rps": 16.4,
                    "latency": { "p50": 10.0, "p95": 20.0, "p99": 30.0, "min": 1.0, "max": 40.0, "mean": 12.0 } },
        "errors": { "http5xx": 3 },
        "steps": [{ "request": "x.http", "requests": 1000, "errors": 3, "meanMs": 12.0 }],
        "series": [{ "t": 1, "requests": 10, "p95": 20.0 }, { "t": 2, "requests": 20, "p95": 25.0 }],
    });
    let page = render(&report);
    assert!(!page.contains("<script>"));
    assert!(page.contains("&lt;script&gt;"));
    assert!(page.contains("p95Ms 2500 over 2000"));
    assert!(page.contains("<polyline"));
    assert!(page.contains("http5xx"));
    assert!(page.contains("rBrowser load test"));
    assert!(page.contains("61.2"));
}

#[test]
fn an_empty_run_still_renders() {
    let page = render(&json!({ "name": "x", "series": [], "steps": [] }));
    assert!(page.starts_with("<!doctype html>"));
    assert!(!page.contains("<polyline"));
}

use serde_json::json;

use rb_plugin_api::PluginHost;

use super::{load, start, take};
use crate::fake::{FakeHost, PAGE, Sheet};
use crate::model::{Kind, Status};
use crate::resolve::Cache;
use crate::settings::Settings;

const WIDE: &str = "body { margin: 0 }\n@media (min-width: 1024px) { .wide { display: grid } }\n";

/// The fixture's sheet: `body` (0..18) used everywhere, `.wide` (48..71) only at >= 1024 px.
fn sheet(wide: bool) -> Sheet {
    let mut used = vec![(0, 18)];
    if wide {
        used.push((48, 71));
    }
    Sheet {
        id: "s1",
        href: Some("http://localhost:8080/coverage/site.css"),
        text: WIDE.to_owned(),
        used,
    }
}

fn unused_sheet() -> Sheet {
    Sheet {
        id: "s2",
        href: Some("http://localhost:8080/coverage/unused.css"),
        text: ".never { color: red }\n".repeat(1000),
        used: Vec::new(),
    }
}

fn script(id: &str, count: u64) -> serde_json::Value {
    json!({ "scriptId": id, "url": "http://localhost:8080/coverage/app.js", "functions": [
        { "functionName": "", "ranges": [{ "startOffset": 0, "endOffset": 100, "count": 1 }] },
        { "functionName": "onClick", "ranges": [{ "startOffset": 20, "endOffset": 60, "count": count }] }
    ] })
}

#[test]
fn a_load_run_reloads_measures_and_stops_everything() {
    let mut host = FakeHost::new()
        .view(1, "desktop", 1280, vec![sheet(true), unused_sheet()])
        .view(2, "mobile", 375, vec![sheet(false), unused_sheet()])
        .scripts(
            1,
            json!([script("7", 0)]),
            json!([script("7", 0), script("12", 0)]),
        )
        .scripts(2, json!([]), json!([script("30", 0)]));
    let views = host.views().expect("views");
    let runs = load(&mut host, &views, &Settings::default());
    assert!(!host.tracking(), "nothing tracks after the run");
    assert_eq!(runs.len(), 2);
    let desktop = &runs[0];
    assert_eq!(desktop.status, Status::Measured);
    assert_eq!(desktop.url, PAGE);
    let css = &desktop.files[0];
    assert_eq!(css.url, "http://localhost:8080/coverage/site.css");
    assert_eq!(css.total, 74);
    assert_eq!(css.used, 18 + 23);
    assert_eq!(css.rules, vec![(0, 18), (48, 71)]);
    let unused = &desktop.files[1];
    assert_eq!(unused.used, 0);
    assert_eq!(unused.total, 22_000);
    assert_eq!(unused.unused_percent(), 100.0);
    // The script of the document before the reload (id 7) is left out.
    let js: Vec<_> = desktop
        .files
        .iter()
        .filter(|file| file.kind == Kind::Js)
        .collect();
    assert_eq!(js.len(), 1);
    assert_eq!(js[0].used, 60);
    assert_eq!(js[0].functions.len(), 1);
    assert!(!js[0].functions[0].executed);
    assert_eq!(runs[1].files[0].used, 18);
    let order = host.methods(1);
    let reload_at = order
        .iter()
        .position(|method| *method == "Profiler.takePreciseCoverage")
        .expect("drained");
    let css_at = order
        .iter()
        .position(|method| *method == "CSS.startRuleUsageTracking")
        .expect("tracked");
    assert!(
        reload_at < css_at,
        "CSS is tracked after the reload: {order:?}"
    );
}

#[test]
fn a_failing_view_is_marked_and_still_stopped() {
    let mut host = FakeHost::new()
        .view(1, "desktop", 1280, vec![sheet(true)])
        .view(2, "mobile", 375, vec![sheet(false)])
        .view(3, "firefox", 1280, Vec::new())
        .engine(3, "firefox")
        .fail(2, "CSS.stopRuleUsageTracking");
    let views = host.views().expect("views");
    let runs = load(&mut host, &views, &Settings::default());
    assert_eq!(runs[0].status, Status::Measured);
    assert_eq!(runs[1].status, Status::Failed);
    assert_eq!(runs[2].status, Status::Unavailable);
    assert_eq!(
        runs[2].error.as_deref(),
        Some("plugins.error.not_available")
    );
    // View 2's CSS tracking could not be stopped by the fake; precise coverage was.
    assert!(!host.views[1].precise);
    assert!(host.methods(3).is_empty());
}

#[test]
fn an_interaction_session_takes_deltas_and_stops_at_the_end() {
    let mut host = FakeHost::new()
        .view(1, "desktop", 1280, vec![sheet(true)])
        .scripts(1, json!([script("7", 1)]), json!([]));
    let views = host.views().expect("views");
    let runs = start(&mut host, &views);
    assert!(host.tracking());
    let settings = Settings::default();
    let mut cache = Cache::default();
    let delta = take(&mut host, &runs[0], false, &settings, &mut cache);
    assert!(host.tracking(), "a delta keeps tracking");
    let js = delta
        .files
        .iter()
        .find(|file| file.kind == Kind::Js)
        .expect("script");
    assert!(js.functions[0].executed);
    let last = take(&mut host, &runs[0], true, &settings, &mut cache);
    assert!(!host.tracking(), "stop ends both kinds of tracking");
    assert_eq!(last.status, Status::Measured);
}

#[test]
fn a_view_that_cannot_start_is_stopped_again() {
    let mut host = FakeHost::new()
        .view(1, "desktop", 1280, vec![sheet(true)])
        .fail(1, "Profiler.startPreciseCoverage");
    let views = host.views().expect("views");
    let runs = start(&mut host, &views);
    assert_eq!(runs[0].status, Status::Failed);
    assert!(!host.tracking());
}

use serde_json::json;

use super::{export, file, get, parse, start, stop, targets};
use crate::fake::{FakeHost, NOW, Sheet};
use crate::model::Mode;
use crate::store::{self, Session};

fn sheet(wide: bool) -> Sheet {
    let mut used = vec![(0, 18)];
    if wide {
        used.push((48, 71));
    }
    Sheet {
        id: "s1",
        href: Some("http://localhost:8080/coverage/site.css"),
        text: "body { margin: 0 }\n@media (min-width: 1024px) { .wide { display: grid } }\n"
            .to_owned(),
        used,
    }
}

fn host() -> FakeHost {
    FakeHost::new()
        .view(1, "desktop", 1280, vec![sheet(true)])
        .view(2, "mobile", 375, vec![sheet(false)])
}

#[test]
fn get_load_measures_every_view_and_keeps_the_result() {
    let mut host = host();
    let answer = get(&mut host, &json!({ "mode": "load" })).expect("measured");
    assert_eq!(answer["mode"], "load");
    assert_eq!(answer["views"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        answer["files"][0]["url"],
        "http://localhost:8080/coverage/site.css"
    );
    assert_eq!(answer["findingCount"], 1);
    assert_eq!(answer["findings"][0]["rule"], "rule-partial");
    assert!(host.written.contains_key("last.json"));
    assert_eq!(store::session(&mut host).running, None);
    assert!(!host.tracking());
    let last = get(&mut host, &json!({ "mode": "last" })).expect("stored");
    assert_eq!(last["views"], answer["views"]);
}

#[test]
fn one_view_is_measured_for_a_target() {
    let mut host = host();
    let answer = get(&mut host, &json!({ "mode": "load", "target": 2 })).expect("measured");
    assert_eq!(answer["views"][0]["view"], 2);
    assert_eq!(answer["views"].as_array().map(Vec::len), Some(1));
    let missing = targets(&mut host, &json!(9)).expect_err("no view 9");
    assert_eq!(missing.code(), Some("plugins.error.not_found"));
    let bad = get(&mut host, &json!({ "mode": "sometimes" })).expect_err("bad mode");
    assert_eq!(bad.code(), Some("plugins.error.invalid"));
}

#[test]
fn a_session_is_started_taken_and_stopped() {
    let mut host = host();
    let started = start(&mut host, &json!({})).expect("started");
    assert_eq!(started["running"], true);
    assert!(host.tracking());
    assert_eq!(store::session(&mut host).running, Some(Mode::Interaction));
    let busy = get(&mut host, &json!({ "mode": "load" })).expect_err("a session runs");
    assert_eq!(busy.code(), Some("plugin.coverage.errors.running"));
    let delta = get(&mut host, &json!({ "mode": "delta" })).expect("delta");
    assert_eq!(delta["running"], true);
    assert_eq!(delta["views"][0]["files"][0]["used"], 41);
    let (summary, found) = stop(&mut host).expect("stopped");
    assert_eq!(summary["running"], false);
    assert_eq!(summary["mode"], "interaction");
    assert!(!found.is_empty());
    assert!(!host.tracking());
    assert_eq!(store::session(&mut host).running, None);
    assert!(host.written.contains_key("last.json"));
}

#[test]
fn delta_without_a_session_and_stop_without_anything_say_so() {
    let mut host = host();
    let delta = get(&mut host, &json!({ "mode": "delta" })).expect_err("no session");
    assert_eq!(delta.code(), Some("plugin.coverage.errors.not-running"));
    let stopped = stop(&mut host).expect_err("nothing");
    assert_eq!(stopped.code(), Some("plugin.coverage.errors.not-running"));
    let last = get(&mut host, &json!({ "mode": "last" })).expect_err("nothing");
    assert_eq!(last.code(), Some("plugin.coverage.errors.none"));
}

#[test]
fn a_load_run_cut_off_earlier_does_not_block_forever() {
    let mut host = host();
    let stale = Session {
        running: Some(Mode::Load),
        started: NOW - store::LOAD_STALE_SECONDS - 1,
        ..Session::default()
    };
    store::save_session(&mut host, &stale).expect("stored");
    assert!(get(&mut host, &json!({ "mode": "load" })).is_ok());
}

#[test]
fn the_default_mode_comes_from_the_settings() {
    let mut host = host().set("defaultMode", "\"interaction\"");
    let answer = get(&mut host, &json!({})).expect_err("delta without session");
    assert_eq!(answer.code(), Some("plugin.coverage.errors.not-running"));
}

#[test]
fn export_and_detail_read_the_last_measurement() {
    let mut host = host();
    assert!(export(&mut host).is_err());
    get(&mut host, &json!({ "mode": "load" })).expect("measured");
    let written = export(&mut host).expect("exported");
    assert_eq!(
        written["written"],
        json!([
            ".rbrowser/out/coverage/coverage.json",
            ".rbrowser/out/coverage/coverage.csv"
        ])
    );
    let csv = String::from_utf8(host.written["coverage.csv"].clone()).expect("text");
    assert!(csv.contains("http://localhost:8080/coverage/site.css,css,false,74,41,33,44.6"));
    let detail = file(
        &mut host,
        &json!({ "url": "http://localhost:8080/coverage/site.css" }),
    )
    .expect("detail");
    assert_eq!(detail["rules"][1]["usedIn"], json!([1]));
}

#[test]
fn arguments_must_be_an_object() {
    assert_eq!(parse("").expect("empty"), json!({}));
    assert!(parse("[1]").is_err());
}

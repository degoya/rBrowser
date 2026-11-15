use serde_json::json;

use super::*;
use crate::collect::PageState;
use crate::fake::FakeHost;

const PAGE: &str = "https://a.test/app/";

fn state(value: serde_json::Value) -> PageState {
    serde_json::from_value(value).expect("state")
}

fn active() -> PageState {
    state(
        json!({ "url": PAGE, "secure": true, "sw": { "supported": true, "controlled": true,
        "registration": { "scope": PAGE, "script": "https://a.test/app/sw.js", "state": "activated", "active": true } } }),
    )
}

fn run(host: &mut FakeHost, state: &PageState) -> (WorkerReport, Vec<String>) {
    let page = Url::parse(PAGE).expect("url");
    let budget = Budget::new(host, 30_000);
    let mut sink = Sink::new(&[], PAGE, Some(1));
    let report = check(host, &budget, &page, state, &mut sink);
    (report, sink.findings.into_iter().map(|f| f.rule).collect())
}

#[test]
fn fetch_handlers_are_found_in_their_usual_forms() {
    assert_eq!(
        fetch_handler("self.addEventListener('fetch', (e) => {})"),
        Some(true)
    );
    assert_eq!(
        fetch_handler("addEventListener ( \"fetch\" , f)"),
        Some(true)
    );
    assert_eq!(fetch_handler("self.onfetch = handle;"), Some(true));
    assert_eq!(
        fetch_handler("workbox.routing.registerRoute(r)"),
        Some(true)
    );
    assert_eq!(
        fetch_handler("self.addEventListener('install', f)"),
        Some(false)
    );
    assert_eq!(fetch_handler("importScripts('workbox-sw.js');"), None);
    assert_eq!(fetch_handler("import { x } from './y.js';"), None);
    assert_eq!(fetch_handler("const important = 1;"), Some(false));
}

#[test]
fn an_active_controlling_worker_with_a_fetch_handler_passes() {
    let mut host = FakeHost::default();
    host.serve(
        "https://a.test/app/sw.js",
        200,
        "text/javascript",
        b"self.addEventListener('fetch', e => e.respondWith(caches.match(e.request)));",
    );
    let (report, rules) = run(&mut host, &active());
    assert!(rules.is_empty(), "{rules:?}");
    assert!(report.active && report.controlled && report.registered);
    assert_eq!(report.fetch_handler, Some(true));
}

#[test]
fn each_missing_piece_is_its_own_finding() {
    let mut host = FakeHost::default();
    let insecure = state(json!({ "url": "http://a.test/", "secure": false }));
    assert_eq!(run(&mut host, &insecure).1, vec![SW_INSECURE]);
    let none = state(json!({ "url": PAGE, "secure": true, "sw": { "supported": true } }));
    assert_eq!(run(&mut host, &none).1, vec![SW_MISSING]);
    let waiting = state(
        json!({ "url": PAGE, "secure": true, "sw": { "supported": true,
        "registration": { "script": "https://a.test/app/sw.js", "state": "installed", "active": false } } }),
    );
    host.serve(
        "https://a.test/app/sw.js",
        200,
        "text/javascript",
        b"// nothing",
    );
    assert_eq!(
        run(&mut host, &waiting).1,
        vec![SW_NOT_ACTIVE, SW_FETCH_HANDLER]
    );
    let mut uncontrolled = active();
    uncontrolled.sw.controlled = false;
    assert_eq!(
        run(&mut host, &uncontrolled).1,
        vec![SW_NOT_CONTROLLING, SW_FETCH_HANDLER]
    );
}

#[test]
fn the_offline_test_reports_an_uncached_start_url() {
    let mut host = FakeHost::default();
    let worker = WorkerReport {
        active: true,
        ..WorkerReport::default()
    };
    let start = Url::parse(PAGE).expect("url");
    let mut sink = Sink::new(&[], PAGE, Some(1));
    host.offline = "false".to_owned();
    let report = offline(&mut host, 1, true, &worker, Some(&start), &mut sink);
    assert_eq!(report.cached, Some(false));
    assert_eq!(sink.findings[0].rule, OFFLINE_NOT_CACHED);
    let mut sink = Sink::new(&[], PAGE, Some(1));
    let report = offline(&mut host, 1, false, &worker, Some(&start), &mut sink);
    assert!(!report.tested && sink.findings.is_empty(), "switched off");
    host.offline = "null".to_owned();
    let report = offline(&mut host, 1, true, &worker, Some(&start), &mut sink);
    assert_eq!(report.cached, None);
    assert!(sink.findings.is_empty(), "no Cache Storage: no finding");
}

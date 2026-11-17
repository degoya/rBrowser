use serde_json::json;

use super::*;
use crate::fake::FakeHost;

const PAGE: &str = "https://a.test/app/";
const MANIFEST: &str = "https://a.test/app/manifest.webmanifest";

fn state(manifests: &[&str]) -> PageState {
    serde_json::from_value(json!({ "url": PAGE, "secure": true, "manifests": manifests }))
        .expect("state")
}

fn run(host: &mut FakeHost, state: &PageState) -> (Loaded, Vec<String>) {
    let page = Url::parse(PAGE).expect("url");
    let budget = Budget::new(host, 30_000);
    let mut sink = Sink::new(&[], PAGE, Some(1));
    let loaded = check(host, &budget, &page, state, &mut sink);
    let found = sink
        .findings
        .iter()
        .map(|f| {
            let params: Vec<String> = f.params.iter().map(|(k, v)| format!("{k}={v}")).collect();
            format!("{} {}", f.rule, params.join(" "))
        })
        .collect();
    (loaded, found)
}

fn complete() -> serde_json::Value {
    json!({
        "name": "App", "short_name": "App", "id": "/app/", "start_url": "./", "scope": "./",
        "display": "standalone", "theme_color": "#123456", "background_color": "#ffffff",
        "icons": [{ "src": "512.png", "sizes": "512x512", "purpose": "maskable" }]
    })
}

#[test]
fn a_complete_manifest_with_a_live_start_url_gives_no_finding() {
    let mut host = FakeHost::default();
    let text = complete().to_string();
    host.serve(MANIFEST, 200, "application/manifest+json", text.as_bytes())
        .serve(PAGE, 200, "text/html", b"<!doctype html>");
    let (loaded, found) = run(&mut host, &state(&[MANIFEST]));
    assert!(found.is_empty(), "{found:?}");
    let report = loaded.report.expect("report");
    assert_eq!(report.status, Some(200));
    assert_eq!(report.start_url.as_deref(), Some(PAGE));
    assert_eq!(report.start_url_status.as_deref(), Some("200"));
    assert!(report.table.iter().any(|(key, _)| key == "display"));
    assert!(loaded.manifest.is_some());
}

#[test]
fn no_link_and_an_unreachable_manifest_end_the_manifest_rules() {
    let mut host = FakeHost::default();
    assert_eq!(run(&mut host, &state(&[])).1, vec!["manifest.missing "]);
    let (loaded, found) = run(&mut host, &state(&["manifest.webmanifest"]));
    assert_eq!(
        found,
        vec![format!("manifest.unreachable url={MANIFEST} status=404")]
    );
    assert_eq!(loaded.report.expect("report").status, Some(404));
    let (_, found) = run(&mut host, &state(&["https://cdn.test/m.json"]));
    assert_eq!(
        found,
        vec!["fetch.other_origin what=manifest url=https://cdn.test/m.json"]
    );
}

#[test]
fn a_wrong_type_and_bad_json_are_findings() {
    let mut host = FakeHost::default();
    host.serve(MANIFEST, 200, "text/plain", b"{ nope");
    let (loaded, found) = run(&mut host, &state(&[MANIFEST]));
    assert_eq!(found.len(), 2);
    assert_eq!(found[0], "manifest.content_type type=text/plain");
    assert!(found[1].starts_with("manifest.invalid error="), "{found:?}");
    assert!(loaded.manifest.is_none());
    assert!(loaded.report.expect("report").error.is_some());
}

#[test]
fn missing_members_and_a_start_url_outside_its_scope_are_findings() {
    let mut host = FakeHost::default();
    let text = json!({
        "start_url": "/elsewhere/", "scope": "/app/", "display": "browser",
        "icons": [{ "src": "192.png", "sizes": "192x192" }]
    })
    .to_string();
    host.serve(
        MANIFEST,
        200,
        "application/json; charset=utf-8",
        text.as_bytes(),
    );
    let (_, found) = run(&mut host, &state(&[MANIFEST]));
    assert_eq!(
        found,
        vec![
            "manifest.name ".to_owned(),
            "manifest.display display=browser".to_owned(),
            "manifest.recommended field=maskable".to_owned(),
            "manifest.recommended field=theme_color".to_owned(),
            "manifest.recommended field=background_color".to_owned(),
            "manifest.recommended field=id".to_owned(),
            "start_url.out_of_scope url=https://a.test/elsewhere/ scope=https://a.test/app/"
                .to_owned(),
            "start_url.status url=https://a.test/elsewhere/ status=404".to_owned(),
        ]
    );
}

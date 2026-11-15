//! The fixtures over every view (RB-070-04): `tests/fixtures/site/grid.html` in three views and
//! the snippet checked in with `tests/fixtures/untrusted-project/`, run through the plugin's
//! native traits against `MemoryHost` - the host's rules for the capabilities, the CDP domains
//! the manifest grants, the project paths and the out folder.

use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, ViewInfo};
use serde_json::{Value, json};
use snippets_plugin::Snippets;

const MANIFEST: &str = include_str!("../manifest.toml");
const PAGE: &str = include_str!("../../../tests/fixtures/site/grid.html");
const PROJECT_SNIPPET: &str =
    include_str!("../../../tests/fixtures/untrusted-project/.rbrowser/snippets/title.js");
const TITLE: &str = "rBrowser grid fixture";

fn view(id: u32, width: u32) -> ViewInfo {
    ViewInfo {
        id,
        label: format!("View {width}"),
        breakpoint: format!("w{width}"),
        engine: "chromium".to_owned(),
        width,
        height: 800,
        dpr: 1.0,
        url: "http://127.0.0.1:8080/grid.html".to_owned(),
        title: TITLE.to_owned(),
    }
}

/// A host granting what the manifest grants; every CDP command answers one object that serves
/// the frame tree, the isolated world and the evaluation alike.
fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Cdp)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite)
        .with_file(".rbrowser/snippets/title.js", PROJECT_SNIPPET.as_bytes());
    host.cdp_domains = vec!["Runtime".to_owned(), "Page".to_owned()];
    host.views = vec![view(1, 375), view(2, 768), view(3, 1280)];
    host.active = Some(3);
    host.settings.insert(
        "library".to_owned(),
        json!([{ "name": "Title", "code": "return document.title" }]).to_string(),
    );
    host.cdp_answer = json!({
        "frameTree": { "frame": { "id": "MAIN" } },
        "executionContextId": 9,
        "result": { "type": "object", "value": {
            "ok": true, "type": "string", "text": format!("\"{TITLE}\""), "value": TITLE } },
    })
    .to_string();
    host
}

fn call(host: &mut MemoryHost, tool: &str, args: &Value) -> Value {
    let answer = Snippets
        .call_tool(host, tool, &args.to_string())
        .expect("answers");
    serde_json::from_str(&answer).expect("JSON")
}

#[test]
fn the_manifest_grants_the_domains_the_plugin_sends() {
    assert!(MANIFEST.contains(r#"cdp = { domains = ["Runtime", "Page"] }"#));
    assert!(PAGE.contains(&format!("<title>{TITLE}</title>")));
}

#[test]
fn a_user_snippet_gives_the_title_of_every_view_as_one_result() {
    let mut host = host();
    let answer = call(
        &mut host,
        "run",
        &json!({ "name": "Title", "target": "all" }),
    );
    assert_eq!(answer["results"].as_array().map(Vec::len), Some(1));
    assert_eq!(answer["results"][0]["value"], TITLE);
    assert_eq!(answer["results"][0]["views"], json!([1, 2, 3]));
    // Isolated by default: the plugin's own world in the main frame of each view.
    let methods: Vec<&str> = host.cdp_sent.iter().map(|sent| sent.1.as_str()).collect();
    assert_eq!(methods.len(), 9);
    assert_eq!(
        &methods[..3],
        [
            "Page.getFrameTree",
            "Page.createIsolatedWorld",
            "Runtime.evaluate"
        ]
    );
    assert!(
        host.out.contains_key("last.json"),
        "remembered for run_last"
    );
}

#[test]
fn the_checked_in_snippet_is_listed_and_runs_in_the_active_view() {
    let mut host = host();
    let listed = call(&mut host, "list", &json!({}));
    assert_eq!(listed["project"], "loaded");
    let project: Vec<&Value> = listed["snippets"]
        .as_array()
        .expect("list")
        .iter()
        .filter(|entry| entry["source"] == "project")
        .collect();
    assert_eq!(project.len(), 1);
    assert_eq!(project[0]["name"], "title");
    assert_eq!(project[0]["file"], "title.js");
    let answer = call(
        &mut host,
        "run",
        &json!({ "name": "title", "source": "project", "target": "active" }),
    );
    assert_eq!(answer["views"], json!([3]));
}

#[test]
fn a_main_world_snippet_runs_without_a_context() {
    let mut host = host();
    call(
        &mut host,
        "run",
        &json!({ "code": "// @world main\nreturn window.__last", "target": 1 }),
    );
    assert_eq!(host.cdp_sent.len(), 1);
    let params: Value = serde_json::from_str(&host.cdp_sent[0].2).expect("JSON");
    assert!(params.get("contextId").is_none());
    assert_eq!(params["timeout"], 5000);
}

#[test]
fn a_host_without_cdp_leaves_every_view_failed_but_answers() {
    let mut host = MemoryHost::new().grant(Capability::Views);
    host.views = vec![view(1, 375)];
    let answer = call(
        &mut host,
        "run",
        &json!({ "code": "return 1", "world": "main" }),
    );
    assert_eq!(answer["results"][0]["ok"], false);
    assert_eq!(
        answer["results"][0]["text"],
        "plugins.error.capability_denied"
    );
}

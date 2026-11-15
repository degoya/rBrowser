//! The fixture over the views (RB-070-01): `tests/fixtures/site/storage/` in three views - two in
//! the session `default`, one in `admin` - run through the plugin's `feature` export, against the
//! scripted browser (`support/browser.rs`) and against `MemoryHost`, which holds the host's rules
//! for the capabilities the manifest grants.

#[path = "support/browser.rs"]
mod browser;

use browser::{Browser, view};
use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, ViewId};
use serde_json::{Value, json};
use storage_panel::StoragePanel;

const INDEX: &str = include_str!("../../../tests/fixtures/site/storage/index.html");
const OTHER: &str = include_str!("../../../tests/fixtures/site/storage/other.html");
const MANIFEST: &str = include_str!("../manifest.toml");
const ORIGIN: &str = "http://127.0.0.1:18767";
const CONFIG: &str = r#"{"version":1,"startUrl":"http://127.0.0.1:18767/","sessions":["admin"],
  "breakpoints":[{"id":"mobile","width":375},{"id":"tablet","width":768},
  {"id":"desktop","width":1280,"session":"admin"}]}"#;

/// The text of the first `<script>` of a page.
fn first_script(page: &str) -> &str {
    let start = page.find("<script>").map_or(0, |at| at + "<script>".len());
    let end = page[start..]
        .find("</script>")
        .map_or(page.len(), |at| start + at);
    &page[start..end]
}

fn browser() -> Browser {
    let mut browser = Browser::default();
    browser
        .project
        .insert(".rbrowser/config.json".into(), CONFIG.as_bytes().to_vec());
    for (id, breakpoint, session) in [
        (1, "mobile", "default"),
        (2, "tablet", "default"),
        (3, "desktop", "admin"),
    ] {
        browser.add_view(
            view(id, breakpoint, "chromium", &format!("{ORIGIN}/")),
            session,
        );
        // server.py: the HttpOnly session cookie of each browser session.
        browser.set_cookie(
            id,
            json!({ "name": "session_id", "value": format!("s{session}"), "domain": "127.0.0.1",
                    "path": "/", "httpOnly": true }),
        );
    }
    // The person accepts the cookies in a default view (the fixture's button).
    browser.set_cookie(
        1,
        json!({ "name": "consent", "value": "yes", "domain": "127.0.0.1", "path": "/",
                "sameSite": "Lax" }),
    );
    browser
}

fn panel(browser: &mut Browser, args: &Value) -> Value {
    let answer = StoragePanel
        .run_command(browser, "panel", &args.to_string())
        .expect("the panel's call answers");
    serde_json::from_str(&answer).expect("JSON")
}

/// A document start: page/pins.js, the fixture's first script, then the plugin's answer.
fn load(browser: &mut Browser, view: ViewId) -> Option<String> {
    browser.run_page_script(view);
    let logged = browser.local_item(view, "featureFlag");
    browser.set_local(view, "featureFlag", "off");
    let message = json!({ "view": view, "message": { "type": "loaded", "origin": ORIGIN } });
    let answer = StoragePanel
        .call_tool(browser, "@page", &message.to_string())
        .expect("answers");
    let answer: Value = serde_json::from_str(&answer).expect("JSON");
    browser.take_page_answer(view, &answer["record"]);
    logged
}

#[test]
fn the_fixture_logs_the_flag_in_its_first_script_on_both_pages() {
    for page in [INDEX, OTHER] {
        let script = first_script(page);
        assert!(script.contains("localStorage.getItem('featureFlag')"));
        assert!(script.contains("console.log"));
    }
}

#[test]
fn consent_is_in_default_and_missing_in_admin_while_the_httponly_cookie_is_everywhere() {
    let mut browser = browser();
    let list = panel(&mut browser, &json!({ "op": "list", "kind": "cookies" }));
    let rows = list["rows"].as_array().expect("rows");
    let consent = rows
        .iter()
        .find(|r| r["name"] == "consent")
        .expect("consent");
    assert_eq!(consent["state"], "missing");
    assert!(consent["cells"]["1"].is_object() && consent["cells"]["2"].is_object());
    assert!(consent["cells"]["3"].is_null());
    let sid = rows
        .iter()
        .find(|r| r["name"] == "session_id")
        .expect("sid");
    assert_eq!(sid["state"], "differs", "each session has its own login");
    assert_eq!(sid["cells"]["3"]["httpOnly"], true);
}

#[test]
fn a_pinned_flag_is_set_before_the_first_script_after_reload_and_navigation() {
    let mut browser = browser();
    panel(
        &mut browser,
        &json!({ "op": "set", "kind": "local", "target": 2, "name": "featureFlag",
                 "value": "on" }),
    );
    panel(
        &mut browser,
        &json!({ "op": "pin", "kind": "local", "view": 2, "scope": ORIGIN,
                 "name": "featureFlag" }),
    );
    for view in [1, 2, 3] {
        assert_eq!(
            load(&mut browser, view).as_deref(),
            Some("on"),
            "reload {view}"
        );
        browser.navigate(view, &format!("{ORIGIN}/other.html"));
        assert_eq!(
            load(&mut browser, view).as_deref(),
            Some("on"),
            "other.html {view}"
        );
    }
}

#[test]
fn the_calls_keep_to_the_manifests_capabilities() {
    // The manifest grants views, evaluate, cdp (Network only), project-read and out-write.
    assert!(MANIFEST.contains(r#"cdp = { domains = ["Network"] }"#));
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Evaluate)
        .grant(Capability::Cdp)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite)
        .with_file(".rbrowser/config.json", CONFIG.as_bytes());
    host.cdp_domains = vec!["Network".to_owned()];
    host.views = vec![view(1, "mobile", "chromium", &format!("{ORIGIN}/"))];
    host.cdp_answer = json!({ "cookies": [{ "name": "consent", "value": "yes",
        "domain": "127.0.0.1", "path": "/" }] })
    .to_string();
    host.evaluate_answer =
        json!({ "origin": ORIGIN, "local": [["featureFlag", "on"]], "session": [], "more": 0 })
            .to_string();
    let answer = StoragePanel
        .call_tool(&mut host, "get", r#"{"target": "all"}"#)
        .expect("the host's rules let every call through");
    let answer: Value = serde_json::from_str(&answer).expect("JSON");
    assert_eq!(answer["views"][0]["cookies"][0]["name"], "consent");
    assert_eq!(answer["views"][0]["localStorage"][0]["key"], "featureFlag");
    assert!(
        host.cdp_sent
            .iter()
            .all(|(_, method, _)| method.starts_with("Network."))
    );

    host.settings.insert("mcpValues".into(), "true".into());
    StoragePanel
        .call_tool(
            &mut host,
            "set",
            r#"{"kind": "cookie", "name": "consent", "value": "no", "target": 1}"#,
        )
        .expect("a cookie write passes the CDP rule");
    let pin = json!({ "op": "pin", "kind": "local", "view": 1, "scope": ORIGIN,
                      "name": "featureFlag" });
    StoragePanel
        .run_command(&mut host, "panel", &pin.to_string())
        .expect("the pins file is written below the out folder");
    assert!(host.out.contains_key("pins.json"));
}

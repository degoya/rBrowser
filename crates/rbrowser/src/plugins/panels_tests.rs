use rb_plugin_host::parse_manifest;

use super::*;

const MANIFEST: &str = r#"
manifest_version = 1
id = "hello-panel"
name = "Hello panel"
version = "0.1.0"
api_version = "0.1.0"
kinds = ["feature-plugin"]
mcp_prefix = "hello"
key_id = "test-v1"
public_key = "AAAA"

[locales]
en = "locales/en.json"

[[panels]]
id = "main"
title = "plugin.hello-panel.panels.main"
entry = "panel/index.html"
icon = "hand"

[[panels]]
id = "ask"
title = "plugin.hello-panel.panels.ask"
entry = "panel/ask.html"
placement = "dialog"
"#;

fn manifest() -> PluginManifest {
    parse_manifest(MANIFEST.as_bytes()).expect("a valid manifest")
}

fn files() -> BTreeMap<String, Vec<u8>> {
    BTreeMap::from([
        ("page/hello.js".to_owned(), b"1;".to_vec()),
        ("panel/index.html".to_owned(), b"<p>hi</p>".to_vec()),
        ("panel/app.js".to_owned(), b"2;".to_vec()),
    ])
}

#[test]
fn every_panel_is_offered_with_its_title_placement_icon_and_url() {
    let manifest = manifest();
    let panels = panels_of([&manifest], &BTreeSet::new(), |key| format!("<{key}>"));
    assert_eq!(
        panels,
        vec![
            PanelInfo {
                plugin: "hello-panel".to_owned(),
                panel: "main".to_owned(),
                title: "<plugin.hello-panel.panels.main>".to_owned(),
                placement: PanelPlacement::Sidebar,
                icon: Some("hand".to_owned()),
                url: "rbrowser://plugin/hello-panel/panel/index.html".to_owned(),
            },
            PanelInfo {
                plugin: "hello-panel".to_owned(),
                panel: "ask".to_owned(),
                title: "<plugin.hello-panel.panels.ask>".to_owned(),
                placement: PanelPlacement::Dialog,
                icon: None,
                url: "rbrowser://plugin/hello-panel/panel/ask.html".to_owned(),
            },
        ]
    );
    // The shell reads `icon` only when a manifest gave one.
    let rows = serde_json::to_value(&panels).expect("serialises");
    assert_eq!(rows[0]["icon"], "hand");
    assert!(rows[1].get("icon").is_none());
    let hidden = BTreeSet::from(["hello-panel".to_owned()]);
    assert!(panels_of([&manifest], &hidden, str::to_owned).is_empty());
}

#[test]
fn only_panel_files_are_served_and_html_gets_the_csp_first() {
    let manifest = manifest();
    let files = files();
    let served = panel_files([(&manifest, &files)]);
    let urls: Vec<&str> = served.keys().map(String::as_str).collect();
    assert_eq!(
        urls,
        [
            "rbrowser://plugin/hello-panel/panel/app.js",
            "rbrowser://plugin/hello-panel/panel/index.html",
        ],
        "never a page script"
    );
    let html = &served["rbrowser://plugin/hello-panel/panel/index.html"];
    assert_eq!(html.mime, "text/html");
    let text = String::from_utf8(html.bytes.clone()).expect("UTF-8");
    assert!(text.starts_with("<meta http-equiv=\"Content-Security-Policy\" content=\""));
    assert!(text.ends_with("<p>hi</p>"));
    let script = &served["rbrowser://plugin/hello-panel/panel/app.js"];
    assert_eq!(script.mime, "text/javascript");
    assert_eq!(script.bytes, b"2;");
}

#[test]
fn the_csp_allows_no_network_and_the_header_sandboxes() {
    for directive in [
        "default-src 'none'",
        "connect-src 'none'",
        "frame-src 'none'",
        "worker-src 'none'",
        "form-action 'none'",
        "base-uri 'none'",
    ] {
        assert!(PANEL_CSP.contains(directive), "{directive}");
        assert!(PANEL_CSP_HEADER.contains(directive), "{directive}");
    }
    assert!(!PANEL_CSP.contains("http"));
    assert!(PANEL_CSP_HEADER.starts_with(PANEL_CSP));
    assert!(PANEL_CSP_HEADER.ends_with("; sandbox allow-scripts"));
}

#[test]
fn panel_requests_parse_and_unknown_ones_are_refused() {
    let list: PanelRequest = serde_json::from_value(json!({ "op": "list" })).expect("list");
    assert_eq!(list, PanelRequest::List);
    let call: PanelRequest = serde_json::from_value(json!({
        "op": "call", "plugin": "hello-panel", "tool": "greet", "args": { "name": "x" },
    }))
    .expect("call");
    assert_eq!(
        call,
        PanelRequest::Call {
            plugin: "hello-panel".to_owned(),
            command: None,
            tool: Some("greet".to_owned()),
            args: json!({ "name": "x" }),
        }
    );
    assert!(serde_json::from_value::<PanelRequest>(json!({ "op": "install" })).is_err());
    // Settings > Plugins reads the list; there is no request that installs or trusts (RB-080-51).
    let installed: PanelRequest =
        serde_json::from_value(json!({ "op": "installed" })).expect("installed");
    assert_eq!(installed, PanelRequest::Installed);
    for op in ["enable", "disable", "trust", "trustKey", "remove"] {
        assert!(serde_json::from_value::<PanelRequest>(json!({ "op": op })).is_err());
    }
}

#[test]
fn a_call_names_one_command_or_tool_and_stays_within_the_limit() {
    assert_eq!(
        checked_call(Some("greet".to_owned()), None, &json!({})),
        Ok(Call::Command("greet".to_owned()))
    );
    assert_eq!(
        checked_call(None, Some("greet".to_owned()), &Value::Null),
        Ok(Call::Tool("greet".to_owned()))
    );
    assert!(checked_call(None, None, &json!({})).is_err());
    assert!(checked_call(Some("a".to_owned()), Some("b".to_owned()), &json!({})).is_err());
    let large = json!("x".repeat(MAX_MESSAGE_BYTES));
    assert_eq!(
        checked_call(Some("a".to_owned()), None, &large),
        Err("plugins.error.too_large".to_owned())
    );
}

#[test]
fn an_event_names_its_plugin() {
    assert_eq!(
        event_message("hello-panel", json!({ "view": 2, "message": "hi" })),
        json!({ "event": { "view": 2, "message": "hi", "plugin": "hello-panel" } })
    );
}

#[test]
fn only_the_panel_origin_is_resolved() {
    assert!(resource("rbrowser://ui/index.html").is_none());
    assert!(resource("https://plugin/x").is_none());
}

#[test]
fn a_page_names_the_panel_it_shows() {
    // RB-080-57: the docked panels move with the panel the page shows.
    let selected: PanelRequest = serde_json::from_value(
        json!({ "op": "selected", "plugin": "hello-panel", "panel": "log" }),
    )
    .expect("selected");
    assert_eq!(
        selected,
        PanelRequest::Selected {
            plugin: "hello-panel".to_owned(),
            panel: "log".to_owned()
        }
    );
    assert!(
        serde_json::from_value::<PanelRequest>(json!({ "op": "selected", "plugin": "x" })).is_err()
    );
}

// RB-080-62 review: the panels follow the app's theme, not only the system's - the settings a
// panel gets carry `ui.theme` beside the plugin's own keys.
#[test]
fn a_panels_settings_carry_the_apps_theme() {
    let own = json!({ "fonts.allowedHosts": [] });
    assert_eq!(
        with_theme(own.clone(), rb_config::Theme::Dark),
        json!({ "fonts.allowedHosts": [], "ui.theme": "dark" })
    );
    assert_eq!(
        with_theme(own, rb_config::Theme::System)["ui.theme"],
        json!("system")
    );
    assert_eq!(
        with_theme(json!(null), rb_config::Theme::Light),
        json!({ "ui.theme": "light" })
    );
}

#[test]
fn a_page_names_the_core_panel_it_shows() {
    // RB-080-69: Findings in the panels page move with the panels too.
    let selected: PanelRequest =
        serde_json::from_value(json!({ "op": "selectedCore", "panel": "findings" }))
            .expect("selectedCore");
    assert_eq!(
        selected,
        PanelRequest::SelectedCore {
            panel: "findings".to_owned()
        }
    );
    assert!(serde_json::from_value::<PanelRequest>(json!({ "op": "selectedCore" })).is_err());
}

//! The fixture (RB-070-02): `tests/fixtures/site/network/` - its mock file as the project's
//! `.rbrowser/mocks/api.json` and its `srcset` page as the host captures it in a narrow and a
//! wide view - run through the plugin's native traits against `MemoryHost`, the host's rules
//! for the capabilities and the project paths.

use network_panel_plugin::{NetworkPanel, RULES_TOOL};
use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, ViewInfo};
use serde_json::{Value, json};

const MOCKS: &str = include_str!("../../../tests/fixtures/site/network/mocks/api.json");
const PAGE: &str = "http://127.0.0.1:8080/network/index.html";

fn view(id: u32, label: &str, width: u32) -> ViewInfo {
    ViewInfo {
        id,
        label: label.to_owned(),
        breakpoint: label.to_lowercase(),
        engine: "chromium".to_owned(),
        width,
        height: 800,
        dpr: 1.0,
        url: PAGE.to_owned(),
        title: "Network fixture".to_owned(),
    }
}

fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite)
        .with_file(".rbrowser/mocks/api.json", MOCKS.as_bytes());
    host.views = vec![view(1, "Mobile", 390), view(2, "Desktop", 1600)];
    host.now = 1_731_300_000;
    host
}

fn call(host: &mut MemoryHost, tool: &str, args: &Value) -> Value {
    let answer = NetworkPanel
        .call_tool(host, tool, &args.to_string())
        .expect("answers");
    serde_json::from_str(&answer).expect("JSON")
}

#[test]
fn the_fixture_mocks_become_host_rules_under_trust_only() {
    let mut host = host();
    let trusted = json!({ "$network": { "session": "s1", "trusted": true } });
    let rules = call(&mut host, RULES_TOOL, &trusted);
    let ids: Vec<&str> = rules["rules"]
        .as_array()
        .expect("rules")
        .iter()
        .filter_map(|rule| rule["id"].as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "api.json#user-500",
            "api.json#no-tracking",
            "api.json#fixture-header"
        ]
    );
    assert_eq!(rules["rules"][0]["action"]["status"], 500);
    assert_eq!(rules["rules"][0]["action"]["delayMs"], 800);
    assert_eq!(rules["rules"][1]["action"]["type"], "block");
    assert_eq!(
        rules["rules"][2]["action"]["headers"],
        json!([["x-fixture", "rewritten"]])
    );
    let untrusted = json!({ "$network": { "session": "s1", "trusted": false } });
    assert_eq!(call(&mut host, RULES_TOOL, &untrusted)["rules"], json!([]));
    let overview = call(&mut host, "mocks", &untrusted);
    assert_eq!(overview["files"][0]["path"], ".rbrowser/mocks/api.json");
    assert_eq!(overview["files"][0]["blocked"], true);
}

#[test]
fn the_srcset_images_are_marked_as_loaded_in_one_view_only() {
    let mut host = host();
    let entry = |id: &str, path: &str, kind: &str, size: f64| {
        json!({ "id": id, "url": format!("http://127.0.0.1:8080/network/{path}"), "method": "GET",
                "resourceType": kind, "status": 200, "size": size, "time": 1.0, "fromCache": false })
    };
    let args = json!({
        "$network": { "session": "s1", "trusted": true },
        "$console": { "views": [
            { "view": 1, "network": [entry("1", "index.html", "Document", 2000.0),
                                     entry("2", "img/hero-480.svg", "Image", 200.0)] },
            { "view": 2, "network": [entry("3", "index.html", "Document", 2000.0),
                                     entry("4", "img/hero-1600.svg", "Image", 210.0)] },
        ] },
        "differences": true,
    });
    let list = call(&mut host, "list", &args);
    let marked: Vec<(&str, bool)> = list["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter_map(|row| {
            Some((
                row["url"].as_str()?.rsplit('/').next()?,
                row["partial"].as_bool()?,
            ))
        })
        .collect();
    assert_eq!(marked, [("hero-480.svg", true), ("hero-1600.svg", true)]);
}

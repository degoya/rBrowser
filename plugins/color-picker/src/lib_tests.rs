use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, ViewInfo};

use crate::pixels::tests::png_base64;
use serde_json::{Value, json};

use super::*;

const MANIFEST: &str = include_str!("../manifest.toml");

#[test]
fn pick_shows_the_pipette_in_every_view() {
    let mut host = MemoryHost::default();
    let answer = ColorPicker
        .run_command(&mut host, "pick", "{}")
        .expect("pick");
    let answer: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(answer["$post"][0]["target"], "all");
    assert_eq!(answer["$post"][0]["message"]["type"], "pick");
    assert_eq!(answer["$post"][0]["message"]["config"]["loupeSize"], 11);
    // RB-090-64: the pipette's layer takes the focus; the page script names it by this text.
    assert_eq!(answer["$post"][0]["message"]["label"], "Colour picker");
}

#[test]
fn the_layer_name_is_the_plugin_name_of_the_catalogue() {
    let locales: Value = serde_json::from_str(include_str!("../locales/en.json")).expect("locales");
    assert_eq!(layer_name(), locales["plugin"]["color-picker"]["name"]);
}

fn view(id: u32, engine: &str) -> ViewInfo {
    ViewInfo {
        id,
        label: format!("View {id}"),
        breakpoint: "bp".to_owned(),
        engine: engine.to_owned(),
        width: 800,
        height: 600,
        dpr: 1.0,
        url: "http://localhost/".to_owned(),
        title: String::new(),
    }
}

#[test]
fn pick_names_the_active_view_the_only_one_to_show_the_loupe_at_once() {
    let mut host = MemoryHost::new().grant(Capability::Views);
    host.views = vec![view(3, "chromium")];
    host.active = Some(3);
    let answer = ColorPicker
        .run_command(&mut host, "pick", "{}")
        .expect("pick");
    let answer: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(answer["$post"][0]["target"], 3);
    assert_eq!(answer["$post"][0]["message"]["active"], 3);
    // Without an active view (or the capability) no view shows it before the pointer comes.
    let answer = ColorPicker
        .run_command(&mut MemoryHost::default(), "pick", "{}")
        .expect("pick");
    let answer: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(answer["$post"][0]["message"]["active"], Value::Null);
}

#[test]
fn pick_skips_the_views_it_cannot_read() {
    // RB-080-77, diag9: view 1 was Firefox; its page script asked for a window image at every
    // rest and the log had `view 1 failed: plugins.error.not_available` 33 times.
    let mut host = MemoryHost::new().grant(Capability::Views);
    host.views = vec![
        view(1, "firefox"),
        view(2, "chromium"),
        view(3, "webkit"),
        view(4, "chromium"),
    ];
    let answer = ColorPicker
        .run_command(&mut host, "pick", "{}")
        .expect("pick");
    let answer: Value = serde_json::from_str(&answer).expect("json");
    let targets: Vec<&Value> = answer["$post"]
        .as_array()
        .expect("posts")
        .iter()
        .filter(|post| post["message"]["type"] == "pick")
        .map(|post| &post["target"])
        .collect();
    assert_eq!(targets, [&json!(2), &json!(4)]);
}

fn pick_answer(args: &str) -> Value {
    let answer = ColorPicker
        .run_command(&mut MemoryHost::default(), "pick", args)
        .expect("pick");
    serde_json::from_str(&answer).expect("json")
}

#[test]
fn pick_tells_the_panel_and_the_status_bar_however_it_was_started() {
    // RB-080-64: the panel showed nothing until a colour was taken; a shortcut runs the same
    // command, so the panel learns of it from the answer's posts, not from its own click.
    let answer = pick_answer("{}");
    assert_eq!(
        answer["$post"][1],
        json!({ "target": "panel", "message": { "type": "picking" } })
    );
    assert_eq!(answer["$post"][2]["target"], "status");
    assert_eq!(
        answer["$post"][2]["message"]["key"],
        "plugin.color-picker.notice.picking"
    );
    assert_eq!(answer["$post"][2]["message"]["level"], "info");
}

#[test]
fn pick_with_stop_ends_the_pipette_everywhere() {
    let answer = pick_answer(r#"{"stop":true}"#);
    let posts = answer["$post"].as_array().expect("posts");
    assert_eq!(
        posts[0],
        json!({ "target": "all", "message": { "type": "stop" } })
    );
    assert_eq!(
        posts[1],
        json!({ "target": "panel", "message": { "type": "stopped" } })
    );
    assert_eq!(
        posts[2]["message"]["key"],
        "plugin.color-picker.notice.stopped"
    );
    // No pipette starts.
    assert!(posts.iter().all(|post| post["message"]["type"] != "pick"));
}

#[test]
fn every_notice_is_a_text_of_the_plugin() {
    let locales: Value = serde_json::from_str(include_str!("../locales/en.json")).expect("locales");
    for name in [
        "picking",
        "picked",
        "stopped",
        "unreadable",
        "unreadable_size",
        "unreadable_image",
    ] {
        let text = &locales["plugin"]["color-picker"]["notice"][name];
        assert!(text.as_str().is_some_and(|text| !text.is_empty()), "{name}");
    }
    let text = locales["plugin"]["color-picker"]["notice"]["picked"]
        .as_str()
        .unwrap_or_default();
    assert!(text.contains("{value}"));
}

#[test]
fn no_notice_names_the_plugin_the_status_bar_names_it_already() {
    // RB-080-76: the core shows a notice as "{plugin}: {text}"; "Colour picker: picked ..." came
    // out as "Colour picker: Colour picker: picked ...".
    let locales: Value = serde_json::from_str(include_str!("../locales/en.json")).expect("locales");
    let own = &locales["plugin"]["color-picker"];
    let name = own["name"].as_str().expect("name").to_lowercase();
    for (key, text) in own["notice"].as_object().expect("notices") {
        let text = text.as_str().unwrap_or_default().to_lowercase();
        assert!(!text.starts_with(&name), "notice.{key}: {text}");
    }
}

#[test]
fn the_panel_command_opens_the_panel() {
    let answer = ColorPicker
        .run_command(&mut MemoryHost::default(), "panel", "")
        .expect("panel");
    assert_eq!(
        answer,
        json!({ "$open": { "panel": "picker" } }).to_string()
    );
}

#[test]
fn unknown_names_and_odd_arguments_are_refused() {
    let mut host = MemoryHost::default();
    let unknown = ColorPicker
        .run_command(&mut host, "paint", "{}")
        .expect_err("unknown");
    assert_eq!(
        unknown.code.as_deref(),
        Some("plugins.error.unknown_contribution")
    );
    let tool = ColorPicker
        .call_tool(&mut host, "fill", "{}")
        .expect_err("unknown");
    assert_eq!(
        tool.code.as_deref(),
        Some("plugins.error.unknown_contribution")
    );
    let odd = ColorPicker
        .call_tool(&mut host, "contrast", "[1]")
        .expect_err("odd");
    assert_eq!(odd.code.as_deref(), Some("plugins.error.invalid"));
}

#[test]
fn contrast_runs_without_any_capability() {
    let answer = ColorPicker
        .call_tool(
            &mut MemoryHost::default(),
            "contrast",
            r##"{"foreground":"#767676","background":"#ffffff"}"##,
        )
        .expect("contrast");
    let answer: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(answer["ratio"], 4.54);
}

#[test]
fn the_manifest_grants_what_the_plugin_reaches_and_nothing_else() {
    // The window image without a clip is a permission of its own (review S2 of RB-080-64).
    assert!(MANIFEST.contains(r#"cdp = { domains = ["Page"], window_image = true }"#));
    assert!(MANIFEST.contains("mcp_prefix = \"color\""));
    for capability in ["http", "project-read", "out-write", "findings", "sidecar"] {
        assert!(
            !MANIFEST.contains(&format!("\n{capability} =")),
            "{capability}"
        );
    }
    // The host's rules for exactly these grants let `color_sample` through.
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Evaluate)
        .grant(Capability::Cdp);
    host.cdp_domains = vec!["Page".to_owned()];
    host.views.push(ViewInfo {
        id: 1,
        label: "Desktop".into(),
        breakpoint: "desktop".into(),
        engine: "chromium".into(),
        width: 1280,
        height: 800,
        dpr: 1.0,
        url: "http://127.0.0.1:18767/colors/".into(),
        title: String::new(),
    });
    host.active = Some(1);
    host.evaluate_answer =
        json!({ "width": 1280, "height": 800, "dpr": 1, "scrollX": 0, "scrollY": 0 }).to_string();
    // color_sample reads a clip of 11 CSS px around the point.
    host.cdp_answer = json!({ "data": png_base64(11, 11, |_, _| [0x1e, 0x88, 0xe5]) }).to_string();
    let answer = ColorPicker
        .call_tool(&mut host, "sample", r#"{"x":100,"y":100}"#)
        .expect("sample");
    let answer: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(answer["views"][0]["color"]["hex"], "#1e88e5");
    assert_eq!(host.cdp_sent[0].1, "Page.captureScreenshot");
}

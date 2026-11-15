use super::*;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, ViewInfo};
use serde_json::json;

use crate::image::tests::png;
use crate::store::{STATE_FILE, STATE_PATH};

fn view(id: u32, breakpoint: &str, width: u32) -> ViewInfo {
    ViewInfo {
        id,
        label: format!("{breakpoint} view"),
        breakpoint: breakpoint.to_owned(),
        engine: "chromium".to_owned(),
        width,
        height: 800,
        dpr: 1.0,
        url: "https://a.test/".to_owned(),
        title: String::new(),
    }
}

fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite)
        .with_file(".rbrowser/designs/mobile.png", &png(393, 852))
        .with_file(".rbrowser/designs/desktop.png", &png(1440, 900));
    host.views = vec![
        view(1, "mobile", 375),
        view(2, "tablet", 768),
        view(3, "desktop", 1440),
    ];
    host.settings.insert(
        "designs".into(),
        r#"{"mobile": "designs/mobile.png", "desktop": "designs/desktop.png"}"#.into(),
    );
    host
}

fn set(host: &mut MemoryHost, args: Value) -> Result<Value, Failure> {
    run(host, &args.to_string()).map(|answer| serde_json::from_str(&answer).expect("json"))
}

/// What the next call reads: the file the last one wrote.
fn persist(host: &mut MemoryHost) {
    if let Some(bytes) = host.out.get(STATE_FILE).cloned() {
        host.project.insert(STATE_PATH.to_owned(), bytes);
    }
}

#[test]
fn the_acceptance_call_reaches_every_view() {
    let mut host = host();
    let answer = set(
        &mut host,
        json!({ "enabled": true, "opacity": 50, "blend": "difference", "target": "all" }),
    )
    .expect("set");
    assert_eq!(answer["state"]["enabled"], json!(true));
    assert_eq!(answer["state"]["opacity"], json!(50));
    assert_eq!(answer["state"]["blend"], json!("difference"));
    assert_eq!(answer["saved"], json!(true));
    let posts = answer["$post"].as_array().expect("posts");
    assert_eq!(posts[0]["target"], json!("all"));
    assert_eq!(posts[0]["message"]["state"]["blend"], json!("difference"));
    assert_eq!(posts[0]["message"]["reload"], json!(true));
    assert_eq!(posts[1]["target"], json!("panel"));
    let views = answer["views"].as_array().expect("views");
    let paths: Vec<&Value> = views.iter().map(|view| &view["image"]["path"]).collect();
    assert_eq!(
        paths,
        [
            json!("designs/mobile.png"),
            json!("designs/mobile.png"),
            json!("designs/desktop.png")
        ]
        .iter()
        .collect::<Vec<_>>()
    );
    assert_eq!(views[1]["label"], json!("tablet view"));
    assert_eq!(answer["designs"], json!(["desktop.png", "mobile.png"]));
}

#[test]
fn no_argument_answers_the_state_without_writing_or_posting() {
    let mut host = host();
    let answer = set(&mut host, json!({})).expect("state");
    assert_eq!(answer["state"]["enabled"], json!(false));
    assert!(answer.get("$post").is_none());
    assert!(answer.get("saved").is_none());
    assert!(host.out.is_empty());
}

#[test]
fn a_view_target_answers_that_views_design_only_but_the_state_is_everyones() {
    let mut host = host();
    let answer = set(&mut host, json!({ "invert": true, "target": 3 })).expect("set");
    let views = answer["views"].as_array().expect("views");
    assert_eq!(views.len(), 1);
    assert_eq!(views[0]["view"], json!(3));
    assert_eq!(answer["$post"][0]["target"], json!("all"));
    let refused = set(&mut host, json!({ "target": "left" })).expect_err("target");
    assert_eq!(
        refused.code.as_deref(),
        Some("plugin.image-overlay.errors.bad_args")
    );
}

#[test]
fn offset_split_and_anchor_change_in_parts() {
    let mut host = host();
    set(&mut host, json!({ "offset": { "x": 12, "y": -4 } })).expect("offset");
    persist(&mut host);
    let answer = set(
        &mut host,
        json!({ "offset": { "y": 8 }, "split": 30, "anchor": "viewport" }),
    )
    .expect("more");
    assert_eq!(answer["state"]["offset"], json!({ "x": 12.0, "y": 8.0 }));
    assert_eq!(
        answer["state"]["split"],
        json!({ "on": true, "position": 30.0 })
    );
    assert_eq!(answer["state"]["anchor"], json!("viewport"));
    assert_eq!(answer["$post"][0]["message"]["reload"], json!(false));
    persist(&mut host);
    let off = set(&mut host, json!({ "split": false })).expect("off");
    assert_eq!(
        off["state"]["split"],
        json!({ "on": false, "position": 30.0 })
    );
}

#[test]
fn values_out_of_range_are_refused() {
    let mut host = host();
    for args in [
        json!({ "opacity": 101 }),
        json!({ "opacity": -1 }),
        json!({ "blend": "darken" }),
        json!({ "offset": { "x": 1e9 } }),
        json!({ "split": 120 }),
        json!({ "zoom": 2 }),
        json!({ "designs": { "a b": "designs/x.png" } }),
    ] {
        let refused = set(&mut host, args.clone()).expect_err("refused");
        assert_eq!(
            refused.code.as_deref(),
            Some("plugin.image-overlay.errors.bad_args"),
            "{args}"
        );
    }
    assert!(host.out.is_empty());
}

#[test]
fn a_design_outside_the_folder_is_refused_with_its_message() {
    let mut host = host();
    let refused = set(
        &mut host,
        json!({ "designs": { "mobile": "../config.json" } }),
    )
    .expect_err("outside");
    assert_eq!(
        refused.code.as_deref(),
        Some("plugin.image-overlay.errors.path_outside")
    );
    assert_eq!(
        refused.params.get("path").map(String::as_str),
        Some("../config.json")
    );
    assert!(host.out.is_empty());
}

#[test]
fn a_design_per_breakpoint_is_chosen_and_null_returns_to_the_nearest() {
    let mut host = host();
    let answer = set(
        &mut host,
        json!({ "designs": { "tablet": "designs/desktop.png" } }),
    )
    .expect("tablet");
    assert_eq!(
        answer["views"][1]["image"]["path"],
        json!("designs/desktop.png")
    );
    assert_eq!(answer["$post"][0]["message"]["reload"], json!(true));
    persist(&mut host);
    let back = set(&mut host, json!({ "designs": { "mobile": null } })).expect("nearest");
    assert_eq!(
        back["state"]["designs"],
        json!({ "desktop": "designs/desktop.png", "tablet": "designs/desktop.png" })
    );
    assert_eq!(
        back["views"][0]["image"]["path"],
        json!("designs/mobile.png")
    );
}

#[test]
fn reset_drops_the_overrides() {
    let mut host = host();
    set(&mut host, json!({ "enabled": true, "opacity": 10 })).expect("set");
    persist(&mut host);
    let answer = set(&mut host, json!({ "reset": true })).expect("reset");
    assert_eq!(answer["state"]["enabled"], json!(false));
    assert_eq!(answer["state"]["opacity"], json!(50));
}

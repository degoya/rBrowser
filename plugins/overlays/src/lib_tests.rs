use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, ViewInfo};
use serde_json::{Value, json};

use super::*;
use crate::state::{STATE_FILE, STATE_PATH};

fn view(id: u32, width: u32) -> ViewInfo {
    ViewInfo {
        id,
        label: format!("{width}px"),
        breakpoint: format!("bp{id}"),
        engine: "chromium".to_owned(),
        width,
        height: 800,
        dpr: 1.0,
        url: "http://localhost/overflow.html".to_owned(),
        title: "Overflow".to_owned(),
    }
}

fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Evaluate)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite);
    host.views = vec![view(1, 375), view(2, 1440)];
    host
}

/// What `out.write` wrote becomes readable again, as the real project folder does.
fn sync(host: &mut MemoryHost) {
    if let Some(bytes) = host.out.get(STATE_FILE).cloned() {
        host.project.insert(STATE_PATH.to_owned(), bytes);
    }
}

fn command(host: &mut MemoryHost, command: &str, args: Value) -> Result<Value, Failure> {
    let answer = Overlays.run_command(host, command, &args.to_string());
    sync(host);
    answer.map(|text| serde_json::from_str(&text).expect("JSON"))
}

fn tool(host: &mut MemoryHost, tool: &str, args: Value) -> Result<Value, Failure> {
    let answer = Overlays.call_tool(host, tool, &args.to_string());
    sync(host);
    answer.map(|text| serde_json::from_str(&text).expect("JSON"))
}

#[test]
fn a_command_toggles_its_overlay_and_posts_the_state_to_pages_and_panel() {
    let mut host = host();
    let on = command(&mut host, "overflow", json!({})).expect("on");
    assert_eq!(on["overlays"]["overflow"]["enabled"], json!(true));
    let posts = on["$post"].as_array().expect("posts");
    assert_eq!(posts[0]["target"], json!("all"));
    assert_eq!(posts[1]["target"], json!("panel"));
    let message = &posts[0]["message"];
    assert_eq!(message["type"], json!("state"));
    assert_eq!(message["state"]["all"]["overflow"]["enabled"], json!(true));
    assert_eq!(message["settings"]["mutationThrottleMs"], json!(250));
    let off = command(&mut host, "overflow", json!({})).expect("off");
    assert_eq!(off["overlays"]["overflow"]["enabled"], json!(false));
}

#[test]
fn outlines_take_their_option_from_the_command() {
    let mut host = host();
    let on = command(&mut host, "outlines", json!({ "containersOnly": true })).expect("on");
    assert_eq!(
        on["overlays"]["outlines"],
        json!({ "enabled": true, "options": { "containersOnly": true } })
    );
}

#[test]
fn the_highlight_needs_a_selector_and_counts_its_matches_per_view() {
    let mut host = host();
    let refused = command(&mut host, "highlight", json!({})).expect_err("no selector");
    assert_eq!(refused.code(), Some("plugin.overlays.errors.no_selector"));

    host.evaluate_answer = "3".to_owned();
    let answer = tool(
        &mut host,
        "set",
        json!({ "overlay": "highlight", "options": { "selector": ".card \"x\"" } }),
    )
    .expect("on");
    assert_eq!(
        answer["matches"],
        json!([{ "view": 1, "count": 3 }, { "view": 2, "count": 3 }])
    );
    assert_eq!(
        answer["overlays"]["highlight"]["options"]["index"],
        json!(0)
    );
    let (_, script) = host.evaluated.last().expect("evaluated");
    assert!(
        script.contains(r#"querySelectorAll(".card \"x\"")"#),
        "{script}"
    );
}

#[test]
fn an_invalid_selector_is_refused_and_changes_nothing() {
    let mut host = host();
    host.evaluate_answer = "-1".to_owned();
    let refused = tool(
        &mut host,
        "set",
        json!({ "overlay": "highlight", "options": { "selector": "div[" } }),
    )
    .expect_err("invalid");
    assert_eq!(refused.code(), Some("plugin.overlays.errors.bad_selector"));
    assert!(host.out.is_empty());
}

#[test]
fn the_arrow_keys_step_through_the_matches() {
    let mut host = host();
    host.evaluate_answer = "4".to_owned();
    command(&mut host, "highlight", json!({ "selector": ".card" })).expect("on");
    let evaluated = host.evaluated.len();
    let next = command(&mut host, "highlight", json!({ "step": 1 })).expect("next");
    assert_eq!(next["overlays"]["highlight"]["options"]["index"], json!(1));
    let back = command(&mut host, "highlight", json!({ "step": -2 })).expect("back");
    assert_eq!(back["overlays"]["highlight"]["options"]["index"], json!(-1));
    assert_eq!(
        host.evaluated.len(),
        evaluated,
        "stepping counts nothing again"
    );
    let off = command(&mut host, "highlight", json!({})).expect("toggle");
    assert_eq!(off["overlays"]["highlight"]["enabled"], json!(false));
}

#[test]
fn a_view_as_target_sets_that_view_only() {
    let mut host = host();
    let answer = tool(
        &mut host,
        "set",
        json!({ "overlay": "outlines", "enabled": true, "target": 2 }),
    )
    .expect("set");
    assert_eq!(answer["overlays"]["outlines"]["enabled"], json!(true));
    let all = tool(&mut host, "state", json!({ "target": "all" })).expect("state");
    assert_eq!(all["overlays"]["outlines"]["enabled"], json!(false));
    let two = tool(&mut host, "state", json!({ "target": 2 })).expect("state");
    assert_eq!(two["overlays"]["outlines"]["enabled"], json!(true));
}

#[test]
fn off_and_all_disabled_switch_every_overlay_off() {
    let mut host = host();
    command(&mut host, "overflow", json!({})).expect("on");
    tool(
        &mut host,
        "set",
        json!({ "overlay": "outlines", "target": 1 }),
    )
    .expect("on");
    let off = command(&mut host, "off", json!({})).expect("off");
    for id in ["overflow", "outlines", "highlight"] {
        assert_eq!(off["overlays"][id]["enabled"], json!(false), "{id}");
    }
    let one = tool(&mut host, "state", json!({ "target": 1 })).expect("state");
    assert_eq!(one["overlays"]["outlines"]["enabled"], json!(false));

    command(&mut host, "overflow", json!({})).expect("on");
    let all = tool(
        &mut host,
        "set",
        json!({ "overlay": "all", "enabled": false }),
    )
    .expect("all off");
    assert_eq!(all["overlays"]["overflow"]["enabled"], json!(false));
    let refused = tool(
        &mut host,
        "set",
        json!({ "overlay": "all", "enabled": true }),
    )
    .expect_err("all on");
    assert_eq!(refused.code(), Some("plugin.overlays.errors.all_on"));
}

#[test]
fn unknown_overlays_tools_and_commands_are_refused() {
    let mut host = host();
    let overlay = tool(&mut host, "set", json!({ "overlay": "zindex" })).expect_err("unknown");
    assert_eq!(
        overlay.code(),
        Some("plugin.overlays.errors.unknown_overlay")
    );
    let option = tool(
        &mut host,
        "set",
        json!({ "overlay": "overflow", "options": { "colour": "red" } }),
    )
    .expect_err("unknown option");
    assert_eq!(option.code(), Some("plugin.overlays.errors.bad_option"));
    let args = tool(&mut host, "set", json!(["overflow"])).expect_err("not an object");
    assert_eq!(args.code(), Some(codes::INVALID));
    let unknown_tool = tool(&mut host, "zindex", json!({})).expect_err("tool");
    assert_eq!(unknown_tool.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    let unknown_command = command(&mut host, "zindex", json!({})).expect_err("command");
    assert_eq!(unknown_command.code(), Some(codes::UNKNOWN_CONTRIBUTION));
}

#[test]
fn overflow_measures_every_view_and_logs_the_causes() {
    let mut host = host();
    host.evaluate_answer = json!({
        "hasHorizontalScroll": true, "scrollWidth": 416, "clientWidth": 375, "innerWidth": 375,
        "causes": [{ "selector": "div.promo", "rect": { "x": 16, "y": 120, "width": 400, "height": 80 }, "overhang": 41 }],
        "vw100": []
    })
    .to_string();
    let answer = tool(&mut host, "overflow", json!({ "target": "all" })).expect("measured");
    let views = answer["views"].as_array().expect("views");
    assert_eq!(views.len(), 2);
    assert_eq!(views[0]["view"], json!(1));
    assert_eq!(views[0]["breakpoint"], json!("bp1"));
    assert_eq!(views[0]["hasHorizontalScroll"], json!(true));
    assert_eq!(views[0]["causes"][0]["selector"], json!("div.promo"));
    assert_eq!(views[0]["causes"][0]["overhang"], json!(41));
    assert_eq!(views[0]["causes"][0]["rect"]["width"], json!(400.0));
    assert!(host.evaluated[0].1.contains("show100vw: true"));
    assert!(host.logs.iter().any(|(level, line)| level == "warn"
        && line.starts_with("view 1: horizontal scroll")
        && line.contains("div.promo (+41 px)")));
}

#[test]
fn overflow_of_one_view_reports_a_failed_measurement_in_its_entry() {
    let mut host = host();
    host.evaluate_answer = "\"not a report\"".to_owned();
    let answer = tool(
        &mut host,
        "overflow",
        json!({ "target": 2, "show100vw": false }),
    )
    .expect("answered");
    assert_eq!(
        answer["views"],
        json!([{ "view": 2, "label": "1440px", "breakpoint": "bp2", "error": "plugins.error.bad_output" }])
    );
    assert!(host.evaluated[0].1.contains("show100vw: false"));

    let mut bare = MemoryHost::new().grant(Capability::Views);
    bare.views = vec![view(1, 375)];
    let answer = Overlays
        .call_tool(&mut bare, "overflow", "{}")
        .expect("answered");
    let answer: Value = serde_json::from_str(&answer).expect("JSON");
    assert_eq!(answer["views"][0]["error"], json!(codes::CAPABILITY_DENIED));
}

#[test]
fn a_page_says_hello_and_gets_the_state_and_the_settings() {
    let mut host = host();
    command(&mut host, "outlines", json!({})).expect("on");
    let hello = tool(
        &mut host,
        PAGE_TOOL,
        json!({ "view": 1, "message": { "type": "hello" } }),
    )
    .expect("hello");
    assert_eq!(hello["state"]["all"]["outlines"]["enabled"], json!(true));
    assert_eq!(hello["settings"]["colors"]["cause"], json!("#e5262b"));
    assert!(hello.get("$post").is_none());
}

#[test]
fn a_page_reports_its_overflow_into_the_log() {
    let mut host = host();
    let report = json!({ "hasHorizontalScroll": true, "scrollWidth": 416, "clientWidth": 375,
        "causes": [{ "selector": "div.promo", "overhang": 41 }] });
    let answer = tool(
        &mut host,
        PAGE_TOOL,
        json!({ "view": 1, "message": { "type": "overflow", "report": report } }),
    )
    .expect("logged");
    assert_eq!(answer, json!({}));
    assert!(
        host.logs
            .iter()
            .any(|(_, line)| line.contains("div.promo (+41 px)"))
    );
    tool(
        &mut host,
        PAGE_TOOL,
        json!({ "view": 1, "message": { "type": "overflow", "report": { "hasHorizontalScroll": false } } }),
    )
    .expect("logged");
    assert_eq!(
        host.logs.last().map(|(_, line)| line.as_str()),
        Some("view 1: no horizontal scroll")
    );
    let bad = tool(
        &mut host,
        PAGE_TOOL,
        json!({ "view": 1, "message": { "type": "eval" } }),
    )
    .expect_err("unknown message");
    assert_eq!(bad.code(), Some(codes::INVALID));
}

#[test]
fn a_start_forgets_single_views_and_without_restore_everything() {
    let mut host = host();
    command(&mut host, "overflow", json!({})).expect("on");
    tool(
        &mut host,
        "set",
        json!({ "overlay": "outlines", "target": 2 }),
    )
    .expect("on");
    Overlays.activate(&mut host).expect("activated");
    sync(&mut host);
    let two = tool(&mut host, "state", json!({ "target": 2 })).expect("state");
    assert_eq!(two["overlays"]["overflow"]["enabled"], json!(true));
    assert_eq!(two["overlays"]["outlines"]["enabled"], json!(false));

    host.settings
        .insert("restoreOnStart".to_owned(), "false".to_owned());
    Overlays.activate(&mut host).expect("activated");
    sync(&mut host);
    let all = tool(&mut host, "state", json!({})).expect("state");
    assert_eq!(all["overlays"]["overflow"]["enabled"], json!(false));
}

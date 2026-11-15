use super::*;
use crate::store::{STATE_FILE, STATE_PATH};
use rb_plugin_api::Capability;
use rb_plugin_api::test_support::MemoryHost;
use serde_json::json;

fn host() -> MemoryHost {
    MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite)
}

fn command(host: &mut MemoryHost, id: &str, args: &str) -> Value {
    let text = RulersGuides
        .run_command(host, id, args)
        .expect("the command runs");
    serde_json::from_str(&text).expect("JSON answer")
}

#[test]
fn each_command_switches_its_layer_and_posts_it_everywhere() {
    let mut host = host();
    let rulers = command(&mut host, "toggle", "{}");
    assert_eq!(rulers["state"]["visible"]["rulers"], true);
    assert_eq!(rulers["$post"][0]["target"], "all");
    assert_eq!(
        rulers["$post"][0]["message"]["state"]["visible"]["rulers"],
        true
    );

    let bytes = host.out.get(STATE_FILE).cloned().expect("written");
    let mut host = host.with_file(STATE_PATH, &bytes);
    let again = command(&mut host, "toggle", "");
    assert_eq!(again["state"]["visible"]["rulers"], false);

    let grid = command(&mut host, "grid", "{}");
    assert_eq!(grid["state"]["visible"]["baseline"], true);
    let columns = command(&mut host, "columns", r#"{"on": true}"#);
    assert_eq!(columns["state"]["visible"]["columns"], true);
    let columns = command(&mut host, "columns", r#"{"on": true}"#);
    assert_eq!(columns["state"]["visible"]["columns"], true);
}

#[test]
fn the_rulers_setting_is_the_default_the_command_toggles_from() {
    let mut host = host();
    host.settings
        .insert("visible".to_owned(), "true".to_owned());
    let answer = command(&mut host, "toggle", "{}");
    assert_eq!(answer["state"]["visible"]["rulers"], false);
}

#[test]
fn unknown_contributions_are_refused() {
    let mut host = host();
    let failure = RulersGuides
        .run_command(&mut host, "paint", "{}")
        .expect_err("unknown");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugins.error.unknown_contribution")
    );
    let failure = RulersGuides
        .call_tool(&mut host, "get", "{}")
        .expect_err("unknown");
    assert_eq!(failure.kind, FailureKind::NotFound);
    let failure = RulersGuides
        .run_command(&mut host, "toggle", "[")
        .expect_err("bad");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.rulers-guides.errors.bad_args")
    );
}

#[test]
fn the_tool_and_the_page_are_served_through_call_tool() {
    let mut host = host();
    let state = RulersGuides
        .call_tool(&mut host, SET_TOOL, "{}")
        .expect("state");
    assert!(state.contains("\"state\""));
    let page = RulersGuides
        .call_tool(
            &mut host,
            PAGE_TOOL,
            &json!({"view": 1, "message": {"op": "layout", "geometry": {"clientWidth": 375, "innerWidth": 375}}})
                .to_string(),
        )
        .expect("layout");
    assert!(page.contains("\"layout\""));
}

#[test]
fn a_full_state_post_fits_the_hosts_message_limit() {
    // 200 guides and 32 setups, posted with the overrides: under 64 KiB of JSON.
    let mut host = host();
    let guides: Vec<Value> = (0..200)
        .map(|index| json!({"axis": "x", "unit": "percent", "value": f64::from(index) / 2.0 + 0.123}))
        .collect();
    let mut setups = serde_json::Map::new();
    for index in 0..31 {
        setups.insert(
            format!("breakpoint-{index:02}"),
            json!({"preset": "bootstrap", "columns": 12, "gutter": 24.5, "margin": 12.5, "maxWidth": 1200.5, "align": "center"}),
        );
    }
    let answer = set::run(
        &mut host,
        &json!({"guides": guides, "columns": setups, "baseline": {"lineHeight": 8, "offset": 0}})
            .to_string(),
    )
    .expect("set");
    let answer: Value = serde_json::from_str(&answer).expect("JSON");
    for post in answer["$post"].as_array().expect("posts") {
        let size = post["message"].to_string().len();
        assert!(size < 64 * 1024, "a post of {size} bytes");
    }
}

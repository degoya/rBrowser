use rb_plugin_api::feature::Feature;
use serde_json::json;

use super::Simulators;
use crate::state::STATE_FILE;
use crate::testing::{host, json, with_saved};

#[test]
fn pick_opens_the_picker() {
    let mut host = host();
    let answer = Simulators
        .run_command(&mut host, "pick", "{}")
        .expect("pick runs");
    assert_eq!(json(&answer), json!({ "$open": { "panel": "picker" } }));
}

#[test]
fn set_and_the_page_reach_their_handlers_and_nothing_else_does() {
    let mut host = host();
    let answer = Simulators
        .call_tool(&mut host, "set", r#"{"type": "dimmed", "target": 1}"#)
        .expect("set runs");
    assert_eq!(json(&answer)["views"][0]["type"], "dimmed");
    let mut host = with_saved(crate::testing::host(), &host);
    let page = Simulators
        .call_tool(
            &mut host,
            "@page",
            r#"{"view": 1, "message": {"op": "init"}}"#,
        )
        .expect("page answers");
    assert_eq!(json(&page)["spec"]["filter"][0]["op"], "brightness");

    for failure in [
        Simulators.call_tool(&mut host, "state", "{}"),
        Simulators.run_command(&mut host, "set", "{}"),
    ] {
        assert_eq!(
            failure.expect_err("unknown").code.as_deref(),
            Some("plugins.error.unknown_contribution")
        );
    }
}

#[test]
fn a_start_forgets_the_last_session_unless_persist_is_on() {
    let mut first = host();
    Simulators
        .call_tool(&mut first, "set", r#"{"type": "dimmed", "target": 1}"#)
        .expect("set runs");

    let mut host = with_saved(crate::testing::host(), &first);
    Simulators.activate(&mut host).expect("activates");
    let cleared = json(&String::from_utf8(host.out[STATE_FILE].clone()).expect("UTF-8"));
    assert_eq!(cleared["views"], json!({}));

    let mut host = with_saved(crate::testing::host(), &first);
    host.settings
        .insert("persist".to_owned(), "true".to_owned());
    Simulators.activate(&mut host).expect("activates");
    assert!(host.out.is_empty());
}

#[test]
fn a_start_without_a_project_is_no_failure() {
    let mut host = rb_plugin_api::test_support::MemoryHost::new();
    Simulators.activate(&mut host).expect("activates");
    assert!(host.out.is_empty());
}

use serde_json::json;

use super::handle;
use crate::set;
use crate::testing::{emulated, host, json, with_saved};

fn init(host: &mut rb_plugin_api::test_support::MemoryHost, view: u32) -> serde_json::Value {
    let args = json!({ "view": view, "message": { "op": "init" } });
    json(&handle(host, &args.to_string()).expect("init answers"))
}

#[test]
fn a_reloaded_chromium_view_gets_its_native_emulation_again() {
    let mut first = host();
    set::run(&mut first, r#"{"type": "deuteranopia", "target": 2}"#).expect("set");
    let mut host = with_saved(host(), &first);
    let answer = init(&mut host, 2);
    assert_eq!(answer, json!({ "spec": null }));
    assert_eq!(emulated(&host), [(2, "deuteranopia".to_owned())]);
}

#[test]
fn a_reloaded_view_gets_its_drawing_again() {
    let mut first = host();
    set::run(&mut first, r#"{"type": "tritanomaly", "target": 1}"#).expect("set");
    let mut host = with_saved(host(), &first);
    let answer = init(&mut host, 1);
    assert_eq!(answer["spec"]["filter"][0]["op"], "matrix");
    assert!(emulated(&host).is_empty());
}

#[test]
fn a_view_without_a_choice_follows_the_default_setting() {
    let mut host = host();
    assert_eq!(init(&mut host, 1), json!({ "spec": null }));
    assert!(host.cdp_sent.is_empty());

    host.settings
        .insert("default".to_owned(), "\"glaucoma\"".to_owned());
    let answer = init(&mut host, 3);
    assert_eq!(answer["spec"]["overlay"]["kind"], "tunnel");
}

#[test]
fn an_unknown_request_is_refused() {
    let mut host = host();
    let args = json!({ "view": 1, "message": { "op": "paint" } });
    let failure = handle(&mut host, &args.to_string()).expect_err("refused");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.simulators.errors.bad_args")
    );
    let failure = handle(&mut host, "[]").expect_err("refused");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.simulators.errors.bad_args")
    );
}

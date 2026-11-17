use serde_json::{Value, json};

use super::run;
use crate::testing::{emulated, host, json, with_saved};

fn set(host: &mut rb_plugin_api::test_support::MemoryHost, args: Value) -> Value {
    json(&run(host, &args.to_string()).expect("set runs"))
}

fn refused(host: &mut rb_plugin_api::test_support::MemoryHost, args: Value) -> String {
    let failure = run(host, &args.to_string()).expect_err("refused");
    format!(
        "{} {}",
        failure.code.unwrap_or_default(),
        failure
            .params
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn view_entry(answer: &Value, view: u32) -> Value {
    answer["views"]
        .as_array()
        .and_then(|views| views.iter().find(|entry| entry["view"] == view))
        .cloned()
        .unwrap_or(Value::Null)
}

fn post_to(answer: &Value, target: Value) -> Option<Value> {
    answer["$post"]
        .as_array()?
        .iter()
        .find(|post| post["target"] == target)
        .map(|post| post["message"].clone())
}

#[test]
fn deuteranopia_in_a_chromium_view_is_chromiums_own_emulation() {
    let mut host = host();
    let answer = set(&mut host, json!({ "type": "deuteranopia", "target": 2 }));
    assert_eq!(emulated(&host), [(2, "deuteranopia".to_owned())]);
    let entry = view_entry(&answer, 2);
    assert_eq!(entry["type"], "deuteranopia");
    assert_eq!(entry["mode"], "native");
    assert_eq!(view_entry(&answer, 1)["mode"], "off");
    // The page script draws nothing over the native emulation.
    assert_eq!(
        post_to(&answer, json!(2)),
        Some(json!({ "type": "apply", "spec": null }))
    );
    assert!(host.logs.iter().any(|(level, line)| level == "info"
        && line.contains("view 2: deuteranopia")
        && line.contains("Emulation.setEmulatedVisionDeficiency")));
    assert_eq!(answer["saved"], true);
}

#[test]
fn tritanomaly_is_drawn_by_the_page_script() {
    let mut host = host();
    let answer = set(&mut host, json!({ "type": "tritanomaly", "target": 1 }));
    assert!(emulated(&host).is_empty());
    assert_eq!(view_entry(&answer, 1)["mode"], "script");
    assert_eq!(view_entry(&answer, 1)["severity"], 0.6);
    let message = post_to(&answer, json!(1)).expect("posted to view 1");
    assert_eq!(message["type"], "apply");
    assert_eq!(message["spec"]["filter"][0]["op"], "matrix");
    assert!(host.logs.iter().any(|(level, line)| level == "info"
        && line.contains("view 1: tritanomaly")
        && line.contains("page script")));
}

#[test]
fn none_removes_the_simulation_and_clears_the_native_one() {
    let mut first = host();
    set(&mut first, json!({ "type": "protanopia", "target": 1 }));
    let mut host = with_saved(host(), &first);
    let answer = set(&mut host, json!({ "type": "none", "target": 1 }));
    assert_eq!(emulated(&host), [(1, "none".to_owned())]);
    assert_eq!(view_entry(&answer, 1)["mode"], "off");
    assert_eq!(view_entry(&answer, 1)["type"], "none");
    assert_eq!(
        post_to(&answer, json!(1)),
        Some(json!({ "type": "apply", "spec": null }))
    );
}

#[test]
fn switching_from_native_to_a_drawn_type_clears_the_native_one_first() {
    let mut first = host();
    set(&mut first, json!({ "type": "achromatopsia", "target": 2 }));
    let mut host = with_saved(host(), &first);
    let answer = set(&mut host, json!({ "type": "glaucoma", "target": 2 }));
    assert_eq!(emulated(&host), [(2, "none".to_owned())]);
    assert_eq!(view_entry(&answer, 2)["mode"], "script");
}

#[test]
fn all_views_get_it_and_firefox_draws_it_with_the_page_script() {
    let mut host = host();
    let answer = set(&mut host, json!({ "type": "protanopia", "target": "all" }));
    assert_eq!(
        emulated(&host),
        [(1, "protanopia".to_owned()), (2, "protanopia".to_owned())]
    );
    assert_eq!(view_entry(&answer, 3)["mode"], "script");
    assert_eq!(
        post_to(&answer, json!(3)).expect("posted to view 3")["spec"]["filter"][0]["op"],
        "matrix"
    );
    assert!(post_to(&answer, json!("panel")).is_some());
}

#[test]
fn the_same_drawing_everywhere_is_one_post_to_all() {
    let mut host = host();
    host.settings
        .insert("preferNative".to_owned(), "false".to_owned());
    let answer = set(&mut host, json!({ "type": "sunlight", "target": "all" }));
    let posts = answer["$post"].as_array().expect("posts");
    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0]["target"], "all");
    assert_eq!(posts[1]["target"], "panel");
    assert!(emulated(&host).is_empty());
}

#[test]
fn without_a_target_the_active_view_is_meant() {
    let mut host = host();
    host.active = Some(2);
    let answer = set(&mut host, json!({ "type": "dimmed" }));
    assert_eq!(view_entry(&answer, 2)["type"], "dimmed");
    assert_eq!(view_entry(&answer, 1)["type"], "none");

    let mut host = crate::testing::host();
    host.active = None;
    assert!(
        refused(&mut host, json!({ "type": "dimmed" }))
            .starts_with("plugin.simulators.errors.no_view")
    );
}

#[test]
fn bad_arguments_are_refused_by_name() {
    let mut host = host();
    assert_eq!(
        refused(&mut host, json!({ "type": "x-ray" })),
        "plugin.simulators.errors.bad_args reason=type"
    );
    assert_eq!(
        refused(&mut host, json!({ "type": "dimmed", "severity": 1.5 })),
        "plugin.simulators.errors.bad_args reason=severity"
    );
    assert_eq!(
        refused(&mut host, json!({ "type": "dimmed", "target": 99 })),
        "plugin.simulators.errors.bad_args reason=target"
    );
    assert_eq!(
        refused(&mut host, json!({ "type": "dimmed", "target": "every" })),
        "plugin.simulators.errors.bad_args reason=target"
    );
    assert!(host.cdp_sent.is_empty());
    assert!(host.out.is_empty());
}

#[test]
fn without_a_type_it_answers_the_state_and_the_catalogue() {
    let mut first = host();
    set(
        &mut first,
        json!({ "type": "cataract", "severity": 0.3, "target": 3 }),
    );
    let mut host = with_saved(host(), &first);
    let answer = set(&mut host, json!({}));
    assert!(host.cdp_sent.is_empty());
    assert!(host.out.is_empty());
    assert_eq!(answer["active"], 1);
    assert_eq!(view_entry(&answer, 3)["type"], "cataract");
    assert_eq!(view_entry(&answer, 3)["severity"], 0.3);
    assert_eq!(view_entry(&answer, 3)["label"], "Firefox");
    let catalog = answer["catalog"].as_array().expect("catalogue");
    assert_eq!(catalog.len(), 15);
    assert_eq!(
        catalog[0],
        json!({ "id": "protanopia", "group": "color", "severity": null, "native": true })
    );
    assert_eq!(catalog[1]["severity"], 0.6);
    assert!(answer.get("$post").is_none());
}

#[test]
fn a_strength_makes_chromium_draw_it_with_the_page_script() {
    let mut host = host();
    let answer = set(
        &mut host,
        json!({ "type": "myopia", "severity": 0.5, "target": 1 }),
    );
    assert!(emulated(&host).is_empty());
    let message = post_to(&answer, json!(1)).expect("posted");
    assert_eq!(
        message["spec"]["filter"][0],
        json!({ "op": "blur", "px": 1.0 })
    );

    // A dichromacy has no strength: one given is dropped and Chromium draws it.
    let mut host = crate::testing::host();
    let answer = set(
        &mut host,
        json!({ "type": "tritanopia", "severity": 0.5, "target": 1 }),
    );
    assert_eq!(emulated(&host), [(1, "tritanopia".to_owned())]);
    assert_eq!(view_entry(&answer, 1)["severity"], Value::Null);
}

#[test]
fn prefer_native_off_draws_everything_with_the_page_script() {
    let mut host = host();
    host.settings
        .insert("preferNative".to_owned(), "false".to_owned());
    let answer = set(&mut host, json!({ "type": "deuteranopia", "target": 1 }));
    assert!(emulated(&host).is_empty());
    assert_eq!(view_entry(&answer, 1)["mode"], "script");
}

#[test]
fn a_refused_native_emulation_falls_back_to_the_page_script() {
    let mut host = host();
    host.cdp_domains.clear();
    let answer = set(&mut host, json!({ "type": "deuteranopia", "target": 1 }));
    assert_eq!(view_entry(&answer, 1)["mode"], "script");
    assert!(post_to(&answer, json!(1)).expect("posted")["spec"].is_object());
    assert!(
        host.logs.iter().any(
            |(level, line)| level == "warn" && line.contains("plugins.error.cdp_domain_denied")
        )
    );
}

#[test]
fn without_a_project_the_views_still_change() {
    let mut host = host();
    host.views.truncate(1);
    let mut host = {
        let mut bare = rb_plugin_api::test_support::MemoryHost::new()
            .grant(rb_plugin_api::Capability::Views)
            .grant(rb_plugin_api::Capability::Cdp);
        bare.cdp_domains = host.cdp_domains.clone();
        bare.views = host.views.clone();
        bare.active = host.active;
        bare
    };
    let answer = set(&mut host, json!({ "type": "dimmed" }));
    assert_eq!(answer["saved"], false);
    assert!(post_to(&answer, json!(1)).is_some());
}

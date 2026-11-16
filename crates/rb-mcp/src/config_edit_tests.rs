use serde_json::{Map, Value, json};

use super::{
    BREAKPOINT, PROTECTED, add_breakpoint, refusal, remove_breakpoint, set, update_breakpoint,
    write_refusal,
};

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

#[test]
fn trust_guarded_keys_are_refused() {
    for key in [
        "network.proxy",
        "network.ignoreCertificateErrors",
        "plugins.enabled",
        "extensions",
        "emulation.headers",
        "emulation.headers.Authorization",
    ] {
        assert_eq!(
            refusal(key).map(|failure| failure.code),
            Some(PROTECTED.to_owned()),
            "{key}"
        );
    }
    assert!(refusal("layout.gap").is_none());
    assert!(refusal("startUrl").is_none());
}

/// The screenshot folder is a person's choice (batch-3 security review, finding 1): refused as
/// a key and inside the value of its group, which `config_set` and `settings_set` both ask.
#[test]
fn the_screenshot_folder_is_refused_as_key_and_inside_its_group() {
    let code = |failure: Option<super::ToolFailure>| failure.map(|failure| failure.code);
    assert_eq!(code(refusal("capture.dir")), Some(PROTECTED.to_owned()));
    assert!(refusal("capture.pathPattern").is_none());
    assert_eq!(
        code(write_refusal("capture", &json!({ "dir": "C:/elsewhere" }))),
        Some(PROTECTED.to_owned())
    );
    assert!(write_refusal("capture", &json!({ "pathPattern": "{view}.png" })).is_none());
    assert!(write_refusal("capture", &Value::Null).is_none(), "a reset");
    assert_eq!(
        code(write_refusal(
            "emulation",
            &json!({ "headers": { "X": "1" } })
        )),
        Some(PROTECTED.to_owned())
    );
    assert!(write_refusal("emulation.locale", &json!("de-DE")).is_none());
    let mut document = json!({});
    let refused = set(&mut document, "capture", json!({ "dir": "/tmp" })).expect_err("the folder");
    assert_eq!(refused.code, PROTECTED);
    assert_eq!(document, json!({}));
}

/// The recorder's folders are a person's choice like `capture.dir`, and its secret list only a
/// person shortens (charge-4 review): refused as keys and inside the value of their groups.
#[test]
fn the_recorder_folders_and_secret_fields_are_refused_as_key_and_inside_their_group() {
    let code = |failure: Option<super::ToolFailure>| failure.map(|failure| failure.code);
    for key in [
        "automation.recorder.dir",
        "automation.recorderExport.dir",
        "automation.recorder.secretFields",
    ] {
        assert_eq!(code(refusal(key)), Some(PROTECTED.to_owned()), "{key}");
    }
    assert!(refusal("automation.recorder.recordScroll").is_none());
    assert!(refusal("automation.recorderExport.locatorPreference").is_none());
    assert_eq!(
        code(write_refusal(
            "automation",
            &json!({ "recorder": { "secretFields": [] } })
        )),
        Some(PROTECTED.to_owned())
    );
    assert_eq!(
        code(write_refusal(
            "automation.recorderExport",
            &json!({ "dir": "C:/elsewhere" })
        )),
        Some(PROTECTED.to_owned())
    );
    assert!(write_refusal("automation.recorder", &json!({ "recordHover": true })).is_none());
}

#[test]
fn set_writes_nested_keys_and_null_removes() {
    let mut document = json!({ "layout": { "gap": 4 } });
    set(&mut document, "layout.gap", json!(8)).expect("set");
    set(&mut document, "emulation.locale", json!("de-DE")).expect("set");
    assert_eq!(
        document,
        json!({ "layout": { "gap": 8 }, "emulation": { "locale": "de-DE" } })
    );
    set(&mut document, "layout.gap", Value::Null).expect("removed");
    assert_eq!(document["layout"], json!({}));
}

#[test]
fn a_value_carrying_headers_is_refused_and_a_bad_key_too() {
    let mut document = json!({});
    let refused = set(
        &mut document,
        "emulation",
        json!({ "headers": { "X": "1" } }),
    )
    .expect_err("headers inside");
    assert_eq!(refused.code, PROTECTED);
    assert!(set(&mut document, "a..b", json!(1)).is_err());
    let mut scalar = json!({ "startUrl": "x" });
    assert!(set(&mut scalar, "startUrl.deep", json!(1)).is_err());
    assert_eq!(document, json!({}));
}

#[test]
fn breakpoints_are_added_updated_and_removed() {
    let mut document = json!({ "breakpoints": [{ "id": "mobile", "width": 375, "height": 812 }] });
    add_breakpoint(
        &mut document,
        object(json!({ "id": "phone", "device": "iphone-15" })),
    )
    .expect("a device without a size");
    let again = add_breakpoint(&mut document, object(json!({ "id": "phone" }))).expect_err("twice");
    assert_eq!(again.code, BREAKPOINT);
    update_breakpoint(
        &mut document,
        "mobile",
        object(json!({ "width": 390, "emulation": { "colorScheme": "dark" }, "height": null })),
    )
    .expect("merged");
    assert_eq!(
        document["breakpoints"][0],
        json!({ "id": "mobile", "width": 390, "emulation": { "colorScheme": "dark" } })
    );
    remove_breakpoint(&mut document, "phone").expect("removed");
    assert_eq!(document["breakpoints"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        remove_breakpoint(&mut document, "phone")
            .expect_err("gone")
            .code,
        BREAKPOINT
    );
    assert_eq!(
        update_breakpoint(&mut document, "nope", Map::new())
            .expect_err("missing")
            .code,
        BREAKPOINT
    );
}

#[test]
fn breakpoint_headers_and_a_new_id_are_refused() {
    let mut document = json!({ "breakpoints": [{ "id": "mobile" }] });
    let refused = add_breakpoint(
        &mut document,
        object(json!({ "id": "x", "headers": { "Cookie": "a" } })),
    )
    .expect_err("headers");
    assert_eq!(refused.code, PROTECTED);
    assert!(update_breakpoint(&mut document, "mobile", object(json!({ "id": "other" }))).is_err());
    assert!(add_breakpoint(&mut document, object(json!({ "width": 1 }))).is_err());
}

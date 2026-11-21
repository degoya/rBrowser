use serde_json::{Map, Value, json};

use super::{
    BREAKPOINT, PROTECTED, add_breakpoint, refusal, remove_breakpoint,
    remove_breakpoint_everywhere, set, update_breakpoint, write_refusal,
};
use crate::backend::ConfigScope;

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

/// RB-094-08: which environments get their Basic auth answered is a person's choice; MCP can
/// neither turn it on for another environment nor smuggle a credential into the section.
#[test]
fn the_basic_auth_section_is_refused_whole() {
    for key in ["basicAuth", "basicAuth.environments", "basicAuth.password"] {
        assert_eq!(
            refusal(key).map(|failure| failure.code),
            Some(PROTECTED.to_owned()),
            "{key}"
        );
    }
    let value = serde_json::json!({ "environments": ["staging"] });
    assert_eq!(
        write_refusal("basicAuth", &value).map(|failure| failure.code),
        Some(PROTECTED.to_owned())
    );
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

/// Where a bug report goes and the preview before it is saved are the person's (RB-093-04):
/// refused as keys and inside the value of their group; the parts stay an agent's to pick.
#[test]
fn the_report_folder_and_preview_are_refused_as_key_and_inside_their_group() {
    let code = |failure: Option<super::ToolFailure>| failure.map(|failure| failure.code);
    for key in ["report.folder", "report.preview"] {
        assert_eq!(code(refusal(key)), Some(PROTECTED.to_owned()), "{key}");
    }
    assert!(refusal("report.include").is_none());
    assert!(refusal("report.annotate").is_none());
    assert_eq!(
        code(write_refusal(
            "report",
            &json!({ "folder": "C:/elsewhere" })
        )),
        Some(PROTECTED.to_owned())
    );
    assert_eq!(
        code(write_refusal("report", &json!({ "preview": false }))),
        Some(PROTECTED.to_owned())
    );
    assert!(write_refusal("report", &json!({ "include": ["console"] })).is_none());
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

/// RB-090-61: removing a breakpoint takes the cells and hidden entries that named it, in the file
/// and in the other project file - the settings window's own function (`rb_config::breakpoint_refs`).
#[test]
fn removing_a_breakpoint_takes_its_references_from_both_files() {
    let mut project = json!({
        "breakpoints": [{ "id": "mobile" }, { "id": "tablet" }],
        "layout": { "cells": [
            { "breakpoint": "mobile", "col": 1, "row": 1 },
            { "breakpoint": "tablet", "col": 2, "row": 1 }
        ] },
        "layouts": { "presets": { "big": { "mode": "focus", "focus": "tablet" } } }
    });
    let mut local = json!({ "layout": { "hidden": ["tablet"] } });
    let found =
        remove_breakpoint_everywhere(&mut project, &mut local, ConfigScope::Project, "tablet")
            .expect("removed");
    assert_eq!((found.cells, found.hidden, found.other), (1, 1, 1));
    assert_eq!(project["breakpoints"], json!([{ "id": "mobile" }]));
    assert_eq!(
        project["layout"]["cells"],
        json!([{ "breakpoint": "mobile", "col": 1, "row": 1 }])
    );
    assert_eq!(
        project["layouts"]["presets"]["big"],
        json!({ "mode": "focus" })
    );
    assert_eq!(local, json!({ "layout": { "hidden": [] } }));
    assert_eq!(
        remove_breakpoint_everywhere(&mut project, &mut local, ConfigScope::Project, "tablet")
            .expect_err("gone")
            .code,
        BREAKPOINT
    );
}

#[test]
fn a_breakpoint_the_other_file_still_defines_keeps_its_references() {
    let mut local = json!({
        "breakpoints": [{ "id": "tablet", "width": 800 }],
        "layout": { "hidden": ["tablet"] }
    });
    let mut project = json!({ "breakpoints": [{ "id": "tablet", "width": 768 }] });
    let found =
        remove_breakpoint_everywhere(&mut local, &mut project, ConfigScope::Local, "tablet")
            .expect("removed from the local file");
    assert_eq!(found.total(), 0);
    assert_eq!(local["layout"]["hidden"], json!(["tablet"]));
}

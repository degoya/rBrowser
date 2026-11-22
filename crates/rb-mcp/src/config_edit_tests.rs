use serde_json::{Map, Value, json};

use super::{
    BREAKPOINT, PROTECTED, add_breakpoint, protected_refusal, refusal, remove_breakpoint,
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
    let refused =
        set(&mut document, "capture", json!({ "dir": "/tmp" }), &[]).expect_err("the folder");
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
    set(&mut document, "layout.gap", json!(8), &[]).expect("set");
    set(&mut document, "emulation.locale", json!("de-DE"), &[]).expect("set");
    assert_eq!(
        document,
        json!({ "layout": { "gap": 8 }, "emulation": { "locale": "de-DE" } })
    );
    set(&mut document, "layout.gap", Value::Null, &[]).expect("removed");
    assert_eq!(document["layout"], json!({}));
}

#[test]
fn a_value_carrying_headers_is_refused_and_a_bad_key_too() {
    let mut document = json!({});
    let refused = set(
        &mut document,
        "emulation",
        json!({ "headers": { "X": "1" } }),
        &[],
    )
    .expect_err("headers inside");
    assert_eq!(refused.code, PROTECTED);
    assert!(set(&mut document, "a..b", json!(1), &[]).is_err());
    let mut scalar = json!({ "startUrl": "x" });
    assert!(set(&mut scalar, "startUrl.deep", json!(1), &[]).is_err());
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

/// RB-094-25: the merge joins the two files' breakpoints by id, so a breakpoint both files define
/// leaves both, with its references - else the other file brings it back.
#[test]
fn a_breakpoint_both_files_define_leaves_both() {
    let mut local = json!({
        "breakpoints": [{ "id": "tablet", "width": 800 }],
        "layout": { "hidden": ["tablet"] }
    });
    let mut project =
        json!({ "breakpoints": [{ "id": "tablet", "width": 768 }, { "id": "mobile" }] });
    let found =
        remove_breakpoint_everywhere(&mut local, &mut project, ConfigScope::Local, "tablet")
            .expect("removed from both files");
    assert_eq!(found.hidden, 1);
    assert_eq!(local["breakpoints"], json!([]));
    assert_eq!(local["layout"]["hidden"], json!([]));
    assert_eq!(project["breakpoints"], json!([{ "id": "mobile" }]));
}

/// RB-094-25: a breakpoint only the other file defines is removed there; the scope names no file
/// that has to hold it.
#[test]
fn a_breakpoint_only_the_other_file_defines_is_removed_there() {
    let mut project = json!({
        "breakpoints": [{ "id": "mobile" }],
        "layout": { "cells": [
            { "breakpoint": "mobile", "col": 1, "row": 1 },
            { "breakpoint": "size-1600x1000", "col": 2, "row": 1 }
        ] }
    });
    let mut local =
        json!({ "breakpoints": [{ "id": "size-1600x1000", "width": 1600, "height": 1000 }] });
    let found = remove_breakpoint_everywhere(
        &mut project,
        &mut local,
        ConfigScope::Project,
        "size-1600x1000",
    )
    .expect("removed from the local file");
    assert_eq!(found.cells, 1);
    assert_eq!(local["breakpoints"], json!([]));
    assert_eq!(project["breakpoints"], json!([{ "id": "mobile" }]));
    assert_eq!(
        remove_breakpoint_everywhere(&mut project, &mut local, ConfigScope::Project, "nope")
            .expect_err("no file defines it")
            .code,
        BREAKPOINT
    );
}

/// Security review of wave 096-w32: a plugin key the settings registry protects from MCP (the
/// API client's and the load test's allowed hosts, the API client's secrets) is refused, and so
/// is a value at a parent key that sets one; the plugin's other keys stay open.
#[test]
fn a_plugin_key_protected_from_mcp_is_refused_with_a_parent_that_sets_it() {
    let protected = [
        "api-client.allowedHosts".to_owned(),
        "api-client.secrets".to_owned(),
        "load-test.allowedHosts".to_owned(),
    ];
    for (key, value) in [
        ("api-client.allowedHosts", json!(["attacker.example"])),
        ("load-test.allowedHosts", json!(["attacker.example"])),
        ("api-client.secrets", json!({ "local": { "token": "x" } })),
        ("api-client.secrets.local", json!({ "token": "x" })),
        (
            "api-client",
            json!({ "allowedHosts": ["attacker.example"] }),
        ),
    ] {
        let refused = protected_refusal(key, &value, &protected).map(|failure| failure.code);
        assert_eq!(refused.as_deref(), Some(PROTECTED), "{key}");
    }
    assert!(protected_refusal("api-client.historySize", &json!(20), &protected).is_none());
    assert!(protected_refusal("api-client", &json!({ "historySize": 20 }), &protected).is_none());
    assert!(
        protected_refusal("api-client", &Value::Null, &protected).is_none(),
        "a reset"
    );
}

/// Every `x-protected` key of a bundled plugin's settings schema is refused over MCP by name,
/// so `config_set` refuses it while the plugin is not loaded too.
#[test]
fn every_protected_key_of_a_bundled_plugin_is_refused() {
    fn walk(schema: &Value, path: &str, out: &mut Vec<String>) {
        if schema.get("x-protected") == Some(&Value::Bool(true)) {
            out.push(path.to_owned());
        }
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            for (name, child) in properties {
                walk(child, &format!("{path}.{name}"), out);
            }
        }
    }
    let plugins = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let mut protected = Vec::new();
    for entry in std::fs::read_dir(&plugins).expect("the plugins folder is there") {
        let dir = entry.expect("a plugin folder reads").path();
        let file = dir.join("settings.schema.json");
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let schema: Value = serde_json::from_str(&text).expect("a plugin's schema is JSON");
        let id = dir
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a plugin folder has a name");
        walk(&schema, id, &mut protected);
    }
    assert!(protected.len() >= 13, "{protected:?}");
    let unrefused: Vec<&String> = protected
        .iter()
        .filter(|key| refusal(key).is_none())
        .collect();
    assert!(unrefused.is_empty(), "add to REFUSED_KEYS: {unrefused:?}");
}

/// RB-094-31: a write at a parent key that removes or changes a protected key below it is
/// refused - `null`, or an object without the key - for the plugins' protected keys and for the
/// core's (`capture.dir`, the recorder's secret list); nothing is written. Where the file holds
/// nothing protected below the parent, or the write leaves it as it is, the parent stays open.
#[test]
fn a_parent_write_that_drops_or_changes_a_protected_key_is_refused() {
    let guarded = [
        "api-client.allowedHosts".to_owned(),
        "api-client.secrets".to_owned(),
        "acme-hosts.allowed".to_owned(),
    ];
    let held = json!({
        "api-client": { "allowedHosts": ["api.example"], "historySize": 5 },
        "acme-hosts": { "allowed": ["one.example"], "depth": 2 },
        "capture": { "dir": "C:/shots", "pathPattern": "{view}.png" },
        "automation": { "recorder": { "secretFields": ["pin"], "recordScroll": true } },
        "report": { "preview": true }
    });
    for (key, value) in [
        ("api-client", Value::Null),
        ("api-client", json!({ "historySize": 20 })),
        ("acme-hosts", Value::Null),
        ("acme-hosts", json!({ "depth": 3 })),
        ("capture", Value::Null),
        ("capture", json!({ "pathPattern": "{view}-2.png" })),
        ("automation", json!({})),
        ("automation.recorder", Value::Null),
        ("automation.recorder", json!({ "recordScroll": false })),
        ("report", Value::Null),
    ] {
        let mut document = held.clone();
        let refused = set(&mut document, key, value.clone(), &guarded)
            .expect_err(&format!("{key} = {value}"));
        assert_eq!(refused.code, PROTECTED, "{key} = {value}");
        assert_eq!(document, held, "{key} = {value}: nothing written");
    }
    let mut document = held.clone();
    set(&mut document, "api-client.historySize", json!(20), &guarded).expect("a key beside");
    set(&mut document, "acme-hosts.depth", Value::Null, &guarded).expect("a key beside");
    set(&mut document, "capture.pathPattern", Value::Null, &guarded).expect("a key beside");
    set(
        &mut document,
        "automation.recorder.recordScroll",
        json!(false),
        &guarded,
    )
    .expect("a key beside");
    assert_eq!(
        document["api-client"]["allowedHosts"],
        json!(["api.example"])
    );
    assert_eq!(document["acme-hosts"]["allowed"], json!(["one.example"]));
    assert_eq!(document["capture"]["dir"], json!("C:/shots"));
    let mut document = json!({
        "api-client": { "historySize": 5 },
        "capture": { "pathPattern": "a" }
    });
    set(&mut document, "api-client", Value::Null, &guarded).expect("nothing protected below");
    set(
        &mut document,
        "capture",
        json!({ "pathPattern": "b" }),
        &guarded,
    )
    .expect("no folder held");
    assert_eq!(document, json!({ "capture": { "pathPattern": "b" } }));
}

/// RB-094-31: a guarded key is refused at the key, below it and in a value set at its parent -
/// also a key of a plugin no registry knows (`acme-hosts`, guarded whole when its package does
/// not read).
#[test]
fn a_whole_plugin_prefix_can_be_guarded() {
    let guarded = ["acme-hosts".to_owned()];
    let mut document = json!({});
    for key in ["acme-hosts", "acme-hosts.allowed", "acme-hosts.depth"] {
        let refused = set(&mut document, key, json!(1), &guarded).expect_err(key);
        assert_eq!(refused.code, PROTECTED, "{key}");
    }
    assert_eq!(document, json!({}));
    set(&mut document, "acme-hosts-two.depth", json!(1), &guarded).expect("another id");
}

/// Review of wave 097-w33: a write at a parent key never drops or changes persisted request
/// headers below it - `emulation` emptied or reset, `breakpoints` replaced; the parent stays
/// open where the headers stay as they are.
#[test]
fn a_parent_write_that_drops_request_headers_is_refused() {
    let held = json!({
        "emulation": { "headers": { "X-Team": "a" }, "locale": "en" },
        "breakpoints": [{ "id": "phone", "width": 390, "headers": { "X-View": "p" } }]
    });
    for (key, value) in [
        ("emulation", json!({})),
        ("emulation", Value::Null),
        ("emulation", json!({ "locale": "de" })),
        ("breakpoints", json!([])),
        ("breakpoints", Value::Null),
        ("breakpoints", json!([{ "id": "phone", "width": 400 }])),
    ] {
        let mut document = held.clone();
        let refused =
            set(&mut document, key, value.clone(), &[]).expect_err(&format!("{key} = {value}"));
        assert_eq!(refused.code, PROTECTED, "{key} = {value}");
        assert_eq!(document, held, "{key} = {value}: nothing written");
    }
    let mut document = held.clone();
    set(&mut document, "emulation.locale", json!("de"), &[]).expect("a key beside");
    assert_eq!(document["emulation"]["headers"], json!({ "X-Team": "a" }));
    let mut document = json!({ "emulation": { "locale": "en" }, "breakpoints": [{ "id": "a" }] });
    set(&mut document, "emulation", json!({}), &[]).expect("no headers held");
    set(&mut document, "breakpoints", json!([]), &[]).expect("no headers held");
}

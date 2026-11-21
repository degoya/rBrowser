use serde_json::json;

use super::{paths, protected_paths, without};

#[test]
fn the_paths_follow_properties_references_and_optional_wrappers() {
    let schema = json!({
        "type": "object",
        "properties": {
            "startUrl": { "type": "string" },
            "breakpoints": { "type": "array", "x-protected": true },
            "network": { "$ref": "#/$defs/Network" },
            "capture": { "anyOf": [{ "$ref": "#/$defs/Capture" }, { "type": "null" }] },
        },
        "$defs": {
            "Network": {
                "type": "object",
                "properties": {
                    "proxy": { "type": "string", "x-protected": true },
                    "offline": { "type": "boolean" },
                },
            },
            "Capture": {
                "type": "object",
                "properties": { "dir": { "type": "string", "x-protected": true } },
            },
        },
    });
    assert_eq!(
        protected_paths(&schema),
        vec!["breakpoints", "capture.dir", "network.proxy"]
    );
}

#[test]
fn a_reference_cycle_ends() {
    let schema = json!({
        "properties": { "node": { "$ref": "#/$defs/Node" } },
        "$defs": {
            "Node": {
                "properties": {
                    "next": { "$ref": "#/$defs/Node" },
                    "secret": { "x-protected": true },
                },
            },
        },
    });
    assert_eq!(protected_paths(&schema), vec!["node.secret"]);
}

#[test]
fn without_drops_each_path_and_keeps_the_rest() {
    let config = json!({
        "version": 1,
        "breakpoints": [{ "id": "mobile", "width": 375 }],
        "network": { "proxy": "http://proxy.test:8080", "offline": false },
        "capture": null,
    });
    let paths = [
        "breakpoints".to_owned(),
        "network.proxy".to_owned(),
        "capture.dir".to_owned(),
        "emulation.headers".to_owned(),
    ];
    assert_eq!(
        without(config, &paths),
        json!({ "version": 1, "network": { "offline": false }, "capture": null })
    );
}

#[test]
fn the_project_schema_names_its_protected_keys() {
    // The keys `rb_config` marks `x-protected` (RB-010-02): request headers, the network keys,
    // the folders, the breakpoints (written through the breakpoint tools only).
    let paths = paths();
    for key in [
        "breakpoints",
        "emulation.headers",
        "network.proxy",
        "capture.dir",
        "extensions",
    ] {
        assert!(paths.iter().any(|path| path == key), "{key}: {paths:?}");
    }
    assert!(!paths.iter().any(|path| path == "startUrl"), "{paths:?}");
}

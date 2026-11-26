use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Map, Value, json};

use super::{BreakpointAddParams, BreakpointRemoveParams, BreakpointUpdateParams, ConfigSetParams};
use crate::backend::ConfigScope;
use crate::config_edit::PROTECTED;
use crate::testing::{FakeBackend, answer_json, server};

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

#[tokio::test]
async fn config_set_writes_the_chosen_file() {
    let fake = Arc::new(FakeBackend::default());
    let (error, _) = answer_json(
        &server(&fake)
            .config_set(Parameters(ConfigSetParams {
                key: "layout.gap".to_owned(),
                value: json!(12),
                scope: ConfigScope::Local,
            }))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(
        fake.document(ConfigScope::Local),
        json!({ "layout": { "gap": 12 } })
    );
    assert_eq!(fake.document(ConfigScope::Project), json!({}));
}

#[tokio::test]
async fn config_set_refuses_trust_guarded_keys_and_writes_nothing() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    for key in [
        "network.proxy",
        "emulation.headers",
        "plugins.allow",
        "extensions",
    ] {
        let (error, answer) = answer_json(
            &server
                .config_set(Parameters(ConfigSetParams {
                    key: key.to_owned(),
                    value: json!("x"),
                    scope: ConfigScope::Local,
                }))
                .await
                .expect("answer"),
        );
        assert!(error, "{key}");
        assert_eq!(answer["code"], PROTECTED, "{key}");
    }
    assert_eq!(fake.document(ConfigScope::Local), json!({}));
}

/// Security review of wave 096-w32: with the plugins loaded, `config_set` refuses what the
/// settings registry protects from MCP (`crate::testing::PLUGIN_PROTECTED`) - a host an MCP
/// client allows itself would be read as the person's grant - and writes a plugin's other keys.
#[tokio::test]
async fn config_set_refuses_plugin_keys_protected_from_mcp() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    for (key, value) in [
        ("api-client.allowedHosts", json!(["attacker.example"])),
        ("load-test.allowedHosts", json!(["attacker.example"])),
        ("api-client.secrets", json!({})),
        (
            "api-client",
            json!({ "allowedHosts": ["attacker.example"] }),
        ),
    ] {
        let (error, answer) = answer_json(
            &server
                .config_set(Parameters(ConfigSetParams {
                    key: key.to_owned(),
                    value,
                    scope: ConfigScope::Local,
                }))
                .await
                .expect("answer"),
        );
        assert!(error, "{key}");
        assert_eq!(answer["code"], PROTECTED, "{key}");
    }
    assert_eq!(fake.document(ConfigScope::Local), json!({}));
    let (error, _) = answer_json(
        &server
            .config_set(Parameters(ConfigSetParams {
                key: "api-client.historySize".to_owned(),
                value: json!(20),
                scope: ConfigScope::Local,
            }))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(
        fake.document(ConfigScope::Local),
        json!({ "api-client": { "historySize": 20 } })
    );
}

/// RB-094-31: `config_set` at a parent key that would drop or change a protected key the file
/// holds is refused and writes nothing - `null` or an object without the key, for a plugin's
/// protected key and the core's `capture.dir`; the plugin's other keys stay writable.
#[tokio::test]
async fn config_set_refuses_a_parent_write_that_drops_a_protected_key() {
    let fake = Arc::new(FakeBackend::default());
    let held = json!({
        "api-client": { "allowedHosts": ["api.example"], "historySize": 5 },
        "capture": { "dir": "C:/shots" }
    });
    fake.set_document(ConfigScope::Local, held.clone());
    let server = server(&fake);
    for (key, value) in [
        ("api-client", Value::Null),
        ("api-client", json!({ "historySize": 20 })),
        ("capture", Value::Null),
        ("capture", json!({ "pathPattern": "{view}.png" })),
    ] {
        let (error, answer) = answer_json(
            &server
                .config_set(Parameters(ConfigSetParams {
                    key: key.to_owned(),
                    value: value.clone(),
                    scope: ConfigScope::Local,
                }))
                .await
                .expect("answer"),
        );
        assert!(error, "{key} = {value}");
        assert_eq!(answer["code"], PROTECTED, "{key} = {value}");
    }
    assert_eq!(fake.document(ConfigScope::Local), held);
    let (error, _) = answer_json(
        &server
            .config_set(Parameters(ConfigSetParams {
                key: "api-client.historySize".to_owned(),
                value: json!(20),
                scope: ConfigScope::Local,
            }))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(
        fake.document(ConfigScope::Local)["api-client"],
        json!({ "allowedHosts": ["api.example"], "historySize": 20 })
    );
}

/// Review of wave 097-w33: a protected key is refused as `config.key_protected` before the file
/// is read - also with no project open or the file unreadable (the fake's `null` document).
#[tokio::test]
async fn config_set_refuses_a_protected_key_before_reading_the_file() {
    let fake = Arc::new(FakeBackend::default());
    fake.set_document(ConfigScope::Project, Value::Null);
    let server = server(&fake);
    for (key, value) in [
        ("emulation.headers", json!({ "X": "1" })),
        ("capture.dir", json!("C:/elsewhere")),
        ("api-client.allowedHosts", json!(["attacker.example"])),
    ] {
        let (error, answer) = answer_json(
            &server
                .config_set(Parameters(ConfigSetParams {
                    key: key.to_owned(),
                    value,
                    scope: ConfigScope::Project,
                }))
                .await
                .expect("answer"),
        );
        assert!(error, "{key}");
        assert_eq!(answer["code"], PROTECTED, "{key}");
    }
    let (error, answer) = answer_json(
        &server
            .config_set(Parameters(ConfigSetParams {
                key: "layout.gap".to_owned(),
                value: json!(4),
                scope: ConfigScope::Project,
            }))
            .await
            .expect("answer"),
    );
    assert!(error, "no file to write");
    assert_ne!(answer["code"], PROTECTED);
}

#[tokio::test]
async fn breakpoints_go_through_the_hosts_check() {
    let fake = Arc::new(FakeBackend::default());
    fake.set_document(
        ConfigScope::Project,
        json!({ "breakpoints": [{ "id": "mobile", "width": 375 }] }),
    );
    let server = server(&fake);
    server
        .breakpoint_add(Parameters(BreakpointAddParams {
            breakpoint: object(json!({ "id": "tablet", "width": 768, "height": 1024 })),
            scope: ConfigScope::Project,
        }))
        .await
        .expect("answer");
    server
        .breakpoint_update(Parameters(BreakpointUpdateParams {
            id: "mobile".to_owned(),
            fields: object(json!({ "width": 390 })),
            scope: ConfigScope::Project,
        }))
        .await
        .expect("answer");
    let (error, answer) = answer_json(
        &server
            .breakpoint_update(Parameters(BreakpointUpdateParams {
                id: "mobile".to_owned(),
                fields: object(json!({ "width": -1 })),
                scope: ConfigScope::Project,
            }))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], "config.error.invalid");
    server
        .breakpoint_remove(Parameters(BreakpointRemoveParams {
            id: "tablet".to_owned(),
            scope: ConfigScope::Project,
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.document(ConfigScope::Project),
        json!({ "breakpoints": [{ "id": "mobile", "width": 390 }] })
    );
}

/// RB-090-61: `breakpoint_remove` writes the other file first, without the references, then the
/// file itself; the answer counts what went along.
#[tokio::test]
async fn breakpoint_remove_takes_its_references_from_both_files() {
    let fake = Arc::new(FakeBackend::default());
    fake.set_document(
        ConfigScope::Project,
        json!({
            "breakpoints": [{ "id": "mobile", "width": 375 }, { "id": "tablet", "width": 768 }],
            "layout": { "cells": [
                { "breakpoint": "mobile", "col": 1, "row": 1 },
                { "breakpoint": "tablet", "col": 2, "row": 1 }
            ] }
        }),
    );
    fake.set_document(
        ConfigScope::Local,
        json!({ "layout": { "hidden": ["tablet"] } }),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .breakpoint_remove(Parameters(BreakpointRemoveParams {
                id: "tablet".to_owned(),
                scope: ConfigScope::Project,
            }))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["removed"], "tablet");
    assert_eq!(
        answer["references"],
        json!({ "cells": 1, "hidden": 1, "other": 0 })
    );
    assert_eq!(
        fake.document(ConfigScope::Project),
        json!({
            "breakpoints": [{ "id": "mobile", "width": 375 }],
            "layout": { "cells": [{ "breakpoint": "mobile", "col": 1, "row": 1 }] }
        })
    );
    assert_eq!(
        fake.document(ConfigScope::Local),
        json!({ "layout": { "hidden": [] } })
    );
}

/// RB-094-25: a breakpoint only `config.local.json` defines (a view added at a size) is removed
/// there although the scope is the project - and one both files define leaves both.
#[tokio::test]
async fn breakpoint_remove_takes_the_breakpoint_from_every_file_that_defines_it() {
    let fake = Arc::new(FakeBackend::default());
    fake.set_document(
        ConfigScope::Project,
        json!({ "breakpoints": [{ "id": "mobile", "width": 375 }, { "id": "desktop", "width": 1920 }] }),
    );
    fake.set_document(
        ConfigScope::Local,
        json!({ "breakpoints": [
            { "id": "size-1600x1000", "width": 1600, "height": 1000 },
            { "id": "desktop", "width": 2000 }
        ] }),
    );
    for id in ["size-1600x1000", "desktop"] {
        let (error, answer) = answer_json(
            &server(&fake)
                .breakpoint_remove(Parameters(BreakpointRemoveParams {
                    id: id.to_owned(),
                    scope: ConfigScope::Project,
                }))
                .await
                .expect("answer"),
        );
        assert!(!error, "{id}: {answer}");
    }
    assert_eq!(
        fake.document(ConfigScope::Project),
        json!({ "breakpoints": [{ "id": "mobile", "width": 375 }] })
    );
    assert_eq!(
        fake.document(ConfigScope::Local),
        json!({ "breakpoints": [] })
    );
}

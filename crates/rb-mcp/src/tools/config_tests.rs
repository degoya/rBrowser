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

use serde_json::json;

use super::*;

#[test]
fn a_callers_keys_are_dropped_and_only_a_granted_plugin_is_offered() {
    let forged = json!({ "a": 1, "$apiClient": "panel", "$apiResult": { "status": 200 } });
    assert_eq!(offered(&forged, false, Origin::Tool), json!({ "a": 1 }));
    assert_eq!(
        offered(&forged, true, Origin::Tool),
        json!({ "a": 1, "$apiClient": "tool" })
    );
    assert_eq!(
        offered(&Value::Null, true, Origin::Panel),
        json!({ "$apiClient": "panel" })
    );
    assert_eq!(offered(&Value::Null, false, Origin::Panel), Value::Null);
    assert_eq!(
        offer_of(&json!({ "$apiClient": "command" })),
        Some(Origin::Command)
    );
    assert_eq!(offer_of(&json!({ "$apiClient": "admin" })), None);
    assert_eq!(offer_of(&json!({})), None);
}

#[test]
fn an_answer_asks_for_an_op_or_for_nothing() {
    assert_eq!(requested("{\"ok\": true}"), None);
    assert_eq!(requested("not json"), None);
    assert_eq!(
        requested(r#"{"$apiClient": {"op": "run", "id": "users/list.http"}}"#),
        Some(Ok(Op::Run {
            id: "users/list.http".into(),
            environment: None
        }))
    );
    let send = requested(
        r#"{"$apiClient": {"op": "send", "request": {"method": "GET", "url": "https://a.example"}}}"#,
    );
    assert!(matches!(send, Some(Ok(Op::Send { .. }))));
    let bad = requested(r#"{"$apiClient": {"op": "format_disk"}}"#);
    assert!(matches!(bad, Some(Err(failure)) if failure.code() == Some(codes::INVALID)));
}

#[test]
fn mcp_and_commands_read_and_run_saved_requests_only() {
    let draft = Op::Send {
        request: ApiRequest::new("GET", "https://a.example"),
        environment: None,
        id: None,
    };
    let saved = Op::Run {
        id: "a.http".into(),
        environment: None,
    };
    for origin in [Origin::Tool, Origin::Command] {
        assert!(permitted(&saved, origin).is_ok());
        assert!(permitted(&Op::List, origin).is_ok());
        assert!(permitted(&Op::History, origin).is_ok());
        assert!(
            permitted(
                &Op::Load {
                    id: "a.http".into()
                },
                origin
            )
            .is_ok()
        );
        for refused in [
            draft.clone(),
            Op::Allow {
                host: "api.example.com".into(),
            },
            Op::Delete {
                id: "a.http".into(),
            },
            Op::State,
        ] {
            let failure = permitted(&refused, origin).expect_err("refused");
            assert_eq!(failure.code(), Some(codes::CAPABILITY_DENIED));
        }
    }
    assert!(permitted(&draft, Origin::Panel).is_ok());
}

#[test]
fn the_second_call_carries_the_result_and_no_offer() {
    let args = json!({ "id": "a.http", "$apiClient": "tool" });
    assert_eq!(
        with_result(&args, json!({ "status": 204 })),
        json!({ "id": "a.http", "$apiResult": { "status": 204 } })
    );
}

#[test]
fn mcp_runs_saved_collections_reads_the_report_and_benchmarks_saved_requests() {
    let run: Op =
        serde_json::from_value(json!({ "op": "runCollection", "folder": "users" })).expect("reads");
    assert_eq!(
        run,
        Op::RunCollection {
            folder: Some("users".into()),
            environment: None,
        }
    );
    let bench: Op = serde_json::from_value(
        json!({ "op": "bench", "id": "a.http", "environments": ["staging", "production"] }),
    )
    .expect("reads");
    assert_eq!(bench.id(), "bench");
    for origin in [Origin::Tool, Origin::Command, Origin::Panel] {
        assert!(permitted(&run, origin).is_ok());
        assert!(permitted(&Op::Report, origin).is_ok());
        assert!(permitted(&bench, origin).is_ok());
    }
    let free: Result<Op, _> = serde_json::from_value(
        json!({ "op": "bench", "request": { "method": "GET", "url": "https://a.example" } }),
    );
    assert!(free.is_err(), "a benchmark takes a saved request only");
}

#[test]
fn an_import_reads_with_its_defaults_and_is_the_panels_alone() {
    let op = requested(
        r#"{"$apiClient": {"op": "import", "files": [{"name": "a.json", "text": "{}"}]}}"#,
    );
    let Some(Ok(op)) = op else {
        panic!("reads: {op:?}")
    };
    assert_eq!(
        op,
        Op::Import {
            files: vec![ImportFile {
                name: "a.json".into(),
                text: "{}".into()
            }],
            format: None,
            folder: None,
            environments: true,
            preview: false,
        }
    );
    assert_eq!(op.id(), "import");
    assert!(permitted(&op, Origin::Panel).is_ok());
    for origin in [Origin::Tool, Origin::Command] {
        let failure = permitted(&op, origin).expect_err("refused");
        assert_eq!(failure.code(), Some(codes::CAPABILITY_DENIED));
    }
}

#[test]
fn a_capture_reads_its_requests_and_only_the_panel_opens_one() {
    let save = requested(
        r#"{"$apiClient": {"op": "capture", "requests": [{"view": 1, "id": "1.3"}], "save": true}}"#,
    );
    assert_eq!(
        save,
        Some(Ok(Op::Capture {
            requests: vec![CaptureRef {
                view: 1,
                id: "1.3".into()
            }],
            save: true,
            open: false,
        }))
    );
    let Some(Ok(save)) = save else {
        panic!("a capture reads");
    };
    assert_eq!(save.id(), "capture");
    for origin in [Origin::Panel, Origin::Tool, Origin::Command] {
        assert!(permitted(&save, origin).is_ok(), "{origin:?}");
    }
    let open = Op::Capture {
        requests: Vec::new(),
        save: false,
        open: true,
    };
    assert!(permitted(&open, Origin::Panel).is_ok());
    assert!(permitted(&open, Origin::Tool).is_err());
    assert!(permitted(&open, Origin::Command).is_err());
    let forged = requested(
        r#"{"$apiClient": {"op": "capture", "requests": [{"view": 1, "id": "1.3", "headers": {}}]}}"#,
    );
    assert!(matches!(forged, Some(Err(_))));
}

#[test]
fn collections_variables_and_private_values_are_changed_from_the_panel_only() {
    let ops = [
        json!({ "op": "createFolder", "folder": "users" }),
        json!({ "op": "renameFolder", "folder": "users", "to": "people" }),
        json!({ "op": "duplicateFolder", "folder": "users" }),
        json!({ "op": "deleteFolder", "folder": "users" }),
        json!({ "op": "duplicate", "id": "users/me.http", "name": "Me copy" }),
        json!({ "op": "move", "id": "users/me.http", "folder": "people" }),
        json!({ "op": "variables" }),
        json!({ "op": "saveVariables", "file": {}, "private": { "dev": { "a": "1", "b": null }, "qa": null }, "renamed": { "x": "y" } }),
        json!({ "op": "setSecret", "name": "password", "value": "pw" }),
        json!({ "op": "secrets", "names": ["password"], "environment": "dev" }),
    ];
    for op in ops {
        let answer = json!({ OFFER_KEY: op }).to_string();
        let op = requested(&answer).expect("asks").expect("reads");
        assert!(permitted(&op, Origin::Panel).is_ok(), "{op:?}");
        for origin in [Origin::Tool, Origin::Command] {
            let failure = permitted(&op, origin).expect_err("refused");
            assert_eq!(failure.code(), Some(codes::CAPABILITY_DENIED), "{op:?}");
        }
    }
    let deleted = json!({ OFFER_KEY: { "op": "deleteFolder" } }).to_string();
    assert!(requested(&deleted).expect("asks").is_err());
}

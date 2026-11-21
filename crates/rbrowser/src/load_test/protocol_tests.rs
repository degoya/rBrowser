use serde_json::json;

use super::*;

#[test]
fn a_callers_keys_are_dropped_and_only_a_granted_plugin_is_offered() {
    let forged = json!({ "x": 1, OFFER_KEY: "panel", RESULT_KEY: { "started": true } });
    assert_eq!(offered(&forged, false, Origin::Tool), json!({ "x": 1 }));
    assert_eq!(
        offered(&forged, true, Origin::Tool),
        json!({ "x": 1, OFFER_KEY: "tool" })
    );
    assert_eq!(
        offered(&Value::Null, true, Origin::Panel),
        json!({ OFFER_KEY: "panel" })
    );
    assert_eq!(offered(&Value::Null, false, Origin::Panel), Value::Null);
    assert_eq!(
        offer_of(&json!({ OFFER_KEY: "command" })),
        Some(Origin::Command)
    );
    assert_eq!(offer_of(&json!({ OFFER_KEY: "root" })), None);
}

#[test]
fn an_answer_asks_for_an_op_or_for_nothing() {
    assert_eq!(requested("{\"plain\":1}"), None);
    assert_eq!(requested("not json"), None);
    assert_eq!(
        requested(&json!({ OFFER_KEY: { "op": "stop" } }).to_string()),
        Some(Ok(Op::Stop))
    );
    let start = json!({ OFFER_KEY: { "op": "start", "id": "a.json",
        "confirm": { "targets": ["localhost"], "users": 50, "rps": 200.0, "durationS": 300 } } });
    let Some(Ok(Op::Start { id, confirm })) = requested(&start.to_string()) else {
        panic!("a start");
    };
    assert_eq!(id, "a.json");
    assert_eq!(confirm.users, 50);
    let bad = requested(&json!({ OFFER_KEY: { "op": "start", "id": "a.json" } }).to_string());
    assert_eq!(
        bad.expect("asks").expect_err("bad").code(),
        Some(codes::INVALID)
    );
}

#[test]
fn mcp_only_reads_reports_and_never_starts_changes_or_stops_a_run() {
    let confirm = Confirm {
        targets: vec!["localhost".into()],
        users: 1,
        rps: 1.0,
        duration_s: 1,
    };
    let writing = [
        Op::State,
        Op::Load {
            id: "a.json".into(),
        },
        Op::Delete {
            id: "a.json".into(),
        },
        Op::Preview {
            id: "a.json".into(),
        },
        Op::Allow {
            host: "example.org".into(),
            confirmed: true,
        },
        Op::Start {
            id: "a.json".into(),
            confirm,
        },
        Op::Stop,
        Op::Status,
    ];
    for op in &writing {
        let failure = permitted(op, Origin::Tool).expect_err(op.id());
        assert_eq!(failure.code(), Some(codes::CAPABILITY_DENIED));
        assert_eq!(failure.param("capability"), Some("load-test"));
        assert!(permitted(op, Origin::Panel).is_ok());
    }
    for op in [
        Op::Reports,
        Op::Report { id: None },
        Op::Compare {
            a: "1-a".into(),
            b: "2-a".into(),
        },
    ] {
        assert!(permitted(&op, Origin::Tool).is_ok());
    }
}

#[test]
fn a_command_may_stop_a_run_and_nothing_else_of_it() {
    assert!(permitted(&Op::Stop, Origin::Command).is_ok());
    assert!(permitted(&Op::Reports, Origin::Command).is_ok());
    assert!(
        permitted(
            &Op::Preview {
                id: "a.json".into()
            },
            Origin::Command
        )
        .is_err()
    );
    assert!(
        permitted(
            &Op::Allow {
                host: "x".into(),
                confirmed: true
            },
            Origin::Command
        )
        .is_err()
    );
}

#[test]
fn the_second_call_gets_the_result_without_the_offer() {
    let args = json!({ "op": {}, OFFER_KEY: "panel" });
    assert_eq!(
        with_result(&args, json!({ "ok": true })),
        json!({ "op": {}, RESULT_KEY: { "ok": true } })
    );
}

/// RB-096-09: the panel's empty request list opens the API client panel - a person's click only.
#[test]
fn only_the_panel_opens_the_api_client() {
    let answer = json!({ OFFER_KEY: { "op": "openApiClient" } });
    assert_eq!(requested(&answer.to_string()), Some(Ok(Op::OpenApiClient)));
    assert_eq!(Op::OpenApiClient.id(), "openApiClient");
    assert!(permitted(&Op::OpenApiClient, Origin::Panel).is_ok());
    assert!(permitted(&Op::OpenApiClient, Origin::Command).is_err());
    assert!(permitted(&Op::OpenApiClient, Origin::Tool).is_err());
}

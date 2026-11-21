use serde_json::json;

use super::*;

#[test]
fn an_open_from_the_panel_asks_the_core_to_open_the_api_client() {
    let answer = api_open(&json!({ "view": 2, "id": "2.7", OFFER_KEY: "panel" })).expect("asks");
    assert_eq!(
        answer,
        json!({ OFFER_KEY: { "op": "capture", "requests": [{ "view": 2, "id": "2.7" }], "open": true } })
    );
}

#[test]
fn an_open_over_mcp_only_reads_the_request() {
    let answer = api_open(&json!({ "view": 2, "id": "2.7", OFFER_KEY: "tool" })).expect("asks");
    assert_eq!(answer[OFFER_KEY]["open"], false);
    assert_eq!(answer[OFFER_KEY]["save"], Value::Null);
}

#[test]
fn a_save_names_every_request_once() {
    let answer = api_save(&json!({
        "requests": [{ "view": 1, "id": "1.3" }, { "view": 2, "id": "2.9" }],
        OFFER_KEY: "panel",
    }))
    .expect("asks");
    assert_eq!(
        answer,
        json!({ OFFER_KEY: { "op": "capture", "save": true,
                             "requests": [{ "view": 1, "id": "1.3" }, { "view": 2, "id": "2.9" }] } })
    );
}

#[test]
fn the_cores_result_is_the_answer() {
    let result = json!({ "saved": [], "skipped": [], "variables": [] });
    assert_eq!(
        api_save(&json!({ RESULT_KEY: result.clone() })).expect("answers"),
        result
    );
    assert_eq!(
        api_open(&json!({ RESULT_KEY: result.clone() })).expect("answers"),
        result
    );
}

#[test]
fn without_the_api_client_nothing_is_asked() {
    let failure = api_open(&json!({ "view": 1, "id": "1.3" })).expect_err("no core");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.network-panel.errors.no_api_client")
    );
}

#[test]
fn arguments_of_the_wrong_form_are_refused() {
    let offer = |mut args: Value| {
        args[OFFER_KEY] = json!("panel");
        args
    };
    for args in [
        json!({ "id": "1.3" }),
        json!({ "view": -1, "id": "1.3" }),
        json!({ "view": 1, "id": "" }),
        json!({ "view": 1, "id": "x".repeat(201) }),
        json!({ "view": 5_000_000_000_u64, "id": "1.3" }),
    ] {
        assert!(api_open(&offer(args.clone())).is_err(), "{args}");
    }
    for args in [
        json!({}),
        json!({ "requests": [] }),
        json!({ "requests": "1.3" }),
        json!({ "requests": [{ "view": 1 }] }),
        json!({ "requests": (0..=MAX_REQUESTS).map(|n| json!({ "view": 1, "id": n.to_string() })).collect::<Vec<_>>() }),
    ] {
        assert!(api_save(&offer(args.clone())).is_err(), "{args}");
    }
}

#[test]
fn the_commands_show_the_panel_and_tell_it_what_to_do() {
    assert_eq!(
        command(OPEN_COMMAND),
        Some(json!({ "$open": { "panel": "network" },
                     "$post": [{ "target": "panel", "message": { "apiOpen": true } }] }))
    );
    assert_eq!(
        command(SAVE_COMMAND),
        Some(json!({ "$open": { "panel": "network" },
                     "$post": [{ "target": "panel", "message": { "apiSave": true } }] }))
    );
    assert_eq!(command("other"), None);
}

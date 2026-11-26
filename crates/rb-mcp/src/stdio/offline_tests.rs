use serde_json::json;

use super::{NO_INSTANCE, NO_INSTANCE_CODE, PROTOCOL_VERSION, answer};

const WHY: &str = "no running rBrowser instance for `/home/me/site`";

#[test]
fn initialize_succeeds_with_the_client_s_version_and_says_why_in_the_instructions() {
    let message = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": { "protocolVersion": "2025-03-26", "capabilities": {} },
    })
    .to_string();
    let answer = answer(&message, WHY).expect("an answer");
    assert_eq!(answer["id"], 1);
    assert_eq!(answer["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(answer["result"]["serverInfo"]["name"], "rbrowser");
    assert!(answer["result"]["capabilities"]["tools"].is_object());
    // A client that connects before rBrowser runs still learns of the prompts and resources
    // (RB-095-01).
    assert!(answer["result"]["capabilities"]["prompts"].is_object());
    assert!(answer["result"]["capabilities"]["resources"].is_object());
    let instructions = answer["result"]["instructions"].as_str().expect("text");
    assert!(instructions.contains(WHY), "{instructions}");
    let bare = super::answer(r#"{"jsonrpc":"2.0","id":"a","method":"initialize"}"#, WHY)
        .expect("an answer");
    assert_eq!(bare["result"]["protocolVersion"], PROTOCOL_VERSION);
}

#[test]
fn ping_answers_and_notifications_get_nothing() {
    let ping = answer(r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#, WHY).expect("an answer");
    assert_eq!(ping, json!({ "jsonrpc": "2.0", "id": 2, "result": {} }));
    assert!(
        answer(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            WHY
        )
        .is_none()
    );
    assert!(answer(r#"{"jsonrpc":"2.0","id":null,"method":"tools/list"}"#, WHY).is_none());
}

#[test]
fn every_other_request_is_an_error_that_says_what_to_do() {
    for method in ["tools/list", "tools/call", "resources/list"] {
        let message = json!({ "jsonrpc": "2.0", "id": 7, "method": method }).to_string();
        let answer = answer(&message, WHY).expect("an answer");
        assert_eq!(answer["id"], 7, "{method}");
        assert_eq!(answer["error"]["code"], NO_INSTANCE_CODE, "{method}");
        assert_eq!(answer["error"]["message"], WHY, "{method}");
        assert_eq!(answer["error"]["data"]["code"], NO_INSTANCE, "{method}");
        assert_eq!(
            answer["error"]["data"]["legacyCode"], "no_instance",
            "{method}"
        );
    }
}

#[test]
fn a_line_that_is_no_json_is_a_parse_error() {
    let answer = answer("{ not json", WHY).expect("an answer");
    assert_eq!(answer["error"]["code"], -32700);
    assert!(answer["id"].is_null());
}

/// F2 (RB-030-02): the offline `initialize` promises `tools/list_changed`, so a client re-lists
/// once the bridge reaches rBrowser.
#[test]
fn initialize_promises_the_list_changed_notification() {
    let answer = answer(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        WHY,
    )
    .expect("an answer");
    assert_eq!(
        answer["result"]["capabilities"]["tools"]["listChanged"],
        true
    );
}

/// F2: only a version the server takes is echoed - the bridge sends it as `MCP-Protocol-Version`
/// once rBrowser runs, and the server refuses an unknown one with HTTP 400.
#[test]
fn initialize_echoes_only_a_version_the_server_takes() {
    let version = |asked: &str| {
        let message = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": { "protocolVersion": asked },
        })
        .to_string();
        answer(&message, WHY).expect("an answer")["result"]["protocolVersion"].clone()
    };
    assert_eq!(version("2024-11-05"), "2024-11-05");
    assert_eq!(version("2025-11-25"), "2025-11-25");
    assert_eq!(version("2099-01-01"), PROTOCOL_VERSION);
    assert_eq!(
        version("2026-07-28"),
        PROTOCOL_VERSION,
        "no initialize handshake"
    );
}

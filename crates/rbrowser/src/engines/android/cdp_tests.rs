use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use super::*;

#[test]
fn a_command_frame_carries_its_id_and_session() {
    let frame: Value = serde_json::from_str(&command_frame(
        7,
        "Page.navigate",
        &json!({ "url": "https://example.test/" }),
        Some("S1"),
    ))
    .expect("json");
    assert_eq!(
        frame,
        json!({
            "id": 7,
            "method": "Page.navigate",
            "params": { "url": "https://example.test/" },
            "sessionId": "S1",
        })
    );
    let bare: Value =
        serde_json::from_str(&command_frame(1, "Target.getTargets", &json!({}), None))
            .expect("json");
    assert!(bare.get("sessionId").is_none());
}

#[test]
fn answers_errors_and_events_are_told_apart() {
    assert_eq!(
        parse_incoming(r#"{"id":3,"result":{"frameId":"F"}}"#),
        Some(Incoming::Answer {
            id: 3,
            outcome: Ok(json!({ "frameId": "F" })),
        })
    );
    assert_eq!(
        parse_incoming(r#"{"id":4,"error":{"code":-32000,"message":"Cannot navigate"}}"#),
        Some(Incoming::Answer {
            id: 4,
            outcome: Err((-32000, "Cannot navigate".to_owned())),
        })
    );
    assert_eq!(
        parse_incoming(
            r#"{"method":"Page.frameNavigated","params":{"frame":{"url":"u"}},"sessionId":"S"}"#
        ),
        Some(Incoming::Event(CdpEvent {
            method: "Page.frameNavigated".to_owned(),
            params: json!({ "frame": { "url": "u" } }),
            session: Some("S".to_owned()),
        }))
    );
    assert_eq!(parse_incoming("not json"), None);
    assert_eq!(parse_incoming(r#"{"what":1}"#), None);
}

/// Only loopback is dialled: the forwarded port is on 127.0.0.1.
#[tokio::test]
async fn only_a_loopback_socket_is_dialled() {
    for url in [
        "ws://10.0.0.7:9222/devtools/browser/x",
        "ws://example.test:9222/devtools/browser/x",
        "http://127.0.0.1:9222/",
    ] {
        assert!(CdpClient::connect(url).await.is_err(), "{url} was dialled");
    }
}

#[tokio::test]
async fn a_command_gets_its_answer_and_events_reach_subscribers() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!(
        "ws://{}/devtools/browser/test",
        listener.local_addr().expect("addr")
    );
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut ws = tokio_tungstenite::accept_async(stream)
            .await
            .expect("handshake");
        while let Some(Ok(Message::Text(text))) = ws.next().await {
            let command: Value = serde_json::from_str(text.as_str()).expect("json");
            let event = json!({ "method": "Page.loadEventFired", "params": {} });
            let answer = json!({ "id": command["id"], "result": { "echo": command["method"] } });
            let _sent = ws.send(Message::text(event.to_string())).await;
            let _sent = ws.send(Message::text(answer.to_string())).await;
        }
    });
    let client = CdpClient::connect(&url).await.expect("connected");
    let mut events = client.events();
    let answer = client
        .call("Page.enable", json!({}), None)
        .await
        .expect("answered");
    assert_eq!(answer, json!({ "echo": "Page.enable" }));
    let event = events.recv().await.expect("an event");
    assert_eq!(event.method, "Page.loadEventFired");
    client.close();
}

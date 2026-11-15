use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use super::*;

/// A WebSocket server the test drives by hand: the commands it got come out of `commands`,
/// frames put into `frames` go to the client; `None` closes the socket.
struct Server {
    url: String,
    commands: mpsc::UnboundedReceiver<Value>,
    frames: mpsc::UnboundedSender<Option<String>>,
}

async fn server() -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("ws://{}/session", listener.local_addr().expect("addr"));
    let (command_tx, commands) = mpsc::unbounded_channel();
    let (frames, mut frame_rx) = mpsc::unbounded_channel::<Option<String>>();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let ws = tokio_tungstenite::accept_async(stream)
            .await
            .expect("handshake");
        let (mut sink, mut source) = ws.split();
        loop {
            tokio::select! {
                frame = frame_rx.recv() => match frame {
                    Some(Some(text)) => {
                        if sink.send(Message::text(text)).await.is_err() {
                            return;
                        }
                    }
                    _ => {
                        let _closed = sink.close().await;
                        return;
                    }
                },
                message = source.next() => match message {
                    Some(Ok(Message::Text(text))) => {
                        let value = serde_json::from_str(text.as_str()).expect("json command");
                        if command_tx.send(value).is_err() {
                            return;
                        }
                    }
                    Some(Ok(_)) => {}
                    _ => return,
                },
            }
        }
    });
    Server {
        url,
        commands,
        frames,
    }
}

fn success(id: &Value, result: Value) -> Option<String> {
    Some(json!({ "type": "success", "id": id, "result": result }).to_string())
}

const SHORT: Duration = Duration::from_millis(200);

#[tokio::test]
async fn answers_find_their_commands_in_any_order() {
    let mut server = server().await;
    let client = BidiClient::connect(&server.url, DEFAULT_TIMEOUT)
        .await
        .expect("connect");
    let first = tokio::spawn({
        let client = client.clone();
        async move { client.command("a.first", json!({})).await }
    });
    let one = server.commands.recv().await.expect("first");
    let second = tokio::spawn({
        let client = client.clone();
        async move { client.command("b.second", json!({ "n": 2 })).await }
    });
    let two = server.commands.recv().await.expect("second");
    assert_eq!(one["method"], "a.first");
    assert_eq!(two["method"], "b.second");
    assert_eq!(two["params"], json!({ "n": 2 }));
    assert_ne!(one["id"], two["id"]);
    // The second is answered first.
    server
        .frames
        .send(success(&two["id"], json!("two")))
        .expect("send");
    server
        .frames
        .send(success(&one["id"], json!("one")))
        .expect("send");
    assert_eq!(second.await.expect("join"), Ok(json!("two")));
    assert_eq!(first.await.expect("join"), Ok(json!("one")));
}

#[tokio::test]
async fn a_command_without_answer_times_out_and_the_client_goes_on() {
    let mut server = server().await;
    let client = BidiClient::connect(&server.url, DEFAULT_TIMEOUT)
        .await
        .expect("connect");
    let result = client
        .command_within("browsingContext.setViewport", json!({}), SHORT)
        .await;
    assert_eq!(
        result,
        Err(BidiError::Timeout {
            method: "browsingContext.setViewport".to_owned(),
            ms: 200,
        })
    );
    let late = server.commands.recv().await.expect("command");
    // The late answer is dropped; the next command works.
    server
        .frames
        .send(success(&late["id"], json!({})))
        .expect("send");
    let next = tokio::spawn({
        let client = client.clone();
        async move { client.command("session.status", json!({})).await }
    });
    let status = server.commands.recv().await.expect("status");
    server
        .frames
        .send(success(&status["id"], json!({ "ready": false })))
        .expect("send");
    assert_eq!(next.await.expect("join"), Ok(json!({ "ready": false })));
}

#[tokio::test]
async fn an_error_answer_is_typed_and_unknown_commands_are_unsupported() {
    let mut server = server().await;
    let client = BidiClient::connect(&server.url, DEFAULT_TIMEOUT)
        .await
        .expect("connect");
    let call = tokio::spawn({
        let client = client.clone();
        async move {
            client
                .command("emulation.setTouchOverride", json!({}))
                .await
        }
    });
    let command = server.commands.recv().await.expect("command");
    let frame = json!({
        "type": "error", "id": command["id"], "error": "unknown command",
        "message": "emulation.setTouchOverride", "stacktrace": ""
    });
    server.frames.send(Some(frame.to_string())).expect("send");
    let error = call.await.expect("join").expect_err("refused");
    assert_eq!(
        error,
        BidiError::Command {
            method: "emulation.setTouchOverride".to_owned(),
            error: "unknown command".to_owned(),
            message: "emulation.setTouchOverride".to_owned(),
        }
    );
    assert!(error.is_unsupported());
}

#[tokio::test]
async fn events_reach_every_subscriber_and_garbage_is_skipped() {
    let server = server().await;
    let client = BidiClient::connect(&server.url, DEFAULT_TIMEOUT)
        .await
        .expect("connect");
    let mut first = client.events();
    let mut second = client.events();
    server
        .frames
        .send(Some("{ not json".to_owned()))
        .expect("send");
    let event =
        json!({ "type": "event", "method": "browsingContext.load", "params": { "context": "c1" } });
    server.frames.send(Some(event.to_string())).expect("send");
    for events in [&mut first, &mut second] {
        let event = events.recv().await.expect("event");
        assert_eq!(event.method, "browsingContext.load");
        assert_eq!(event.params["context"], "c1");
    }
}

#[tokio::test]
async fn a_closed_socket_fails_pending_and_later_commands() {
    let mut server = server().await;
    let client = BidiClient::connect(&server.url, DEFAULT_TIMEOUT)
        .await
        .expect("connect");
    let mut events = client.events();
    let pending = tokio::spawn({
        let client = client.clone();
        async move { client.command("browser.close", json!({})).await }
    });
    let _command = server.commands.recv().await.expect("command");
    server.frames.send(None).expect("close");
    assert_eq!(pending.await.expect("join"), Err(BidiError::Closed));
    tokio::time::timeout(Duration::from_secs(2), client.closed())
        .await
        .expect("closed in time");
    assert!(client.is_closed());
    assert_eq!(
        client.command("session.status", json!({})).await,
        Err(BidiError::Closed)
    );
    assert!(events.recv().await.is_err(), "the events end");
    assert!(client.events().recv().await.is_err(), "and stay ended");
}

#[test]
fn only_loopback_endpoints_are_dialled() {
    let (addr, url) = loopback_target("ws://127.0.0.1:9222/session").expect("ipv4");
    assert_eq!(addr.to_string(), "127.0.0.1:9222");
    assert_eq!(url, "ws://127.0.0.1:9222/session");
    let (addr, _) = loopback_target("ws://localhost:9333/session").expect("localhost");
    assert_eq!(addr.to_string(), "127.0.0.1:9333");
    let (addr, _) = loopback_target("ws://[::1]:9444").expect("ipv6");
    assert_eq!(addr.to_string(), "[::1]:9444");
    for refused in [
        "ws://192.168.1.10:9222/session",
        "ws://example.com:9222/session",
        "wss://127.0.0.1:9222/session",
        "http://127.0.0.1:9222/session",
        "ws://127.0.0.1/session",
    ] {
        assert!(loopback_target(refused).is_err(), "{refused}");
    }
    assert_eq!(
        loopback_target("ws://10.0.0.1:1/x"),
        Err(BidiError::NotLoopback)
    );
}

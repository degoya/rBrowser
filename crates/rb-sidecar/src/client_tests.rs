//! The client against a fake sidecar on an in-memory pipe.

use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, Lines};
use tokio::sync::mpsc;

use super::*;

/// The sidecar's end of the pipes.
struct Fake {
    requests: Lines<BufReader<DuplexStream>>,
    answers: DuplexStream,
}

impl Fake {
    async fn next(&mut self) -> Value {
        let line = self
            .requests
            .next_line()
            .await
            .expect("reads")
            .expect("a line");
        serde_json::from_str(&line).expect("JSON")
    }

    async fn send(&mut self, message: Value) {
        let mut line = message.to_string();
        line.push('\n');
        self.answers
            .write_all(line.as_bytes())
            .await
            .expect("writes");
    }

    async fn answer(&mut self, id: &Value, result: Value) {
        self.send(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
            .await;
    }
}

fn connect() -> (RpcClient, Fake) {
    let (client_out, fake_in) = tokio::io::duplex(4096);
    let (fake_out, client_in) = tokio::io::duplex(4096);
    let client = RpcClient::start(client_in, client_out);
    let fake = Fake {
        requests: BufReader::new(fake_in).lines(),
        answers: fake_out,
    };
    (client, fake)
}

const LONG: Option<Duration> = Some(Duration::from_secs(5));

#[tokio::test]
async fn a_request_gets_its_result() {
    let (client, mut fake) = connect();
    let call = tokio::spawn({
        let client = client.clone();
        async move { client.request("sidecar.ping", Value::Null, LONG).await }
    });
    let request = fake.next().await;
    assert_eq!(request["jsonrpc"], "2.0");
    assert_eq!(request["method"], "sidecar.ping");
    assert!(request.get("params").is_none());
    fake.answer(&request["id"], json!({ "pong": true })).await;
    assert_eq!(
        call.await.expect("joins").expect("answered"),
        json!({ "pong": true })
    );
}

#[tokio::test]
async fn answers_out_of_order_reach_their_callers() {
    let (client, mut fake) = connect();
    let first = tokio::spawn({
        let client = client.clone();
        async move { client.request("a", json!({}), LONG).await }
    });
    let one = fake.next().await;
    let second = tokio::spawn({
        let client = client.clone();
        async move { client.request("b", json!({}), LONG).await }
    });
    let two = fake.next().await;
    assert_ne!(one["id"], two["id"]);
    fake.answer(&two["id"], json!("b")).await;
    fake.answer(&one["id"], json!("a")).await;
    assert_eq!(second.await.expect("joins").expect("b"), json!("b"));
    assert_eq!(first.await.expect("joins").expect("a"), json!("a"));
}

#[tokio::test]
async fn an_error_answer_becomes_a_remote_error() {
    let (client, mut fake) = connect();
    let call = tokio::spawn({
        let client = client.clone();
        async move { client.request("nope", Value::Null, LONG).await }
    });
    let request = fake.next().await;
    fake.send(json!({
        "jsonrpc": "2.0",
        "id": request["id"],
        "error": { "code": -32601, "message": "unknown method nope" }
    }))
    .await;
    let error = call.await.expect("joins").expect_err("remote error");
    assert!(matches!(
        error,
        SidecarError::Remote { ref method, code: -32601, ref message, .. }
            if method == "nope" && message == "unknown method nope"
    ));
}

#[tokio::test(start_paused = true)]
async fn a_timeout_ends_the_request_and_cancels_it_in_the_sidecar() {
    let (client, mut fake) = connect();
    let call = tokio::spawn({
        let client = client.clone();
        async move {
            client
                .request("slow", Value::Null, Some(Duration::from_millis(200)))
                .await
        }
    });
    let request = fake.next().await;
    let error = call.await.expect("joins").expect_err("times out");
    assert!(matches!(error, SidecarError::Timeout { ref method, ms: 200 } if method == "slow"));
    let cancel = fake.next().await;
    assert_eq!(cancel["method"], CANCEL);
    assert_eq!(cancel["params"]["id"], request["id"]);
    assert!(cancel.get("id").is_none());
    // A late answer is ignored, and the client stays usable.
    fake.answer(&request["id"], json!("late")).await;
    let next = tokio::spawn({
        let client = client.clone();
        async move { client.request("fast", Value::Null, LONG).await }
    });
    let request = fake.next().await;
    fake.answer(&request["id"], json!("ok")).await;
    assert_eq!(next.await.expect("joins").expect("ok"), json!("ok"));
}

#[tokio::test]
async fn dropping_a_request_cancels_it_in_the_sidecar() {
    let (client, mut fake) = connect();
    let call = tokio::spawn({
        let client = client.clone();
        async move { client.request("wait", Value::Null, None).await }
    });
    let request = fake.next().await;
    call.abort();
    let cancel = fake.next().await;
    assert_eq!(cancel["method"], CANCEL);
    assert_eq!(cancel["params"]["id"], request["id"]);
}

#[tokio::test]
async fn progress_goes_to_its_request_and_other_notifications_to_subscribers() {
    let (client, mut fake) = connect();
    let mut events = client.subscribe();
    let (progress, mut updates) = mpsc::unbounded_channel();
    let call = tokio::spawn({
        let client = client.clone();
        async move {
            client
                .request_with_progress("work", Value::Null, LONG, progress)
                .await
        }
    });
    let request = fake.next().await;
    let id = request["id"].clone();
    fake.send(
        json!({ "jsonrpc": "2.0", "method": PROGRESS, "params": { "id": id, "percent": 40 } }),
    )
    .await;
    fake.send(json!({ "jsonrpc": "2.0", "method": "webkit.event", "params": { "x": 1 } }))
        .await;
    fake.answer(&id, json!("done")).await;
    assert_eq!(call.await.expect("joins").expect("done"), json!("done"));
    assert_eq!(
        updates.recv().await.expect("progress"),
        json!({ "id": id, "percent": 40 })
    );
    let event = events.recv().await.expect("event");
    assert_eq!(event.method, "webkit.event");
    assert_eq!(event.params, json!({ "x": 1 }));
}

#[tokio::test]
async fn a_malformed_line_is_skipped() {
    let (client, mut fake) = connect();
    let call = tokio::spawn({
        let client = client.clone();
        async move { client.request("a", Value::Null, LONG).await }
    });
    let request = fake.next().await;
    fake.answers
        .write_all(b"Debugger listening\n")
        .await
        .expect("writes");
    fake.answer(&request["id"], json!(1)).await;
    assert_eq!(call.await.expect("joins").expect("answered"), json!(1));
}

#[tokio::test]
async fn an_exit_ends_running_requests_with_an_error() {
    let (client, mut fake) = connect();
    let call = tokio::spawn({
        let client = client.clone();
        async move { client.request("a", Value::Null, None).await }
    });
    fake.next().await;
    drop(fake);
    let error = call.await.expect("joins").expect_err("exited");
    assert!(matches!(error, SidecarError::Exited));
    client.closed().await;
    assert!(client.is_closed());
    let error = client
        .request("b", Value::Null, LONG)
        .await
        .expect_err("closed");
    assert!(matches!(error, SidecarError::Exited));
}

#[tokio::test(start_paused = true)]
async fn the_close_time_is_when_the_output_ended() {
    let (client, fake) = connect();
    assert_eq!(client.closed_at(), None);
    tokio::time::sleep(Duration::from_secs(1)).await;
    let ended = tokio::time::Instant::now().into_std();
    drop(fake);
    client.closed().await;
    tokio::time::sleep(Duration::from_secs(60)).await;
    assert_eq!(client.closed_at(), Some(ended));
}

/// `close_input` closes the sidecar's stdin even while clones of the client live (a request in
/// flight holds one), so the sidecar can end on its own (RB-040-04 review, finding 2).
#[tokio::test]
async fn close_input_ends_the_sidecars_stdin_while_clients_live() {
    let (client, mut fake) = connect();
    let held = client.clone();
    client.close_input();
    let end = tokio::time::timeout(Duration::from_secs(2), fake.requests.next_line()).await;
    assert!(matches!(end, Ok(Ok(None))), "stdin ended: {end:?}");
    drop(held);
}

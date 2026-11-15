use std::sync::Mutex;

use super::*;
use crate::instance::protocol::Envelope;

const NONCE: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

fn recording() -> (Handler, Arc<Mutex<Vec<Request>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let handler: Handler = Arc::new(move |request| {
        log.lock().expect("lock").push(request);
        Ok(None)
    });
    (handler, seen)
}

async fn exchange(input: Vec<u8>, handler: &Handler) -> (Result<Request, Refusal>, Vec<u8>) {
    let (mut client, server) = tokio::io::duplex(256 * 1024);
    client.write_all(&input).await.expect("writes");
    let result = serve_one(server, NONCE, handler).await;
    let mut answer = Vec::new();
    client.read_to_end(&mut answer).await.expect("reads");
    (result, answer)
}

#[tokio::test]
async fn a_valid_request_is_run_and_answered() {
    let (handler, seen) = recording();
    let request = Request::Open {
        urls: vec!["http://localhost:5173/a".to_owned()],
        background: false,
    };
    let (result, answer) = exchange(
        protocol::encode(&Envelope::new(NONCE, request.clone())),
        &handler,
    )
    .await;
    assert_eq!(result, Ok(request.clone()));
    assert_eq!(protocol::decode_reply(&answer), Some(Reply::ok()));
    assert_eq!(*seen.lock().expect("lock"), vec![request]);
}

#[tokio::test]
async fn a_wrong_nonce_is_refused_and_nothing_runs() {
    let (handler, seen) = recording();
    let line = protocol::encode(&Envelope::new(
        &NONCE.replace('f', "e"),
        Request::Reload {
            ignore_cache: false,
        },
    ));
    let (result, answer) = exchange(line, &handler).await;
    assert_eq!(result, Err(Refusal::BadNonce));
    assert_eq!(
        protocol::decode_reply(&answer),
        Some(Reply::refused(Refusal::BadNonce))
    );
    assert!(seen.lock().expect("lock").is_empty());
}

#[tokio::test]
async fn an_overlong_or_unfinished_line_is_refused() {
    let (handler, seen) = recording();
    let (result, _) = exchange(vec![b'x'; MAX_LINE + 10], &handler).await;
    assert_eq!(result, Err(Refusal::BadRequest));
    let (result, _) = exchange(b"{\"version\":1".to_vec(), &handler).await;
    assert_eq!(result, Err(Refusal::BadRequest));
    assert!(seen.lock().expect("lock").is_empty());
}

#[tokio::test]
async fn a_failing_request_is_answered_as_failed() {
    let handler: Handler = Arc::new(|_| Err("the window is closed".to_owned()));
    let line = protocol::encode(&Envelope::new(
        NONCE,
        Request::Reload {
            ignore_cache: false,
        },
    ));
    let (result, answer) = exchange(line, &handler).await;
    assert_eq!(result, Err(Refusal::Failed));
    assert_eq!(
        protocol::decode_reply(&answer),
        Some(Reply::failed("the window is closed"))
    );
}

fn screenshot_line() -> Vec<u8> {
    protocol::encode(&Envelope::new(
        NONCE,
        Request::Screenshot {
            args: crate::capture::ScreenshotArgs::default(),
            views: vec!["mobile".to_owned()],
            out: None,
        },
    ))
}

#[tokio::test]
async fn an_answer_goes_back_with_the_reply() {
    let handler: Handler = Arc::new(|_| Ok(Some(serde_json::json!({ "shots": [] }))));
    let (result, answer) = exchange(screenshot_line(), &handler).await;
    assert!(result.is_ok());
    assert_eq!(
        protocol::decode_reply(&answer),
        Some(Reply::answered(serde_json::json!({ "shots": [] })))
    );
}

#[tokio::test]
async fn an_answer_too_long_for_a_reply_is_answered_as_failed() {
    let handler: Handler =
        Arc::new(|_| Ok(Some(serde_json::json!("x".repeat(protocol::MAX_REPLY)))));
    let (result, answer) = exchange(screenshot_line(), &handler).await;
    assert_eq!(result, Err(Refusal::Failed));
    assert!(answer.len() <= protocol::MAX_REPLY);
    let reply = protocol::decode_reply(&answer).expect("a reply");
    assert_eq!(reply.error, Some(Refusal::Failed));
    assert_eq!(reply.answer, None);
}

#[tokio::test]
async fn a_slow_client_is_cut_off() {
    let (_client, server) = tokio::io::duplex(1024);
    let mut reader = tokio::io::BufReader::new(server);
    let line = read_line_within(&mut reader, 64, Duration::from_millis(50)).await;
    assert_eq!(line, None);
}

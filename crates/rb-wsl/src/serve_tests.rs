use std::io::Cursor;

use super::*;
use crate::protocol::{ErrorCode, PROTOCOL_VERSION, decode, encode, hello, new_nonce};

fn run(input: &str) -> (Served, Vec<Message>) {
    let mut output = Vec::new();
    let served = serve(Cursor::new(input.as_bytes().to_vec()), &mut output, "0.3.0")
        .expect("in-memory io does not fail");
    let text = String::from_utf8(output).expect("utf-8");
    let messages = text
        .lines()
        .map(|line| decode(line).expect("every line is a message"))
        .collect();
    (served, messages)
}

#[test]
fn handshake_ping_and_shutdown() {
    let nonce = new_nonce();
    let input = format!(
        "{}{}{}",
        encode(&hello(&nonce)),
        encode(&Message::Ping),
        encode(&Message::Shutdown)
    );
    let (served, messages) = run(&input);
    assert_eq!(served, Served::Shutdown);
    assert_eq!(
        messages,
        vec![
            Message::Welcome {
                protocol: PROTOCOL_VERSION,
                helper_version: "0.3.0".into(),
                nonce
            },
            Message::Pong
        ]
    );
}

#[test]
fn the_end_of_input_ends_the_helper() {
    let (served, messages) = run(&encode(&hello(&new_nonce())));
    assert_eq!(served, Served::Closed);
    assert_eq!(messages.len(), 1);
    let (served, messages) = run("");
    assert_eq!(served, Served::HandshakeFailed(HandshakeError::Closed));
    assert!(messages.is_empty());
}

#[test]
fn a_wrong_protocol_ends_the_helper_with_its_reason() {
    let hello = Message::Hello {
        protocol: PROTOCOL_VERSION + 1,
        nonce: new_nonce(),
    };
    let (served, messages) = run(&format!("{}{}", encode(&hello), encode(&Message::Ping)));
    assert_eq!(
        served,
        Served::HandshakeFailed(HandshakeError::ProtocolMismatch {
            app: PROTOCOL_VERSION + 1,
            helper: PROTOCOL_VERSION
        })
    );
    assert!(matches!(
        messages.as_slice(),
        [Message::Error {
            code: ErrorCode::ProtocolMismatch,
            ..
        }]
    ));
}

#[test]
fn a_bad_line_after_the_handshake_is_answered_not_fatal() {
    let input = format!(
        "{}garbage\n{}{}",
        encode(&hello(&new_nonce())),
        encode(&Message::Pong),
        encode(&Message::Ping)
    );
    let (served, messages) = run(&input);
    assert_eq!(served, Served::Closed);
    assert!(matches!(
        messages.as_slice(),
        [
            Message::Welcome { .. },
            Message::Error {
                code: ErrorCode::BadRequest,
                ..
            },
            Message::Error {
                code: ErrorCode::BadRequest,
                ..
            },
            Message::Pong
        ]
    ));
}

#[test]
fn an_overlong_line_is_refused_without_reading_it_whole() {
    let input = format!("{}\n", "a".repeat(MAX_LINE * 2));
    let (served, _) = run(&input);
    assert_eq!(served, Served::HandshakeFailed(HandshakeError::TooLong));
}

#[test]
fn bounded_lines() {
    let mut reader = Cursor::new(b"one\ntwo".to_vec());
    assert_eq!(
        read_line(&mut reader, 10).expect("io"),
        Line::Text("one\n".into())
    );
    assert_eq!(
        read_line(&mut reader, 10).expect("io"),
        Line::Text("two".into())
    );
    assert_eq!(read_line(&mut reader, 10).expect("io"), Line::End);
    let mut reader = Cursor::new(b"0123456789abc\nnext\n".to_vec());
    assert_eq!(read_line(&mut reader, 10).expect("io"), Line::TooLong);
    skip_line(&mut reader).expect("io");
    assert_eq!(
        read_line(&mut reader, 10).expect("io"),
        Line::Text("next\n".into())
    );
}

#[test]
fn an_overlong_line_after_the_handshake_is_skipped() {
    let input = format!(
        "{}{}\n{}",
        encode(&hello(&new_nonce())),
        "a".repeat(MAX_LINE + 10),
        encode(&Message::Ping)
    );
    let (served, messages) = run(&input);
    assert_eq!(served, Served::Closed);
    assert!(matches!(
        messages.as_slice(),
        [
            Message::Welcome { .. },
            Message::Error {
                code: ErrorCode::BadRequest,
                ..
            },
            Message::Pong
        ]
    ));
}

use super::*;

fn line(message: &Message) -> String {
    encode(message)
}

#[test]
fn messages_are_one_json_line_each() {
    let hello = Message::Hello {
        protocol: PROTOCOL_VERSION,
        nonce: "ab".repeat(16),
    };
    let text = encode(&hello);
    assert!(text.ends_with('\n'));
    assert_eq!(text.matches('\n').count(), 1);
    assert_eq!(
        text,
        format!(
            "{{\"type\":\"hello\",\"protocol\":1,\"nonce\":\"{}\"}}\n",
            "ab".repeat(16)
        )
    );
    assert_eq!(decode(&text), Ok(hello));
    assert_eq!(decode("{\"type\":\"ping\"}"), Ok(Message::Ping));
    assert_eq!(
        encode(&Message::Welcome {
            protocol: 1,
            helper_version: "0.3.0".into(),
            nonce: "n".into()
        }),
        "{\"type\":\"welcome\",\"protocol\":1,\"helperVersion\":\"0.3.0\",\"nonce\":\"n\"}\n"
    );
}

#[test]
fn bad_lines_are_refused() {
    assert!(matches!(
        decode("not json"),
        Err(HandshakeError::Malformed(_))
    ));
    assert!(matches!(
        decode("{\"type\":\"nope\"}"),
        Err(HandshakeError::Malformed(_))
    ));
    let long = format!("{{\"type\":\"ping\",\"x\":\"{}\"}}", "a".repeat(MAX_LINE));
    assert_eq!(decode(&long), Err(HandshakeError::TooLong));
}

#[test]
fn nonces_are_fresh_hex() {
    let (a, b) = (new_nonce(), new_nonce());
    assert_ne!(a, b);
    assert!(valid_nonce(&a));
    assert!(!valid_nonce("xyz"));
    assert!(!valid_nonce(&"A".repeat(32)));
    assert!(!valid_nonce(&"a".repeat(33)));
}

#[test]
fn the_helper_welcomes_its_own_version() {
    let nonce = new_nonce();
    let (reply, outcome) = answer_hello(&line(&hello(&nonce)), "0.3.0");
    assert_eq!(outcome, Ok(()));
    assert_eq!(
        reply,
        Message::Welcome {
            protocol: PROTOCOL_VERSION,
            helper_version: "0.3.0".into(),
            nonce: nonce.clone()
        }
    );
    assert_eq!(check_welcome(&reply, &nonce), Ok("0.3.0".to_owned()));
}

#[test]
fn a_wrong_protocol_is_a_clear_error_on_the_helper_side() {
    let hello = Message::Hello {
        protocol: PROTOCOL_VERSION + 1,
        nonce: new_nonce(),
    };
    let (reply, outcome) = answer_hello(&line(&hello), "0.3.0");
    let expected = HandshakeError::ProtocolMismatch {
        app: PROTOCOL_VERSION + 1,
        helper: PROTOCOL_VERSION,
    };
    assert_eq!(outcome, Err(expected.clone()));
    assert!(expected.to_string().contains("protocol"), "{expected}");
    let Message::Error { code, protocol, .. } = &reply else {
        panic!("an error reply, not {reply:?}");
    };
    assert_eq!(*code, ErrorCode::ProtocolMismatch);
    assert_eq!(*protocol, Some(PROTOCOL_VERSION));
}

#[test]
fn a_wrong_protocol_is_a_clear_error_on_the_app_side() {
    let nonce = new_nonce();
    let refused = Message::Error {
        code: ErrorCode::ProtocolMismatch,
        message: "x".into(),
        protocol: Some(PROTOCOL_VERSION + 1),
    };
    let error = check_welcome(&refused, &nonce).expect_err("refused");
    assert_eq!(
        error,
        HandshakeError::ProtocolMismatch {
            app: PROTOCOL_VERSION,
            helper: PROTOCOL_VERSION + 1
        }
    );
    assert_eq!(error.i18n_key(), "wsl.error.protocolMismatch");
    let newer = Message::Welcome {
        protocol: PROTOCOL_VERSION + 1,
        helper_version: "9.0.0".into(),
        nonce: nonce.clone(),
    };
    assert_eq!(
        check_welcome(&newer, &nonce),
        Err(HandshakeError::ProtocolMismatch {
            app: PROTOCOL_VERSION,
            helper: PROTOCOL_VERSION + 1
        })
    );
}

#[test]
fn the_app_checks_the_nonce_and_the_message_type() {
    let nonce = new_nonce();
    let other = Message::Welcome {
        protocol: PROTOCOL_VERSION,
        helper_version: "0.3.0".into(),
        nonce: new_nonce(),
    };
    assert_eq!(
        check_welcome(&other, &nonce),
        Err(HandshakeError::NonceMismatch)
    );
    assert_eq!(
        check_welcome(&Message::Pong, &nonce),
        Err(HandshakeError::Unexpected("pong".into()))
    );
    let refused = Message::Error {
        code: ErrorCode::BadRequest,
        message: "bad".into(),
        protocol: None,
    };
    assert!(matches!(
        check_welcome(&refused, &nonce),
        Err(HandshakeError::Refused(_))
    ));
}

#[test]
fn the_helper_refuses_what_is_no_hello() {
    let (reply, outcome) = answer_hello("{\"type\":\"ping\"}", "0.3.0");
    assert_eq!(outcome, Err(HandshakeError::Unexpected("ping".into())));
    assert!(matches!(
        reply,
        Message::Error {
            code: ErrorCode::BadRequest,
            ..
        }
    ));
    let bad_nonce = Message::Hello {
        protocol: PROTOCOL_VERSION,
        nonce: "not hex".into(),
    };
    let (_, outcome) = answer_hello(&line(&bad_nonce), "0.3.0");
    assert!(matches!(outcome, Err(HandshakeError::Malformed(_))));
    let (_, outcome) = answer_hello("garbage", "0.3.0");
    assert!(matches!(outcome, Err(HandshakeError::Malformed(_))));
}

#[test]
fn the_watch_messages_have_their_wire_form() {
    use crate::watch::{Change, ChangeKind, WatchRules};
    let watch = Message::Watch {
        rules: WatchRules {
            watch: vec!["src/**".into()],
            ignore: vec!["dist/".into()],
            debounce_ms: 200,
        },
    };
    assert_eq!(
        encode(&watch),
        "{\"type\":\"watch\",\"rules\":{\"watch\":[\"src/**\"],\"ignore\":[\"dist/\"],\
         \"debounceMs\":200}}\n"
    );
    assert_eq!(decode(&encode(&watch)), Ok(watch));
    let changes = Message::Changes {
        changes: vec![Change::new("src/app.css", ChangeKind::Modify)],
    };
    assert_eq!(
        encode(&changes),
        "{\"type\":\"changes\",\"changes\":[{\"path\":\"src/app.css\",\"kind\":\"modify\"}]}\n"
    );
    assert_eq!(decode("{\"type\":\"rescan\"}"), Ok(Message::Rescan));
    assert_eq!(
        decode("{\"type\":\"watching\",\"folders\":12}"),
        Ok(Message::Watching { folders: 12 })
    );
    assert_eq!(Message::Rescan.kind(), "rescan");
    assert_eq!(
        decode("{\"type\":\"error\",\"code\":\"watch_failed\",\"message\":\"x\"}"),
        Ok(Message::Error {
            code: ErrorCode::WatchFailed,
            message: "x".into(),
            protocol: None
        })
    );
}

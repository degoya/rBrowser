use super::*;

#[test]
fn frames_are_one_json_line_each_and_read_back() {
    for frame in [
        Frame::Listening { port: 9222 },
        Frame::Open { id: 1 },
        Frame::data(7, b"GET / HTTP/1.1\r\n"),
        Frame::Close { id: 7 },
        Frame::Ping,
        Frame::Pong,
        Frame::Shutdown,
        Frame::Error {
            message: "no".to_owned(),
        },
    ] {
        let line = encode(&frame);
        assert!(line.ends_with('\n'));
        assert_eq!(line.matches('\n').count(), 1);
        assert_eq!(decode(&line).expect("decodes"), frame);
    }
}

#[test]
fn the_wire_names_are_snake_case_with_camel_case_fields() {
    assert_eq!(
        encode(&Frame::Open { id: 3 }),
        "{\"type\":\"open\",\"id\":3}\n"
    );
    assert_eq!(
        encode(&Frame::Listening { port: 1 }),
        "{\"type\":\"listening\",\"port\":1}\n"
    );
}

#[test]
fn data_carries_bytes_as_base64() {
    let frame = Frame::data(2, &[0, 255, 10, 13]);
    assert_eq!(
        frame,
        Frame::Data {
            id: 2,
            data: "AP8KDQ==".to_owned()
        }
    );
    assert_eq!(payload("AP8KDQ==").expect("decodes"), vec![0, 255, 10, 13]);
    assert!(payload("not base64!").is_err());
}

#[test]
fn a_full_chunk_fits_in_one_protocol_line() {
    let line = encode(&Frame::data(u32::MAX, &[0xff; CHUNK]));
    assert!(line.len() < crate::protocol::MAX_LINE, "{}", line.len());
}

#[test]
fn garbage_and_overlong_lines_are_refused() {
    assert!(decode("{\"type\":\"teleport\"}").is_err());
    assert!(decode("hello").is_err());
    let long = "x".repeat(crate::protocol::MAX_LINE + 1);
    assert!(decode(&long).is_err());
}

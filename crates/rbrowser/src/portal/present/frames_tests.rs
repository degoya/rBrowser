use super::*;

/// A masked client frame of `opcode` with `payload` (the mask key zero: the bytes stay readable).
fn frame(fin: bool, opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![(u8::from(fin) << 7) | opcode];
    match payload.len() {
        short @ 0..=125 => out.push(0x80 | u8::try_from(short).expect("short")),
        middle @ 126..=0xffff => {
            out.push(0x80 | 126);
            out.extend(u16::try_from(middle).expect("middle").to_be_bytes());
        }
        long => {
            out.push(0x80 | 127);
            out.extend((long as u64).to_be_bytes());
        }
    }
    out.extend([0, 0, 0, 0]);
    out.extend(payload);
    out
}

fn filtered(input: &[u8], chunk: usize) -> Result<Vec<u8>, Broken> {
    let mut filter = ControlOnly::default();
    let mut out = Vec::new();
    for piece in input.chunks(chunk.max(1)) {
        filter.feed(piece, &mut out)?;
    }
    Ok(out)
}

#[test]
fn a_viewer_data_frame_never_reaches_the_dev_server() {
    let invalidate = br#"{"type":"custom","event":"vite:invalidate","data":{"path":"/src/a.ts"}}"#;
    let ping = frame(true, 0x9, b"p");
    let close = frame(true, 0x8, &[0x03, 0xe8]);
    let mut input = Vec::new();
    input.extend(frame(true, 0x1, invalidate));
    input.extend(&ping);
    input.extend(frame(false, 0x2, &[7u8; 70_000]));
    input.extend(frame(true, 0x0, b"rest of it"));
    input.extend(&close);
    let mut expected = ping.clone();
    expected.extend(&close);
    // Whole, byte by byte and in odd chunks: the same control frames, nothing else.
    for chunk in [input.len(), 1, 3, 1000] {
        assert_eq!(
            filtered(&input, chunk),
            Ok(expected.clone()),
            "chunk {chunk}"
        );
    }
}

#[test]
fn a_frame_that_breaks_the_protocol_ends_the_upload() {
    let mut unmasked = frame(true, 0x1, b"x");
    unmasked[1] &= 0x7f;
    unmasked.truncate(2);
    unmasked.push(b'x');
    assert_eq!(filtered(&unmasked, 100), Err(Broken));
    assert_eq!(filtered(&frame(true, 0x9, &[0u8; 126]), 200), Err(Broken));
    assert_eq!(filtered(&frame(false, 0x9, b"p"), 100), Err(Broken));
}

#[tokio::test]
async fn the_copy_sends_the_control_frames_on() {
    let pong = frame(true, 0xa, b"");
    let mut input = frame(true, 0x1, b"vite:invalidate");
    input.extend(&pong);
    let mut from: &[u8] = &input;
    let mut to = Vec::new();
    let sent = copy_control_frames(ControlOnly::default(), &mut from, &mut to)
        .await
        .expect("copies");
    assert_eq!(to, pong);
    assert_eq!(sent, pong.len() as u64);
}

use tokio::io::{AsyncWriteExt, BufReader};

use super::*;

async fn lines(input: &[u8], max: usize) -> Vec<(LineRead, String)> {
    // A small buffer, so lines span several reads.
    let mut reader = BufReader::with_capacity(4, input);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    loop {
        let read = read_line(&mut reader, &mut buf, max).await.expect("reads");
        out.push((read, String::from_utf8_lossy(&buf).into_owned()));
        if read == LineRead::Eof {
            return out;
        }
    }
}

#[tokio::test]
async fn lines_split_at_newlines_across_reads() {
    assert_eq!(
        lines(b"{\"a\":1}\r\n{\"b\":22}\n", 64).await,
        vec![
            (LineRead::Line, "{\"a\":1}".to_owned()),
            (LineRead::Line, "{\"b\":22}".to_owned()),
            (LineRead::Eof, String::new()),
        ]
    );
}

#[tokio::test]
async fn a_long_line_is_skipped_and_the_next_one_read() {
    assert_eq!(
        lines(b"0123456789abcdef\nok\n", 8).await,
        vec![
            (LineRead::TooLong, String::new()),
            (LineRead::Line, "ok".to_owned()),
            (LineRead::Eof, String::new()),
        ]
    );
}

#[tokio::test]
async fn a_last_line_without_newline_is_dropped() {
    assert_eq!(
        lines(b"half", 64).await,
        vec![(LineRead::Eof, String::new())]
    );
}

#[tokio::test]
async fn a_line_written_in_pieces_arrives_whole() {
    let (mut writer, reader) = tokio::io::duplex(64);
    let task = tokio::spawn(async move {
        let mut reader = BufReader::new(reader);
        let mut buf = Vec::new();
        let read = read_line(&mut reader, &mut buf, 64).await.expect("reads");
        (read, buf)
    });
    writer.write_all(b"{\"id\"").await.expect("writes");
    writer.write_all(b":1}\n").await.expect("writes");
    let (read, buf) = task.await.expect("joins");
    assert_eq!(read, LineRead::Line);
    assert_eq!(buf, b"{\"id\":1}");
}

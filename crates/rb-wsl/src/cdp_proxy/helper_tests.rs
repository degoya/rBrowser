use std::time::Duration;

use tokio::io::BufReader;
use tokio::net::TcpStream;

use super::*;
use crate::cdp_proxy::frame::decode;
use crate::protocol::{Message, hello, new_nonce};

const LIMIT: Duration = Duration::from_secs(5);

async fn next_line<R: AsyncBufRead + Unpin>(reader: &mut R) -> String {
    let mut line = String::new();
    tokio::time::timeout(LIMIT, reader.read_line(&mut line))
        .await
        .expect("in time")
        .expect("reads");
    line
}

#[tokio::test]
async fn after_the_handshake_the_helper_listens_and_names_its_port() {
    let (ours, theirs) = tokio::io::duplex(1 << 16);
    let (input, output) = tokio::io::split(theirs);
    let running = tokio::spawn(serve_cdp_proxy(BufReader::new(input), output, "9.9.9", 0));
    let (read, mut write) = tokio::io::split(ours);
    let mut read = BufReader::new(read);
    let nonce = new_nonce();
    write
        .write_all(protocol::encode(&hello(&nonce)).as_bytes())
        .await
        .expect("writes");
    let welcome = protocol::decode(&next_line(&mut read).await).expect("a message");
    assert!(
        matches!(welcome, Message::Welcome { ref helper_version, .. } if helper_version == "9.9.9")
    );
    let Frame::Listening { port } = decode(&next_line(&mut read).await).expect("a frame") else {
        panic!("no listening frame");
    };
    assert_ne!(port, 0);
    // A client connects: the app hears `open`.
    let _client = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connects");
    assert_eq!(
        decode(&next_line(&mut read).await).expect("a frame"),
        Frame::Open { id: 1 }
    );
    write
        .write_all(frame::encode(&Frame::Shutdown).as_bytes())
        .await
        .expect("writes");
    let served = tokio::time::timeout(LIMIT, running)
        .await
        .expect("in time")
        .expect("joins");
    assert_eq!(served.expect("served"), Served::Shutdown);
}

#[tokio::test]
async fn a_taken_port_falls_back_to_a_free_one() {
    let taken = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let port = taken.local_addr().expect("address").port();
    let other = listen(port).await.expect("listens");
    assert_ne!(other.local_addr().expect("address").port(), port);
}

#[tokio::test]
async fn a_bad_hello_is_refused_and_nothing_listens() {
    let (ours, theirs) = tokio::io::duplex(1 << 16);
    let (input, output) = tokio::io::split(theirs);
    let running = tokio::spawn(serve_cdp_proxy(BufReader::new(input), output, "1", 0));
    let (mut read, mut write) = tokio::io::split(ours);
    write
        .write_all(b"{\"type\":\"ping\"}\n")
        .await
        .expect("writes");
    let served = tokio::time::timeout(LIMIT, running)
        .await
        .expect("in time")
        .expect("joins");
    assert!(matches!(
        served.expect("served"),
        Served::HandshakeFailed(_)
    ));
    let mut answer = vec![0u8; 256];
    let read = read.read(&mut answer).await.expect("reads");
    assert!(String::from_utf8_lossy(&answer[..read]).contains("\"type\":\"error\""));
}

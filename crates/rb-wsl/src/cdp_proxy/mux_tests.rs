use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, DuplexStream};

use super::*;

const LIMIT: Duration = Duration::from_secs(5);

/// An echo server on 127.0.0.1; its address.
async fn echo_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let address = listener.local_addr().expect("address");
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let (mut read, mut write) = stream.into_split();
                let _ = tokio::io::copy(&mut read, &mut write).await;
            });
        }
    });
    address
}

/// A server that answers every connection with `bye` and closes it.
async fn bye_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let address = listener.local_addr().expect("address");
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let _ = stream.write_all(b"bye").await;
            let _ = stream.shutdown().await;
        }
    });
    address
}

fn halves(
    end: DuplexStream,
) -> (
    BufReader<tokio::io::ReadHalf<DuplexStream>>,
    tokio::io::WriteHalf<DuplexStream>,
) {
    let (read, write) = tokio::io::split(end);
    (BufReader::new(read), write)
}

/// Both ends of a tunnel in this process, the app's toward `proxy`; the helper's port.
async fn tunnel(proxy: SocketAddr) -> u16 {
    let (helper_end, app_end) = tokio::io::duplex(1 << 20);
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let port = listener.local_addr().expect("address").port();
    let (input, output) = halves(helper_end);
    tokio::spawn(run(input, output, Side::Helper(listener)));
    let (input, output) = halves(app_end);
    tokio::spawn(run(input, output, Side::App(proxy)));
    port
}

async fn round_trip(port: u16, payload: &[u8]) -> Vec<u8> {
    let mut client = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connects");
    client.write_all(payload).await.expect("writes");
    let mut back = vec![0u8; payload.len()];
    tokio::time::timeout(LIMIT, client.read_exact(&mut back))
        .await
        .expect("in time")
        .expect("reads");
    back
}

#[tokio::test]
async fn bytes_cross_the_tunnel_both_ways() {
    let port = tunnel(echo_server().await).await;
    assert_eq!(
        round_trip(port, b"GET /json/version HTTP/1.1\r\n\r\n").await,
        b"GET /json/version HTTP/1.1\r\n\r\n"
    );
}

#[tokio::test]
async fn several_connections_share_the_tunnel_without_mixing() {
    let port = tunnel(echo_server().await).await;
    let big: Vec<u8> = (0..200_000u32).map(|n| (n % 251) as u8).collect();
    let small = b"small".to_vec();
    let (one, two, three) = tokio::join!(
        round_trip(port, &big),
        round_trip(port, &small),
        round_trip(port, b"third")
    );
    assert_eq!(one, big);
    assert_eq!(two, small);
    assert_eq!(three, b"third");
}

#[tokio::test]
async fn a_closed_proxy_connection_closes_the_client_after_its_bytes() {
    let port = tunnel(bye_server().await).await;
    let mut client = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connects");
    let mut answer = Vec::new();
    tokio::time::timeout(LIMIT, client.read_to_end(&mut answer))
        .await
        .expect("in time")
        .expect("reads");
    assert_eq!(answer, b"bye");
}

#[tokio::test]
async fn an_unreachable_proxy_closes_the_client() {
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        listener.local_addr().expect("address")
    };
    let port = tunnel(closed).await;
    let mut client = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connects");
    let mut answer = Vec::new();
    let read = tokio::time::timeout(LIMIT, client.read_to_end(&mut answer))
        .await
        .expect("in time");
    assert!(read.is_err() || answer.is_empty());
}

/// The app end fed by hand: its frames out as lines.
async fn app_by_hand(lines: &str) -> (io::Result<Ended>, String) {
    let (ours, theirs) = tokio::io::duplex(1 << 16);
    let (input, output) = halves(theirs);
    let proxy = echo_server().await;
    let running = tokio::spawn(run(input, output, Side::App(proxy)));
    let (mut read, mut write) = tokio::io::split(ours);
    write.write_all(lines.as_bytes()).await.expect("writes");
    let ended = tokio::time::timeout(LIMIT, running)
        .await
        .expect("in time")
        .expect("joins");
    drop(write);
    let mut written = String::new();
    let _ = tokio::time::timeout(LIMIT, read.read_to_string(&mut written)).await;
    (ended, written)
}

#[tokio::test]
async fn ping_is_answered_bad_lines_are_named_and_shutdown_ends() {
    let (ended, written) =
        app_by_hand("{\"type\":\"ping\"}\nnonsense\n{\"type\":\"shutdown\"}\n").await;
    assert_eq!(ended.expect("ends"), Ended::Shutdown);
    assert!(written.contains("{\"type\":\"pong\"}"), "{written}");
    assert!(written.contains("{\"type\":\"error\""), "{written}");
}

#[tokio::test]
async fn data_for_an_unknown_connection_is_answered_with_close() {
    let line = format!(
        "{}{}",
        crate::cdp_proxy::frame::encode(&Frame::data(9, b"x")),
        crate::cdp_proxy::frame::encode(&Frame::Shutdown)
    );
    let (_, written) = app_by_hand(&line).await;
    assert!(
        written.contains("{\"type\":\"close\",\"id\":9}"),
        "{written}"
    );
}

#[test]
fn ids_skip_those_in_use_and_never_are_zero() {
    let connections = HashMap::new();
    assert_eq!(next_free(&connections, 0), 1);
    assert_eq!(next_free(&connections, u32::MAX), 1);
}

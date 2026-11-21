//! RB-096-05: a viewer of the presentation gets the dev server's socket, but sends it nothing
//! but control frames - no `vite:invalidate`, no `import.meta.hot.send`.

use rb_cdp::request::parse_head;
use tokio::net::TcpListener;

use super::tests::{never, pair};
use super::*;

const PORTAL: &str = "https://quick-brown-fox.trycloudflare.com";

/// A masked client frame with a zero mask key.
fn frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![
        0x80 | opcode,
        0x80 | u8::try_from(payload.len()).expect("short"),
    ];
    out.extend([0, 0, 0, 0]);
    out.extend(payload);
    out
}

/// Upgrades a socket through `map`, sends `frames` from the device and answers with what the
/// upstream received after the upgrade's head.
async fn upstream_receives(map: impl FnOnce(&str) -> OriginMap, frames: Vec<u8>) -> Vec<u8> {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    let received = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accepts");
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while head_end(&head).is_none() && stream.read(&mut byte).await.unwrap_or(0) == 1 {
            head.push(byte[0]);
        }
        stream
            .write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n")
            .await
            .expect("writes");
        let mut rest = Vec::new();
        let _ = stream.read_to_end(&mut rest).await;
        rest
    });
    let (mut device, portal_side) = pair().await;
    let request = parse_head(
        b"GET /hmr HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
    )
    .expect("parses");
    let upstream = Upstream::parse(&origin).expect("http");
    let map = map(&origin);
    let forwarding = tokio::spawn(async move {
        forward(portal_side, request, Vec::new(), &upstream, &map, never()).await
    });
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while head_end(&head).is_none() && device.read(&mut byte).await.unwrap_or(0) == 1 {
        head.push(byte[0]);
    }
    assert!(String::from_utf8_lossy(&head).starts_with("HTTP/1.1 101 "));
    device.write_all(&frames).await.expect("writes");
    device.shutdown().await.expect("shuts down");
    let received = tokio::time::timeout(Duration::from_secs(5), received)
        .await
        .expect("the upstream saw the end")
        .expect("joins");
    drop(device);
    let _ = tokio::time::timeout(Duration::from_secs(5), forwarding).await;
    received
}

#[tokio::test]
async fn a_viewer_sends_the_dev_server_control_frames_only() {
    let invalidate = frame(0x1, br#"{"type":"custom","event":"vite:invalidate"}"#);
    let ping = frame(0x9, b"p");
    let mut frames = invalidate.clone();
    frames.extend(&ping);
    let public = upstream_receives(
        |origin| OriginMap::new(origin, PORTAL).public(),
        frames.clone(),
    );
    assert_eq!(public.await, ping);
    // A paired device of the LAN portal drives its page: its frames pass as they are.
    let lan = upstream_receives(|origin| OriginMap::new(origin, PORTAL), frames.clone());
    assert_eq!(lan.await, frames);
}

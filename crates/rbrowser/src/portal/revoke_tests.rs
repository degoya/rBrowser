//! What ends with a device or the portal (Charge 10 review M1, L2, L3): forwarded upgrades,
//! the sockets a device may hold, the connections an address may hold, the secrets in the log
//! mask.

use super::tests::{get, pair, socket, start, status};
use super::*;
use crate::portal::state::MAX_DEVICE_SOCKETS;

/// An upstream that switches protocols on every connection, then echoes until it closes.
async fn echo_upstream() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while head_end(&head).is_none() && stream.read(&mut byte).await.unwrap_or(0) == 1 {
                    head.push(byte[0]);
                }
                let _ = stream
                    .write_all(
                        b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n\
                          Connection: Upgrade\r\n\r\n",
                    )
                    .await;
                let mut buffer = [0u8; 64];
                while let Ok(read) = stream.read(&mut buffer).await {
                    if read == 0 || stream.write_all(&buffer[..read]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    origin
}

/// A device's upgrade through the portal (Vite's HMR socket), once it echoed.
async fn hmr(server: &PortalServer, cookie: &str) -> TcpStream {
    let mut stream = TcpStream::connect(server.address())
        .await
        .expect("connects");
    let request = format!(
        "GET /hmr HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         {cookie}\r\n",
        server.address()
    );
    stream.write_all(request.as_bytes()).await.expect("writes");
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while head_end(&head).is_none() && stream.read(&mut byte).await.unwrap_or(0) == 1 {
        head.push(byte[0]);
    }
    assert!(
        String::from_utf8_lossy(&head).starts_with("HTTP/1.1 101 "),
        "{}",
        String::from_utf8_lossy(&head)
    );
    stream.write_all(b"ping").await.expect("writes");
    let mut echo = [0u8; 4];
    stream.read_exact(&mut echo).await.expect("echoes");
    stream
}

/// Whether `stream` is closed by the other end within five seconds.
async fn closes(stream: &mut TcpStream) -> bool {
    let mut rest = Vec::new();
    matches!(
        tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut rest)).await,
        Ok(Ok(_) | Err(_))
    )
}

async fn running_with_hmr() -> (Arc<PortalServer>, String) {
    let origin = echo_upstream().await;
    let (server, _) = start(Duration::from_secs(300), 5);
    let cookie = pair(&server).await;
    server.set_page(Some(format!("{origin}/")), vec![origin]);
    (server, cookie)
}

#[tokio::test]
async fn a_forwarded_upgrade_ends_when_its_device_is_disconnected() {
    let (server, cookie) = running_with_hmr().await;
    // A second device's connection stays.
    let other_cookie = pair(&server).await;
    let mut first = hmr(&server, &cookie).await;
    let mut second = hmr(&server, &other_cookie).await;
    let device = server.devices()[0].info.id;
    assert!(server.disconnect(device));
    assert!(
        closes(&mut first).await,
        "the HMR socket outlived the disconnect"
    );
    second.write_all(b"pong").await.expect("writes");
    let mut echo = [0u8; 4];
    second
        .read_exact(&mut echo)
        .await
        .expect("the other device stays connected");
    assert_eq!(&echo, b"pong");
}

#[tokio::test]
async fn a_forwarded_upgrade_ends_when_the_portal_stops() {
    let (server, cookie) = running_with_hmr().await;
    let mut stream = hmr(&server, &cookie).await;
    server.stop();
    assert!(
        closes(&mut stream).await,
        "the HMR socket outlived the stop"
    );
}

#[tokio::test]
async fn an_address_holds_a_bounded_number_of_connections() {
    let (server, _) = start(Duration::from_secs(300), 5);
    let mut idle = Vec::new();
    for _ in 0..MAX_PER_ADDRESS {
        idle.push(
            TcpStream::connect(server.address())
                .await
                .expect("connects"),
        );
    }
    // Give the listener the time to take them all.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut refused = TcpStream::connect(server.address())
        .await
        .expect("connects");
    assert!(
        closes(&mut refused).await,
        "one over the cap is closed at once"
    );
    drop(idle);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let answer = get(&server, "/", "").await;
    assert_eq!(status(&answer), 403, "slots come back: {answer}");
}

#[tokio::test]
async fn a_device_holds_a_bounded_number_of_sockets() {
    let (server, _) = start(Duration::from_secs(300), 5);
    let cookie = pair(&server).await;
    let mut open = Vec::new();
    for _ in 0..MAX_DEVICE_SOCKETS {
        open.push(socket(&server, &cookie).await);
    }
    let device = server.devices()[0].info.id;
    assert!(server.open_socket(device).is_none(), "one over the cap");
    drop(open);
}

fn masked(secret: &str) -> bool {
    !rb_diag::redact(&format!("x {secret} y")).contains(secret)
}

#[tokio::test]
async fn the_portal_secrets_leave_the_log_mask_when_given_up() {
    let (server, _) = start(Duration::from_secs(300), 5);
    let first = server.issue_pairing().expect("random");
    let first_token = first.url.rsplit('=').next().expect("a token").to_owned();
    assert!(masked(&first_token));
    let second = server.issue_pairing().expect("random");
    let second_token = second.url.rsplit('=').next().expect("a token").to_owned();
    assert!(!masked(&first_token), "a replaced token is forgotten");
    assert!(masked(&second_token));

    let cookie = pair(&server).await;
    let session = cookie
        .trim_end()
        .rsplit('=')
        .next()
        .expect("a session")
        .to_owned();
    assert!(masked(&session));
    let device = server.devices()[0].info.id;
    assert!(server.disconnect(device));
    assert!(!masked(&session), "a revoked session is forgotten");

    let cookie = pair(&server).await;
    let session = cookie
        .trim_end()
        .rsplit('=')
        .next()
        .expect("a session")
        .to_owned();
    let token = server.issue_pairing().expect("random");
    let token = token.url.rsplit('=').next().expect("a token").to_owned();
    assert!(masked(&session) && masked(&token));
    server.stop();
    assert!(
        !masked(&session) && !masked(&token),
        "a stop forgets them all"
    );
    assert!(server.devices().is_empty(), "a stop revokes every session");
}

//! A device's WebSocket (`/__rbrowser/ws`, RB-080-05): the portal agent of each page on the
//! device opens one after the cookie proved the device. The portal sends what the device
//! replays ([`super::relay::ToDevice`]); the device sends only [`super::relay::Report`]s, small
//! and counted - it can drive nothing in rBrowser. The socket ends when the device is
//! disconnected (its sender is dropped), the portal stops, or the device leaves.

use std::sync::Arc;

use futures_util::{SinkExt as _, StreamExt as _};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::{Role, WebSocketConfig};

use super::pairing::DeviceId;
use super::relay::{MAX_REPORT, parse_report};
use super::server::PortalServer;

/// Runs the socket of `device` on `stream`, whose upgrade asked with `key`.
pub async fn run(mut stream: TcpStream, key: &str, device: DeviceId, server: &Arc<PortalServer>) {
    let Some((id, mut outgoing)) = server.open_socket(device) else {
        return;
    };
    let accept = derive_accept_key(key.as_bytes());
    let switching = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept}\r\n\r\n"
    );
    if stream.write_all(switching.as_bytes()).await.is_err() {
        server.close_socket(device, id);
        return;
    }
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_REPORT))
        .max_frame_size(Some(MAX_REPORT));
    let ws = WebSocketStream::from_raw_socket(stream, Role::Server, Some(config)).await;
    let (mut sink, mut incoming) = ws.split();
    let mut stopped = server.stopped.subscribe();
    loop {
        if *stopped.borrow() {
            break;
        }
        tokio::select! {
            message = outgoing.recv() => match message {
                Some(text) => {
                    if sink.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                // Disconnected: the device's sender is gone.
                None => break,
            },
            frame = incoming.next() => match frame {
                Some(Ok(Message::Text(text))) => {
                    if let Some(report) = parse_report(text.as_str()) {
                        server.note(device, &report);
                    }
                }
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            changed = stopped.changed() => {
                if changed.is_err() {
                    break;
                }
            }
        }
    }
    let _ = sink.close().await;
    server.close_socket(device, id);
}

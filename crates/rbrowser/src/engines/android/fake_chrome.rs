//! A Chrome on a device as the tests see it: a WebSocket on 127.0.0.1 that answers the CDP
//! commands an Android view sends, records them, and reports each navigation of the tab; its
//! `/json/version` answers over plain HTTP on the same port, as Chrome's does.

use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

/// The fake's socket and what it was sent.
pub(crate) struct FakeChrome {
    /// `ws://127.0.0.1:<port>/devtools/browser`.
    pub url: String,
    /// The port.
    pub port: u16,
    /// Every command, `method` with its `params` and `sessionId`.
    pub commands: Arc<Mutex<Vec<Value>>>,
    /// The methods answered with an error.
    refused: Arc<Mutex<Vec<String>>>,
}

impl FakeChrome {
    /// The methods sent so far.
    pub fn methods(&self) -> Vec<String> {
        crate::cdp::lock(&self.commands)
            .iter()
            .filter_map(|command| command["method"].as_str().map(ToOwned::to_owned))
            .collect()
    }

    /// Answers `method` with an error from now on.
    pub fn refuse(&self, method: &str) {
        crate::cdp::lock(&self.refused).push(method.to_owned());
    }
}

/// Chrome's `/json/version` as Chrome on Android answers it: a host of its own and the
/// browser target without an id.
const VERSION: &str = r#"{"Android-Package":"com.android.chrome","Browser":"Chrome/131.0","webSocketDebuggerUrl":"ws://localhost/devtools/browser"}"#;

/// One connection: `/json/version` over plain HTTP, else the browser's WebSocket.
async fn serve(
    mut stream: TcpStream,
    seen: Arc<Mutex<Vec<Value>>>,
    refused: Arc<Mutex<Vec<String>>>,
) {
    let mut head = [0u8; 17];
    if stream.peek(&mut head).await.is_ok() && head.starts_with(b"GET /json/version") {
        let mut request = [0u8; 1024];
        let _read = stream.read(&mut request).await;
        let answer = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{VERSION}",
            VERSION.len()
        );
        let _written = stream.write_all(answer.as_bytes()).await;
        // As Chrome's server does: the connection stays open until the client closes it.
        let _closed = stream.read(&mut request).await;
        return;
    }
    let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await else {
        return;
    };
    while let Some(Ok(Message::Text(text))) = ws.next().await {
        let command: Value = serde_json::from_str(text.as_str()).expect("json");
        crate::cdp::lock(&seen).push(command.clone());
        let method = command["method"].as_str().unwrap_or_default();
        let result = match method {
            "Target.createTarget" => json!({ "targetId": "T1" }),
            "Target.attachToTarget" => json!({ "sessionId": "S1" }),
            "Page.getNavigationHistory" => json!({
                "currentIndex": 1,
                "entries": [
                    { "id": 5, "url": "http://localhost:5000/back" },
                    { "id": 6, "url": "https://example.test/" }
                ]
            }),
            "Page.captureScreenshot" => json!({ "data": "iVBORw0KGgo=" }),
            _ => json!({}),
        };
        if method == "Page.navigate" {
            let event = json!({
                "method": "Page.frameNavigated",
                "sessionId": "S1",
                "params": { "frame": { "id": "F", "url": command["params"]["url"] } }
            });
            let _sent = ws.send(Message::text(event.to_string())).await;
        }
        let answer = if crate::cdp::lock(&refused).iter().any(|name| name == method) {
            json!({ "id": command["id"], "error": { "code": -32000, "message": "refused" } })
        } else {
            json!({ "id": command["id"], "result": result })
        };
        if ws.send(Message::text(answer.to_string())).await.is_err() {
            return;
        }
    }
}

/// Starts the fake; it serves every connection.
pub(crate) async fn start() -> FakeChrome {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let commands = Arc::new(Mutex::new(Vec::new()));
    let refused = Arc::new(Mutex::new(Vec::new()));
    let (seen, refusing) = (commands.clone(), refused.clone());
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(serve(stream, seen.clone(), refusing.clone()));
        }
    });
    FakeChrome {
        url: format!("ws://127.0.0.1:{port}/devtools/browser"),
        port,
        commands,
        refused,
    }
}

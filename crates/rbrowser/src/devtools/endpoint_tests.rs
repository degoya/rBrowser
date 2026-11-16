use std::time::Duration;

use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;

use super::*;
use crate::cdp::fake::FakeBrowser;

struct Setup {
    fake: Arc<FakeBrowser>,
    endpoint: Arc<Endpoint>,
    ended: Arc<Mutex<Vec<ViewId>>>,
}

fn setup() -> Setup {
    let (fake, router) = FakeBrowser::connect();
    let ended = Arc::new(Mutex::new(Vec::new()));
    let record = ended.clone();
    let endpoint = Endpoint::start(
        &Handle::current(),
        router,
        Arc::new(move |view| lock(&record).push(view)),
    )
    .expect("listens");
    Setup {
        fake,
        endpoint,
        ended,
    }
}

/// The `ws://` URL of a ticket's frontend URL.
fn ws_url(ticket: &Ticket) -> String {
    let (_, address) = ticket
        .frontend_url()
        .split_once("?ws=")
        .expect("a ws parameter");
    format!("ws://{address}")
}

async fn connect(
    url: &str,
    origin: Option<&str>,
) -> Result<WsStream, tokio_tungstenite::tungstenite::Error> {
    let mut request = url.into_client_request()?;
    if let Some(origin) = origin {
        request
            .headers_mut()
            .insert("origin", HeaderValue::from_str(origin).expect("header"));
    }
    let port = request.uri().port_u16().expect("a port");
    let stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connects");
    tokio_tungstenite::client_async(request, stream)
        .await
        .map(|(ws, _)| ws)
}

async fn next_json(ws: &mut WsStream) -> Value {
    let frame = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("a message in time")
        .expect("open")
        .expect("no error");
    serde_json::from_str(frame.to_text().expect("text")).expect("json")
}

async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..400 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

#[tokio::test]
async fn the_frontend_url_points_the_hosted_frontend_at_the_endpoint() {
    let setup = setup();
    let ticket = setup.endpoint.grant(ViewId(1), "T1").expect("ticket");
    let prefix = format!(
        "devtools://devtools/bundled/inspector.html?ws=127.0.0.1:{}/devtools/",
        setup.endpoint.port()
    );
    assert!(ticket.frontend_url().starts_with(&prefix));
    assert_eq!(ticket.frontend_url().len(), prefix.len() + 64);
    assert_eq!(format!("{ticket:?}"), "Ticket(<redacted>)");
}

#[tokio::test]
async fn a_connection_reaches_its_page_and_hears_its_events() {
    let setup = setup();
    let ticket = setup.endpoint.grant(ViewId(2), "T2").expect("ticket");
    let mut ws = connect(&ws_url(&ticket), Some(FRONTEND_ORIGIN))
        .await
        .expect("admitted");
    until(|| !setup.fake.sent("Target.attachToTarget").is_empty()).await;
    let attach = &setup.fake.sent("Target.attachToTarget")[0];
    assert_eq!(
        attach["params"],
        json!({ "targetId": "T2", "flatten": true })
    );
    ws.send(Message::text(
        json!({ "id": 41, "method": "DOM.getDocument" }).to_string(),
    ))
    .await
    .expect("sends");
    assert_eq!(next_json(&mut ws).await, json!({ "id": 41, "result": {} }));
    let sent = &setup.fake.sent("DOM.getDocument")[0];
    assert_eq!(sent["sessionId"], "S1");
    setup.fake.emit(json!({
        "method": "DOM.documentUpdated",
        "params": {},
        "sessionId": "S1"
    }));
    setup.fake.emit(json!({
        "method": "DOM.documentUpdated",
        "params": { "other": true },
        "sessionId": "S-ELSEWHERE"
    }));
    assert_eq!(
        next_json(&mut ws).await,
        json!({ "method": "DOM.documentUpdated", "params": {} })
    );
    ws.send(Message::text(
        json!({ "id": 42, "method": "Target.getTargets" }).to_string(),
    ))
    .await
    .expect("sends");
    let refused = next_json(&mut ws).await;
    assert_eq!(refused["id"], 42);
    assert_eq!(refused["error"]["message"], "Not allowed");
    assert!(setup.fake.sent("Target.getTargets").is_empty());
}

#[tokio::test]
async fn a_ticket_admits_one_connection_only() {
    let setup = setup();
    let ticket = setup.endpoint.grant(ViewId(1), "T1").expect("ticket");
    let url = ws_url(&ticket);
    let _first = connect(&url, None).await.expect("admitted");
    assert!(connect(&url, None).await.is_err(), "the token was used");
}

#[tokio::test]
async fn a_wrong_token_another_path_or_a_web_origin_is_refused() {
    let setup = setup();
    let ticket = setup.endpoint.grant(ViewId(1), "T1").expect("ticket");
    let url = ws_url(&ticket);
    let port = setup.endpoint.port();
    let wrong = format!("ws://127.0.0.1:{port}/devtools/{}", "0".repeat(64));
    assert!(connect(&wrong, None).await.is_err());
    assert!(
        connect(&format!("ws://127.0.0.1:{port}/json/list"), None)
            .await
            .is_err()
    );
    assert!(connect(&url, Some("https://evil.test")).await.is_err());
    // None of those used the ticket up.
    assert!(connect(&url, Some(FRONTEND_ORIGIN)).await.is_ok());
}

#[tokio::test]
async fn a_dropped_ticket_admits_nobody() {
    let setup = setup();
    let ticket = setup.endpoint.grant(ViewId(1), "T1").expect("ticket");
    let url = ws_url(&ticket);
    drop(ticket);
    assert!(connect(&url, None).await.is_err());
    assert!(setup.fake.sent("Target.attachToTarget").is_empty());
}

#[tokio::test]
async fn dropping_the_ticket_ends_the_connection_detaches_and_reports_the_view() {
    let setup = setup();
    let ticket = setup.endpoint.grant(ViewId(3), "T3").expect("ticket");
    let mut ws = connect(&ws_url(&ticket), None).await.expect("admitted");
    until(|| !setup.fake.sent("Target.attachToTarget").is_empty()).await;
    drop(ticket);
    until(|| lock(&setup.ended).as_slice() == [ViewId(3)]).await;
    let detach = &setup.fake.sent("Target.detachFromTarget")[0];
    assert_eq!(detach["params"], json!({ "sessionId": "S1" }));
    let closed = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("in time");
    assert!(matches!(
        closed,
        None | Some(Ok(Message::Close(_))) | Some(Err(_))
    ));
}

#[tokio::test]
async fn the_frontend_closing_ends_the_connection_and_reports_the_view() {
    let setup = setup();
    let ticket = setup.endpoint.grant(ViewId(4), "T4").expect("ticket");
    let mut ws = connect(&ws_url(&ticket), None).await.expect("admitted");
    until(|| !setup.fake.sent("Target.attachToTarget").is_empty()).await;
    ws.close(None).await.expect("closes");
    until(|| lock(&setup.ended).as_slice() == [ViewId(4)]).await;
    assert_eq!(setup.fake.sent("Target.detachFromTarget").len(), 1);
}

#[tokio::test]
async fn the_page_detaching_ends_the_connection() {
    let setup = setup();
    let ticket = setup.endpoint.grant(ViewId(5), "T5").expect("ticket");
    let _ws = connect(&ws_url(&ticket), None).await.expect("admitted");
    until(|| !setup.fake.sent("Target.attachToTarget").is_empty()).await;
    // Let the relay subscribe and settle before the browser drops the session.
    tokio::time::sleep(Duration::from_millis(20)).await;
    setup.fake.emit(json!({
        "method": "Target.detachedFromTarget",
        "params": { "sessionId": "S1" }
    }));
    until(|| lock(&setup.ended).as_slice() == [ViewId(5)]).await;
}

#[tokio::test]
async fn an_inspect_request_reaches_the_frontend_once_it_enabled_the_overlay() {
    let setup = setup();
    let ticket = setup.endpoint.grant(ViewId(2), "T2").expect("ticket");
    let mut ws = connect(&ws_url(&ticket), Some(FRONTEND_ORIGIN))
        .await
        .expect("admitted");
    until(|| !setup.fake.sent("Target.attachToTarget").is_empty()).await;
    // Requested before the frontend is ready: it waits.
    setup.endpoint.inspect().request(ViewId(2), 42);
    ws.send(Message::text(
        json!({ "id": 1, "method": "DOM.enable" }).to_string(),
    ))
    .await
    .expect("sends");
    assert_eq!(next_json(&mut ws).await, json!({ "id": 1, "result": {} }));
    ws.send(Message::text(
        json!({ "id": 2, "method": "Overlay.enable" }).to_string(),
    ))
    .await
    .expect("sends");
    assert_eq!(next_json(&mut ws).await, json!({ "id": 2, "result": {} }));
    assert_eq!(next_json(&mut ws).await, Relay::inspect_event(42));
    // Ready now: a request arrives at once, another view's never.
    setup.endpoint.inspect().request(ViewId(9), 1);
    setup.endpoint.inspect().request(ViewId(2), 43);
    assert_eq!(next_json(&mut ws).await, Relay::inspect_event(43));
    assert_eq!(setup.endpoint.inspect().pending(ViewId(9)), Some(1));
    until(|| setup.endpoint.inspect().pending(ViewId(2)).is_none()).await;
    // The overlay off again: a request waits, nothing is pushed to a frontend that ignores it.
    ws.send(Message::text(
        json!({ "id": 3, "method": "Overlay.disable" }).to_string(),
    ))
    .await
    .expect("sends");
    assert_eq!(next_json(&mut ws).await, json!({ "id": 3, "result": {} }));
    setup.endpoint.inspect().request(ViewId(2), 44);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(setup.endpoint.inspect().pending(ViewId(2)), Some(44));
}

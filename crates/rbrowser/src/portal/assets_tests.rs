//! The asset listeners end to end over real sockets on 127.0.0.1 (RB-090-65): a Laravel-like
//! page on one upstream loads its style sheet and module script from a Vite-like upstream on
//! another; both reach the device through the portal, and every refusal path holds.

use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use rb_cdp::request::head_end;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use super::*;
use crate::portal::inject::AGENT_TAG;
use crate::portal::origins::{self, asset_origins};
use crate::portal::pairing::COOKIE_NAME;
use crate::portal::server::Options;

fn start() -> Arc<PortalServer> {
    let changes = Arc::new(AtomicUsize::new(0));
    PortalServer::start(
        &Handle::current(),
        Options {
            ip: Ipv4Addr::LOCALHOST,
            port: None,
            pairing_ttl: Duration::from_secs(300),
            max_devices: 5,
            agent: Arc::from("/*agent*/"),
            on_change: Arc::new(move || {
                changes.fetch_add(1, Ordering::SeqCst);
            }),
        },
    )
    .expect("listens")
}

/// Sends `request` to `address`; the whole answer.
async fn ask(address: &str, request: &str) -> String {
    let mut stream = TcpStream::connect(address).await.expect("connects");
    stream.write_all(request.as_bytes()).await.expect("writes");
    let mut answer = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut answer)).await;
    String::from_utf8_lossy(&answer).into_owned()
}

/// `method path` on the listener at `authority` (its own `Host`) with `extra` header lines.
async fn call(authority: &str, method: &str, path: &str, extra: &str) -> String {
    let request = format!("{method} {path} HTTP/1.1\r\nHost: {authority}\r\n{extra}\r\n");
    ask(authority, &request).await
}

fn status(answer: &str) -> u16 {
    answer
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0)
}

/// The body of `answer`; a chunked one (rewritten as it streamed, RB-090-72) decoded.
fn body(answer: &str) -> String {
    let (head, body) = answer.split_once("\r\n\r\n").unwrap_or(("", ""));
    if head.contains("Transfer-Encoding: chunked") {
        let decoded = crate::portal::inject::decode_chunked(body.as_bytes()).expect("whole chunks");
        String::from_utf8(decoded).expect("utf-8")
    } else {
        body.to_owned()
    }
}

/// Pairs a device (not contacted yet); its cookie header line.
async fn pair(server: &PortalServer) -> String {
    let pairing = server.issue_pairing().expect("random");
    let path = pairing.url.trim_start_matches(server.origin()).to_owned();
    let answer = call(
        &server.address().to_string(),
        "GET",
        &path,
        "User-Agent: Phone\r\n",
    )
    .await;
    let cookie = answer
        .lines()
        .find_map(|line| line.strip_prefix("Set-Cookie: "))
        .expect("a cookie")
        .split(';')
        .next()
        .expect("a pair")
        .to_owned();
    format!("Cookie: {cookie}\r\n")
}

fn answer(content_type: &str, extra: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{extra}\r\n{body}",
        body.len()
    )
}

type Route = Arc<dyn Fn(&str, &str) -> String + Send + Sync>;

/// An upstream that answers each request head with `route(own origin, head)` and hands the
/// heads it saw out.
async fn upstream(route: Route) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    let (seen, heads) = mpsc::channel(16);
    let own = origin.clone();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while head_end(&head).is_none() && stream.read(&mut byte).await.unwrap_or(0) == 1 {
                head.push(byte[0]);
            }
            let head = String::from_utf8_lossy(&head).into_owned();
            let reply = route(&own, &head);
            let _ = seen.send(head).await;
            let _ = stream.write_all(reply.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    });
    (origin, heads)
}

/// The Vite-like dev server: a style sheet and a module script that name it absolutely
/// (`server.origin`) and relatively; CORS reflects the request's origin, as Vite does for one
/// it admits.
fn vite() -> Route {
    Arc::new(|own: &str, head: &str| {
        let cors = head
            .lines()
            .find_map(|line| line.strip_prefix("Origin: "))
            .map(|origin| format!("Access-Control-Allow-Origin: {origin}\r\n"))
            .unwrap_or_default();
        if head.starts_with("GET /app.css ") {
            answer(
                "text/css",
                "",
                &format!("a{{background:url({own}/img.png)}} b{{background:url(/rel.png)}}"),
            )
        } else {
            answer(
                "text/javascript",
                &cors,
                &format!("import \"{own}/@vite/env\";import \"/node_modules/.vite/x.js\";"),
            )
        }
    })
}

/// The portal, its asset listener, and what the tests need of them.
struct Setup {
    server: Arc<PortalServer>,
    /// The device's cookie header line.
    cookie: String,
    /// The asset listener's `ip:port`.
    assets: String,
    /// The page's upstream origin.
    page: String,
    /// The dev server's upstream origin.
    dev: String,
    /// The request heads the dev server saw.
    dev_heads: mpsc::Receiver<String>,
}

fn listed(page: &str, dev: &str) -> origins::AssetOrigins {
    let list = [dev.to_owned()];
    asset_origins([(page, list.as_slice())])
}

/// A page that names its dev server, the dev server, the portal with both and a device paired
/// (not contacted yet).
async fn setup() -> Setup {
    let (dev, dev_heads) = upstream(vite()).await;
    let html = format!(
        "<head><link rel=\"stylesheet\" href=\"{dev}/app.css\">\
         <script type=\"module\" src=\"{dev}/app.js\"></script></head>"
    );
    let (page, _) = upstream(Arc::new(move |_: &str, _: &str| {
        answer("text/html", "", &html)
    }))
    .await;
    let server = start();
    let cookie = pair(&server).await;
    server.set_assets(listed(&page, &dev));
    server.set_page(Some(format!("{page}/")), vec![page.clone()]);
    let listeners = server.asset_listeners();
    assert_eq!(listeners.len(), 1, "{listeners:?}");
    assert_eq!(listeners[0].0, dev);
    let assets = listeners[0].1.trim_start_matches("http://").to_owned();
    Setup {
        server,
        cookie,
        assets,
        page,
        dev,
        dev_heads,
    }
}

#[tokio::test]
async fn a_page_and_its_dev_server_reach_the_device() {
    let Setup {
        server,
        cookie,
        assets,
        page,
        dev,
        mut dev_heads,
    } = setup().await;
    let main = server.address().to_string();
    let portal = server.origin().to_owned();
    let listener = format!("http://{assets}");

    let html = call(&main, "GET", "/", &cookie).await;
    assert_eq!(status(&html), 200, "{html}");
    assert!(body(&html).contains(AGENT_TAG), "{html}");
    assert!(
        body(&html).contains(&format!("href=\"{listener}/app.css\"")),
        "{html}"
    );
    assert!(
        body(&html).contains(&format!("src=\"{listener}/app.js\"")),
        "{html}"
    );
    assert!(!body(&html).contains(&dev), "{html}");

    // A style sheet carries the cookie (no-cors); its absolute URLs point at the listener.
    let css = call(&assets, "GET", "/app.css", &cookie).await;
    assert_eq!(status(&css), 200, "{css}");
    assert_eq!(
        body(&css),
        format!("a{{background:url({listener}/img.png)}} b{{background:url(/rel.png)}}")
    );
    assert!(!body(&css).contains(AGENT_TAG));
    let seen = dev_heads.recv().await.expect("the dev server was asked");
    assert!(seen.contains(&format!("Host: {}\r\n", dev.trim_start_matches("http://"))));
    assert!(!seen.contains(COOKIE_NAME), "{seen}");

    // A module script goes without credentials and with the page's portal origin: from the
    // device's address it is admitted, the dev server sees the page's origin, the device the
    // portal's.
    let js = call(&assets, "GET", "/app.js", &format!("Origin: {portal}\r\n")).await;
    assert_eq!(status(&js), 200, "{js}");
    assert_eq!(
        body(&js),
        format!("import \"{listener}/@vite/env\";import \"/node_modules/.vite/x.js\";")
    );
    assert!(
        js.contains(&format!("Access-Control-Allow-Origin: {portal}\r\n")),
        "{js}"
    );
    let seen = dev_heads.recv().await.expect("the dev server was asked");
    assert!(seen.contains(&format!("Origin: {page}\r\n")), "{seen}");
    server.stop();
}

#[tokio::test]
async fn the_asset_listener_refuses_what_is_not_the_device_page() {
    let Setup {
        server,
        cookie,
        assets,
        page,
        ..
    } = setup().await;
    let main = server.address().to_string();

    // No contact yet: the device never came back with its cookie.
    assert_eq!(status(&call(&assets, "GET", "/app.js", "").await), 403);
    assert_eq!(status(&call(&main, "GET", "/", &cookie).await), 200);
    assert_eq!(status(&call(&assets, "GET", "/app.js", "").await), 200);

    // A wrong cookie, a POST without one, a foreign host, a proxy request.
    let wrong = format!("Cookie: {COOKIE_NAME}=00ff\r\n");
    assert_eq!(status(&call(&assets, "GET", "/app.js", &wrong).await), 403);
    assert_eq!(status(&call(&assets, "POST", "/app.js", "").await), 403);
    let foreign = ask(
        &assets,
        &format!("GET /app.js HTTP/1.1\r\nHost: {main}\r\n{cookie}\r\n"),
    )
    .await;
    assert_eq!(
        status(&foreign),
        403,
        "the main listener's host is foreign here"
    );
    let proxy = ask(
        &assets,
        &format!("GET http://evil.example/ HTTP/1.1\r\nHost: {assets}\r\n{cookie}\r\n"),
    )
    .await;
    assert_eq!(status(&proxy), 403);

    // The portal's own paths stay on the main listener.
    assert_eq!(
        status(&call(&assets, "GET", "/__rbrowser/agent.js", &cookie).await),
        404
    );

    // Another environment is shown, which does not list the dev server.
    let other = "http://127.0.0.1:9".to_owned();
    server.set_page(Some(format!("{other}/")), vec![page.clone(), other]);
    assert_eq!(status(&call(&assets, "GET", "/app.js", &cookie).await), 403);
    server.set_page(Some(format!("{page}/")), vec![page.clone()]);
    assert_eq!(status(&call(&assets, "GET", "/app.js", &cookie).await), 200);

    // The device is disconnected: neither its cookie nor its address grants anything.
    let device = server.devices()[0].info.id;
    assert!(server.disconnect(device));
    assert_eq!(status(&call(&assets, "GET", "/app.js", &cookie).await), 403);
    assert_eq!(status(&call(&assets, "GET", "/app.js", "").await), 403);
    server.stop();
}

/// Whether nobody accepts at `authority` any more, within a bounded poll.
async fn closed(authority: &str) -> bool {
    for _ in 0..100 {
        if TcpStream::connect(authority).await.is_err() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

#[tokio::test]
async fn a_listener_goes_with_its_origin_and_with_the_portal() {
    let Setup {
        server,
        cookie,
        assets,
        page,
        dev,
        ..
    } = setup().await;
    let main = server.address().to_string();
    assert_eq!(status(&call(&main, "GET", "/", &cookie).await), 200);

    // The same origins again: the listener keeps its port.
    server.set_assets(listed(&page, &dev));
    assert_eq!(
        server.asset_listeners()[0].1.trim_start_matches("http://"),
        assets
    );

    // No environment lists it any more: the listener closes and the page names it no more.
    server.set_assets(Vec::new());
    assert!(server.asset_listeners().is_empty());
    assert!(closed(&assets).await, "the listener closed");
    let html = call(&main, "GET", "/", &cookie).await;
    assert!(
        body(&html).contains(&format!("href=\"{dev}/app.css\"")),
        "{html}"
    );

    // Listed again: a new listener; the portal's stop ends it.
    server.set_assets(listed(&page, &dev));
    let again = server.asset_listeners()[0]
        .1
        .trim_start_matches("http://")
        .to_owned();
    assert_eq!(status(&call(&again, "GET", "/app.css", &cookie).await), 200);
    server.stop();
    assert!(server.asset_listeners().is_empty());
    assert!(closed(&again).await, "the portal's stop closed it");
}

/// A dev server that switches protocols (Vite's HMR socket), then echoes until the connection
/// closes; `closed` fires when it saw the end.
async fn hmr_upstream() -> (String, tokio::sync::oneshot::Receiver<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    let (closed, seen) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accepts");
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while head_end(&head).is_none() && stream.read(&mut byte).await.unwrap_or(0) == 1 {
            head.push(byte[0]);
        }
        let _ = stream
            .write_all(
                b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
            )
            .await;
        let mut buffer = [0u8; 64];
        while let Ok(read) = stream.read(&mut buffer).await {
            if read == 0 || stream.write_all(&buffer[..read]).await.is_err() {
                break;
            }
        }
        let _ = closed.send(());
    });
    (origin, seen)
}

/// Sends `ping` on `socket` and waits for its echo.
async fn echoes(socket: &mut TcpStream) -> bool {
    if socket.write_all(b"ping").await.is_err() {
        return false;
    }
    let mut echo = [0u8; 4];
    matches!(
        tokio::time::timeout(Duration::from_secs(5), socket.read_exact(&mut echo)).await,
        Ok(Ok(_))
    ) && &echo == b"ping"
}

/// RB-090-65 review: an HMR socket on the asset listener lives while the shown environment lists
/// its origin - a navigation within it keeps it - and ends when another environment is shown
/// that does not.
#[tokio::test]
async fn a_forwarded_socket_ends_when_the_environment_no_longer_lists_its_origin() {
    let (dev, dev_closed) = hmr_upstream().await;
    let (page, _) = upstream(Arc::new(|_: &str, _: &str| {
        answer("text/html", "", "<head></head>")
    }))
    .await;
    let server = start();
    let cookie = pair(&server).await;
    server.set_assets(listed(&page, &dev));
    server.set_page(Some(format!("{page}/")), vec![page.clone()]);
    let assets = server.asset_listeners()[0]
        .1
        .trim_start_matches("http://")
        .to_owned();
    let mut socket = TcpStream::connect(&assets).await.expect("connects");
    socket
        .write_all(
            format!(
                "GET /?token=t HTTP/1.1\r\nHost: {assets}\r\n{cookie}Upgrade: websocket\r\n\
                 Connection: Upgrade\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .expect("writes");
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while head_end(&head).is_none() && socket.read(&mut byte).await.unwrap_or(0) == 1 {
        head.push(byte[0]);
    }
    assert!(String::from_utf8_lossy(&head).starts_with("HTTP/1.1 101 "));
    assert!(echoes(&mut socket).await);

    server.set_page(Some(format!("{page}/other")), vec![page.clone()]);
    assert!(
        echoes(&mut socket).await,
        "a navigation within the environment keeps it"
    );

    let other = "http://127.0.0.1:9".to_owned();
    server.set_page(Some(format!("{other}/")), vec![page.clone(), other]);
    let mut rest = Vec::new();
    let read = tokio::time::timeout(Duration::from_secs(5), socket.read_to_end(&mut rest)).await;
    assert!(
        matches!(read, Ok(Ok(0)) | Ok(Err(_))),
        "the device's end closes: {read:?}"
    );
    assert!(
        tokio::time::timeout(Duration::from_secs(5), dev_closed)
            .await
            .is_ok(),
        "the dev server's end closes too"
    );
    server.stop();
}

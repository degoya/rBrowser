//! RB-090-72 end to end: a device names the origins its page failed to load from over its
//! socket; an unmapped loopback one reaches the offer with the page's origin, a malformed report
//! and every other origin do not, and one socket names at most `MAX_SOCKET_MISSES`.

use futures_util::SinkExt as _;
use tokio_tungstenite::tungstenite::Message;

use super::tests::{pair, socket, start};
use super::*;
use crate::portal::misses::MAX_SOCKET_MISSES;
use crate::portal::offer::set_sink;
use crate::portal::origin_map::forget_logged;

fn missed(origin: &str) -> String {
    format!(r#"{{"kind":"missed","origin":"{origin}"}}"#)
}

#[tokio::test]
async fn a_device_names_its_misses_and_only_an_unmapped_loopback_one_is_offered() {
    forget_logged();
    let seen: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
    let sink = seen.clone();
    set_sink(Some(Arc::new(move |page: &str, origin: &str| {
        lock(&sink).push((page.to_owned(), origin.to_owned()));
    })));
    let (server, _) = start(Duration::from_secs(300), 5);
    let cookie = pair(&server).await;
    let page = "http://localhost:5173";
    server.set_page(Some(format!("{page}/a")), vec![page.to_owned()]);
    let mut ws = socket(&server, &cookie).await;
    let mut reports = vec![
        // Refused whole: a path, a field of another kind, no JSON.
        missed("http://127.0.0.1:9001/a.png?token=secret"),
        r#"{"kind":"missed","origin":["http://127.0.0.1:9002"]}"#.to_owned(),
        "not json".to_owned(),
        // Taken, not offered: another host, the portal itself, the page's forwarded origin.
        missed("http://project.test"),
        missed(server.origin()),
        missed(page),
        // Offered once by this socket.
        missed("http://127.0.0.1:9000"),
        missed("http://127.0.0.1:9000"),
    ];
    let taken_before = 4;
    for port in 0..MAX_SOCKET_MISSES + 4 {
        reports.push(missed(&format!("http://127.0.0.1:{}", 10_000 + port)));
    }
    // A fence: the socket reads in order, so once this is counted every miss was handled.
    reports.push(r#"{"kind":"replayed","status":"not_found"}"#.to_owned());
    for report in reports {
        ws.send(Message::Text(report.into())).await.expect("sends");
    }
    let fenced = tokio::time::timeout(Duration::from_secs(5), async {
        while server
            .devices()
            .first()
            .map_or(0, |device| device.counts.failed)
            < 1
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    set_sink(None);
    assert!(fenced.is_ok(), "the device's reports arrive");
    let mut expected = vec![(page.to_owned(), "http://127.0.0.1:9000".to_owned())];
    for port in 0..MAX_SOCKET_MISSES - taken_before {
        expected.push((
            page.to_owned(),
            format!("http://127.0.0.1:{}", 10_000 + port),
        ));
    }
    assert_eq!(*lock(&seen), expected);
    assert!(
        server.devices()[0].connected,
        "junk does not end the socket"
    );
}

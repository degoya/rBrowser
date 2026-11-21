use std::net::{IpAddr, Ipv4Addr};

use super::*;

#[test]
fn the_forward_asks_adb_for_a_free_local_port_to_chromes_socket() {
    assert_eq!(
        forward_args("R58N123ABC"),
        [
            "-s",
            "R58N123ABC",
            "forward",
            "tcp:0",
            "localabstract:chrome_devtools_remote"
        ]
    );
    assert_eq!(
        remove_args("R58N123ABC", 51234),
        ["-s", "R58N123ABC", "forward", "--remove", "tcp:51234"]
    );
}

/// Nothing rBrowser runs makes adb listen beyond this machine: no `-a`, no `tcpip`, never an
/// address before the port. RB-096-12 narrows RB-095-02's "no call passes `reverse`": the
/// forward calls still pass none, and a `reverse` call names only `tcp:<port>` of one loopback
/// port on both sides (`super::super::reverse`'s tests hold its shape).
#[test]
fn no_adb_call_opens_anything_to_the_network() {
    use crate::engines::android::reverse::{reverse_args, reverse_list_args, unreverse_args};
    let forwards = [
        forward_args("S1"),
        remove_args("S1", 1),
        devices_args(),
        screen_size_args("S1"),
        screen_density_args("S1"),
        start_chrome_args("S1"),
    ];
    let reverses = [
        reverse_args("S1", 3000),
        unreverse_args("S1", 3000),
        reverse_list_args("S1"),
    ];
    for call in forwards.iter().chain(&reverses) {
        assert!(!call.iter().any(|arg| arg == "-a"), "{call:?}");
        assert!(!call.iter().any(|arg| arg == "tcpip"), "{call:?}");
        assert!(
            !call
                .iter()
                .any(|arg| arg.starts_with("tcp:") && arg.contains('.')),
            "{call:?}"
        );
    }
    for call in &forwards {
        assert!(!call.iter().any(|arg| arg == "reverse"), "{call:?}");
    }
    for call in &reverses {
        assert_eq!(call.iter().filter(|arg| *arg == "reverse").count(), 1);
        assert!(
            call.iter()
                .filter(|arg| arg.starts_with("tcp:"))
                .all(|arg| arg == "tcp:3000"),
            "{call:?}"
        );
    }
}

#[test]
fn the_port_adb_chose_is_read() {
    assert_eq!(parse_port("51234\n"), Some(51234));
    assert_eq!(parse_port("  8000  "), Some(8000));
    assert_eq!(parse_port(""), None);
    assert_eq!(parse_port("0"), None);
    assert_eq!(parse_port("error: device unauthorized"), None);
}

/// Chrome names its browser socket with a host of its own (`localhost`, no port, or the
/// device's): the client always dials 127.0.0.1 at the forwarded port, keeping only the path.
#[test]
fn the_browser_socket_is_always_dialled_on_loopback() {
    let version = r#"{ "Browser": "Chrome/131.0", "webSocketDebuggerUrl": "ws://localhost/devtools/browser/abc-123" }"#;
    assert_eq!(
        browser_socket(51234, version).expect("a socket"),
        "ws://127.0.0.1:51234/devtools/browser/abc-123"
    );
    let elsewhere = r#"{ "webSocketDebuggerUrl": "ws://10.0.0.7:9222/devtools/browser/x" }"#;
    assert_eq!(
        browser_socket(4000, elsewhere).expect("a socket"),
        "ws://127.0.0.1:4000/devtools/browser/x"
    );
    // Chrome on Android names its discoverable browser target without an id (owner's device,
    // 2026-11-21: the view never started).
    let android = r#"{ "Android-Package": "com.android.chrome", "webSocketDebuggerUrl": "ws://localhost/devtools/browser" }"#;
    assert_eq!(
        browser_socket(4000, android).expect("a socket"),
        "ws://127.0.0.1:4000/devtools/browser"
    );
    for bad in [
        "{}",
        "not json",
        r#"{ "webSocketDebuggerUrl": "ws://localhost/elsewhere" }"#,
        r#"{ "webSocketDebuggerUrl": "ws://localhost/devtools/browser/a b" }"#,
    ] {
        assert!(browser_socket(4000, bad).is_none(), "{bad}");
    }
}

#[test]
fn the_version_request_goes_to_loopback_and_its_answer_is_read() {
    let request = version_request(51234);
    assert!(
        request.starts_with("GET /json/version HTTP/1.1\r\n"),
        "{request}"
    );
    assert!(
        request.contains("\r\nHost: 127.0.0.1:51234\r\n"),
        "{request}"
    );
    assert!(request.ends_with("\r\n\r\n"));
    let answer = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"a\":1}";
    assert_eq!(http_body(answer), Some("{\"a\":1}"));
    assert_eq!(http_body("HTTP/1.1 404 Not Found\r\n\r\nnope"), None);
    assert_eq!(http_body("garbage"), None);
}

/// Chrome's DevTools server keeps the connection open after its answer: the answer is whole
/// once its head and `Content-Length` bytes of body arrived, not at the end of the stream.
#[test]
fn an_answer_is_whole_with_its_content_length() {
    let whole =
        b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nContent-Type: application/json\r\n\r\n{\"a\":1}";
    assert!(answer_whole(whole));
    assert!(!answer_whole(&whole[..whole.len() - 1]), "a byte short");
    assert!(
        !answer_whole(b"HTTP/1.1 200 OK\r\nContent-Len"),
        "no head yet"
    );
    let lower = b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n{}";
    assert!(answer_whole(lower), "the header name in any case");
    assert!(
        !answer_whole(b"HTTP/1.1 200 OK\r\n\r\n{}"),
        "without a length only the end of the stream ends it"
    );
}

/// The security condition of the job: the forwarded port answers on loopback only. An adb
/// server started with `-a` binds its forwards to every interface; then the forward is
/// reachable at the machine's own network address, and the view does not open.
#[test]
fn a_forward_reachable_from_the_network_is_refused() {
    let lan = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20));
    let open_on_lan = |addr: SocketAddr| addr.ip() == lan;
    assert_eq!(
        exposure(51234, &[lan], open_on_lan),
        Err(Exposed::At(SocketAddr::new(lan, 51234)))
    );
    assert_eq!(exposure(51234, &[lan], |_| false), Ok(()));
    // Loopback addresses are never asked: there it is meant to answer.
    let asked = std::cell::RefCell::new(Vec::new());
    let probe = |addr: SocketAddr| {
        asked.borrow_mut().push(addr);
        false
    };
    let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
    assert_eq!(exposure(1, &[loopback, lan], probe), Ok(()));
    assert_eq!(*asked.borrow(), [SocketAddr::new(lan, 1)]);
}

/// Every address is asked, not only the first: a second interface that answers is found.
#[test]
fn every_own_address_is_asked() {
    let lan = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20));
    let vpn = IpAddr::V4(Ipv4Addr::new(100, 64, 0, 7));
    let open_on_vpn = |addr: SocketAddr| addr.ip() == vpn;
    assert_eq!(
        exposure(7, &[lan, vpn], open_on_vpn),
        Err(Exposed::At(SocketAddr::new(vpn, 7)))
    );
}

/// No own address to ask is no proof: the forward is refused, never let through unchecked.
#[test]
fn without_an_address_to_ask_the_forward_is_unchecked() {
    let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
    assert_eq!(exposure(1, &[], |_| false), Err(Exposed::Unchecked));
    assert_eq!(exposure(1, &[loopback], |_| false), Err(Exposed::Unchecked));
}

#[test]
fn the_endpoint_is_loopback() {
    assert_eq!(endpoint(51234), SocketAddr::from(([127, 0, 0, 1], 51234)));
    assert!(endpoint(1).ip().is_loopback());
}

#[test]
fn history_steps_to_the_entry_before_or_after() {
    let history = serde_json::json!({
        "currentIndex": 1,
        "entries": [
            { "id": 10, "url": "http://localhost:3000/" },
            { "id": 11 },
            { "id": 12 }
        ]
    });
    assert_eq!(history_entry(&history, -1), Some(10));
    assert_eq!(history_entry(&history, 1), Some(12));
    assert_eq!(history_url(&history, -1), Some("http://localhost:3000/"));
    assert_eq!(history_url(&history, 1), None);
    let start = serde_json::json!({ "currentIndex": 0, "entries": [ { "id": 10 } ] });
    assert_eq!(history_entry(&start, -1), None);
    assert_eq!(history_entry(&start, 1), None);
}

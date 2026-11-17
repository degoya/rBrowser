use serde_json::json;

use super::*;
use crate::request::parse_head;

const BROWSER: &str = "b0b0";

fn token() -> ProxyToken {
    ProxyToken::from_text("a".repeat(64))
}

fn head(lines: &str) -> RequestHead {
    parse_head(format!("{lines}\r\n\r\n").as_bytes()).expect("head")
}

fn get(path: &str, extra: &str) -> RequestHead {
    head(&format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:9222{extra}"
    ))
}

fn bearer() -> String {
    format!("\r\nAuthorization: Bearer {}", "a".repeat(64))
}

const UPGRADE: &str = "\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: a2V5";

#[test]
fn endpoints_are_found_with_and_without_a_trailing_slash() {
    assert_eq!(endpoint("/json/version", BROWSER), Some(Endpoint::Version));
    assert_eq!(endpoint("/json/version/", BROWSER), Some(Endpoint::Version));
    assert_eq!(endpoint("/json", BROWSER), Some(Endpoint::List));
    assert_eq!(endpoint("/json/list/", BROWSER), Some(Endpoint::List));
    assert_eq!(
        endpoint("/json/protocol", BROWSER),
        Some(Endpoint::Protocol)
    );
    assert_eq!(
        endpoint("/devtools/browser/b0b0", BROWSER),
        Some(Endpoint::Browser)
    );
    assert_eq!(
        endpoint("/devtools/page/ABC", BROWSER),
        Some(Endpoint::Page("ABC".to_owned()))
    );
    for missing in [
        "/",
        "/json/new",
        "/json/close/ABC",
        "/devtools/browser/other",
        "/devtools/page/",
        "/devtools/page/A/B",
    ] {
        assert_eq!(endpoint(missing, BROWSER), None, "{missing}");
    }
}

#[test]
fn without_the_token_every_endpoint_is_unauthorized() {
    for path in [
        "/json/version",
        "/json/list",
        "/devtools/browser/b0b0",
        "/nothing",
    ] {
        assert_eq!(
            decide(&get(path, ""), &token(), BROWSER),
            Decision::Refuse(Refusal::Unauthorized),
            "{path}"
        );
    }
    let upgrade = get("/devtools/browser/b0b0", UPGRADE);
    assert_eq!(
        decide(&upgrade, &token(), BROWSER),
        Decision::Refuse(Refusal::Unauthorized)
    );
}

#[test]
fn a_foreign_host_or_origin_is_forbidden_before_the_token_is_looked_at() {
    let rebinding = head(&format!(
        "GET /json HTTP/1.1\r\nHost: evil.test:9222{}",
        bearer()
    ));
    assert_eq!(
        decide(&rebinding, &token(), BROWSER),
        Decision::Refuse(Refusal::Forbidden)
    );
    let no_host = head(&format!("GET /json HTTP/1.1{}", bearer()));
    assert_eq!(
        decide(&no_host, &token(), BROWSER),
        Decision::Refuse(Refusal::Forbidden)
    );
    let page_origin = get(
        "/json",
        &format!("{}\r\nOrigin: https://evil.test", bearer()),
    );
    assert_eq!(
        decide(&page_origin, &token(), BROWSER),
        Decision::Refuse(Refusal::Forbidden)
    );
    for host in ["localhost:1", "127.0.0.1", "[::1]:9"] {
        let fine = head(&format!("GET /json HTTP/1.1\r\nHost: {host}{}", bearer()));
        assert!(
            matches!(decide(&fine, &token(), BROWSER), Decision::Http(..)),
            "{host}"
        );
    }
    let devtools = get(
        "/json",
        &format!("{}\r\nOrigin: devtools://devtools", bearer()),
    );
    assert!(matches!(
        decide(&devtools, &token(), BROWSER),
        Decision::Http(..)
    ));
}

#[test]
fn with_the_token_json_is_answered_and_websockets_are_upgraded() {
    let version = get("/json/version", &bearer());
    let Decision::Http(Endpoint::Version, base) = decide(&version, &token(), BROWSER) else {
        panic!("not answered");
    };
    assert_eq!(
        base,
        Base {
            host: "127.0.0.1:9222".to_owned(),
            prefix: String::new()
        }
    );
    let upgrade = get("/devtools/page/T1", &format!("{}{UPGRADE}", bearer()));
    assert_eq!(
        decide(&upgrade, &token(), BROWSER),
        Decision::Upgrade(Endpoint::Page("T1".to_owned()), "a2V5".to_owned())
    );
}

#[test]
fn a_path_token_keeps_its_prefix_in_the_answers_urls() {
    let path = format!("/t/{}/json/version", "a".repeat(64));
    let Decision::Http(Endpoint::Version, base) = decide(&get(&path, ""), &token(), BROWSER) else {
        panic!("not answered");
    };
    assert_eq!(base.prefix, format!("/t/{}", "a".repeat(64)));
    assert_eq!(
        ws_url(&base, "/devtools/browser/b0b0"),
        format!(
            "ws://127.0.0.1:9222/t/{}/devtools/browser/b0b0",
            "a".repeat(64)
        )
    );
}

#[test]
fn wrong_methods_unknown_paths_and_plain_gets_of_sockets_are_refused() {
    let post = head(&format!(
        "POST /json HTTP/1.1\r\nHost: 127.0.0.1{}",
        bearer()
    ));
    assert_eq!(
        decide(&post, &token(), BROWSER),
        Decision::Refuse(Refusal::MethodNotAllowed)
    );
    let unknown = get("/json/new", &bearer());
    assert_eq!(
        decide(&unknown, &token(), BROWSER),
        Decision::Refuse(Refusal::NotFound)
    );
    let plain = get("/devtools/browser/b0b0", &bearer());
    assert_eq!(
        decide(&plain, &token(), BROWSER),
        Decision::Refuse(Refusal::BadRequest)
    );
    let upgrade_json = get("/json", &format!("{}{UPGRADE}", bearer()));
    assert_eq!(
        decide(&upgrade_json, &token(), BROWSER),
        Decision::Refuse(Refusal::BadRequest)
    );
}

#[test]
fn refusals_have_their_status_and_never_name_the_token() {
    assert_eq!(Refusal::Unauthorized.status(), 401);
    assert_eq!(Refusal::Forbidden.status(), 403);
    assert_eq!(Refusal::NotFound.status(), 404);
    assert_eq!(Refusal::MethodNotAllowed.status(), 405);
    assert_eq!(Refusal::BadRequest.status(), 400);
    assert_eq!(Refusal::TooLarge.status(), 431);
    assert!(Refusal::Unauthorized.reason().contains("token"));
}

#[test]
fn the_version_answer_has_chromes_fields_and_the_browser_socket() {
    let base = Base {
        host: "127.0.0.1:9".to_owned(),
        prefix: String::new(),
    };
    let version = json!({"protocolVersion": "1.3", "product": "Chrome/156.0", "revision": "@abc", "userAgent": "UA", "jsVersion": "15.6"});
    assert_eq!(
        version_body(&base, BROWSER, &version),
        json!({
            "Browser": "Chrome/156.0",
            "Protocol-Version": "1.3",
            "User-Agent": "UA",
            "V8-Version": "15.6",
            "WebKit-Version": "@abc",
            "webSocketDebuggerUrl": "ws://127.0.0.1:9/devtools/browser/b0b0"
        })
    );
}

#[test]
fn the_list_has_one_entry_per_target_with_its_socket() {
    let base = Base {
        host: "localhost:9".to_owned(),
        prefix: "/t/x".to_owned(),
    };
    let infos = [
        json!({"targetId": "T1", "type": "page", "title": "A", "url": "https://a.test/", "attached": true}),
    ];
    assert_eq!(
        list_body(&base, &infos),
        json!([{
            "description": "",
            "id": "T1",
            "title": "A",
            "type": "page",
            "url": "https://a.test/",
            "webSocketDebuggerUrl": "ws://localhost:9/t/x/devtools/page/T1"
        }])
    );
}

#[test]
fn the_protocol_answer_lists_the_domains() {
    let domains = json!({"domains": [{"name": "Page", "version": "1.3"}]});
    assert_eq!(
        protocol_body(&domains),
        json!({"domains": [{"name": "Page", "version": "1.3"}]})
    );
    assert_eq!(protocol_body(&json!(null)), json!({"domains": []}));
}

#[test]
fn a_browser_id_is_32_hex_digits() {
    let id = new_browser_id().expect("random");
    assert_eq!(id.len(), 32);
    assert!(id.bytes().all(|b| b.is_ascii_hexdigit()));
}

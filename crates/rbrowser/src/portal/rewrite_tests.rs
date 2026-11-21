use rb_cdp::request::parse_head;

use super::*;
use crate::portal::origin_map::OriginMap;
use crate::portal::origins::Upstream;

const PORTAL: &str = "http://192.168.1.20:4000";

fn upstream() -> Upstream {
    Upstream::parse("http://localhost:5173").expect("parses")
}

fn map() -> OriginMap {
    OriginMap::new("http://localhost:5173", PORTAL)
}

fn lines(text: &str) -> Vec<&str> {
    text.split("\r\n").collect()
}

#[test]
fn the_request_names_the_upstream_and_loses_the_portal_cookie() {
    let head = parse_head(
        b"GET /app?x=1 HTTP/1.1\r\nHost: 192.168.1.20:4000\r\nOrigin: http://192.168.1.20:4000\r\n\
          Referer: http://192.168.1.20:4000/start\r\nCookie: a=1; __rbportal=secret; b=2\r\n\
          Accept-Encoding: gzip, br\r\nConnection: keep-alive\r\nProxy-Authorization: x\r\n\r\n",
    )
    .expect("parses");
    let upstream = upstream();
    let text = request_head(&head, &upstream, &map(), false);
    let lines = lines(&text);
    assert_eq!(lines[0], "GET /app?x=1 HTTP/1.1");
    assert!(lines.contains(&"Host: localhost:5173"));
    assert!(lines.contains(&"Origin: http://localhost:5173"));
    assert!(lines.contains(&"Referer: http://localhost:5173/start"));
    assert!(lines.contains(&"Cookie: a=1; b=2"));
    assert!(lines.contains(&"Accept-Encoding: identity"));
    assert!(lines.contains(&"Connection: close"));
    assert!(!text.contains("secret"), "{text}");
    assert!(!text.contains("Proxy-Authorization"));
    assert!(!text.contains("keep-alive"));
    assert!(text.ends_with("\r\n\r\n"));
}

#[test]
fn a_request_with_only_the_portal_cookie_sends_none() {
    let head = parse_head(b"GET / HTTP/1.1\r\nHost: h\r\nCookie: __rbportal=secret\r\n\r\n")
        .expect("parses");
    let text = request_head(&head, &upstream(), &map(), false);
    assert!(!text.contains("Cookie"), "{text}");
}

#[test]
fn an_upgrade_keeps_its_headers() {
    let head = parse_head(
        b"GET /?token=x HTTP/1.1\r\nHost: h\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
          Sec-WebSocket-Key: k\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: vite-hmr\r\n\r\n",
    )
    .expect("parses");
    let text = request_head(&head, &upstream(), &map(), true);
    let lines = lines(&text);
    assert!(lines.contains(&"Upgrade: websocket"));
    assert!(lines.contains(&"Connection: Upgrade"));
    assert!(lines.contains(&"Sec-WebSocket-Protocol: vite-hmr"));
    assert!(!lines.contains(&"Connection: close"));
}

#[test]
fn a_response_head_parses() {
    let head = parse_response_head(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: 12\r\n\r\nbody",
    )
    .expect("parses");
    assert_eq!(head.status, 200);
    assert!(head.is_html());
    assert_eq!(head.content_length(), Some(12));
    assert!(!head.is_chunked());
    assert!(parse_response_head(b"garbage\r\n\r\n").is_err());
}

#[test]
fn the_response_moves_locations_and_cookies_to_the_portal() {
    let head = parse_response_head(
        b"HTTP/1.1 302 Found\r\nLocation: http://localhost:5173/login\r\n\
          Set-Cookie: sid=1; Domain=localhost; Path=/; HttpOnly\r\nSet-Cookie: __rbportal=evil\r\n\
          Strict-Transport-Security: max-age=1\r\nConnection: keep-alive\r\n\
          Transfer-Encoding: chunked\r\n\r\n",
    )
    .expect("parses");
    let text = response_head(&head, &map(), OutBody::Upstream);
    let lines = lines(&text);
    assert_eq!(lines[0], "HTTP/1.1 302 Found");
    assert!(lines.contains(&"Location: http://192.168.1.20:4000/login"));
    assert!(lines.contains(&"Set-Cookie: sid=1; Path=/; HttpOnly"));
    assert!(
        lines.contains(&"Transfer-Encoding: chunked"),
        "streamed as it came"
    );
    assert!(lines.contains(&"Connection: close"));
    assert!(!text.contains("__rbportal"));
    assert!(!text.contains("Strict-Transport-Security"));
}

#[test]
fn a_rewritten_body_gets_its_new_length() {
    let head = parse_response_head(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nTransfer-Encoding: chunked\r\n\r\n",
    )
    .expect("parses");
    let text = response_head(&head, &map(), OutBody::Length(42));
    let lines = lines(&text);
    assert!(lines.contains(&"Content-Length: 42"));
    assert!(!text.contains("Transfer-Encoding"));
}

#[test]
fn a_switching_protocols_answer_keeps_the_upgrade() {
    let head = parse_response_head(
        b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
          Sec-WebSocket-Accept: a\r\n\r\n",
    )
    .expect("parses");
    let text = response_head(&head, &map(), OutBody::Upstream);
    assert!(text.contains("Upgrade: websocket\r\n"));
    assert!(text.contains("Connection: Upgrade\r\n"));
    assert!(!text.contains("Connection: close"));
}

#[test]
fn cors_names_the_portal_origin_without_a_path() {
    let head = parse_response_head(
        b"HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: http://localhost:5173\r\n\r\n",
    )
    .expect("parses");
    let text = response_head(&head, &map(), OutBody::Upstream);
    assert!(
        text.contains("Access-Control-Allow-Origin: http://192.168.1.20:4000\r\n"),
        "{text}"
    );
}

const ASSETS: &str = "http://192.168.1.20:4100";

/// RB-090-65: a module script on the asset listener carries the page's portal origin; the dev
/// server sees the page's upstream origin (its CORS rule admits it) and the device the portal's.
#[test]
fn an_asset_request_names_the_page_upstream_and_its_cors_the_page_portal() {
    let map = OriginMap::new("http://localhost:8000", PORTAL).with("http://localhost:5173", ASSETS);
    let head = parse_head(
        b"GET /@vite/client HTTP/1.1\r\nHost: 192.168.1.20:4100\r\nOrigin: http://192.168.1.20:4000\r\n\
          Referer: http://192.168.1.20:4000/dashboard\r\n\r\n",
    )
    .expect("parses");
    let text = request_head(&head, &upstream(), &map, false);
    let lines = lines(&text);
    assert!(lines.contains(&"Host: localhost:5173"));
    assert!(lines.contains(&"Origin: http://localhost:8000"), "{text}");
    assert!(
        lines.contains(&"Referer: http://localhost:8000/dashboard"),
        "{text}"
    );
    let answer = parse_response_head(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\n\
          Access-Control-Allow-Origin: http://localhost:8000\r\nLocation: http://localhost:5173/x\r\n\r\n",
    )
    .expect("parses");
    assert!(answer.is_css_or_script());
    assert!(!answer.is_html());
    let text = response_head(&answer, &map, OutBody::Upstream);
    assert!(
        text.contains("Access-Control-Allow-Origin: http://192.168.1.20:4000\r\n"),
        "{text}"
    );
    assert!(
        text.contains("Location: http://192.168.1.20:4100/x\r\n"),
        "{text}"
    );
}

#[test]
fn style_sheets_and_scripts_are_recognised() {
    for mime in [
        "text/css",
        "text/css; charset=utf-8",
        "Text/JavaScript",
        "application/javascript",
    ] {
        let head = parse_response_head(
            format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\n\r\n").as_bytes(),
        )
        .expect("parses");
        assert!(head.is_css_or_script(), "{mime}");
    }
    let image =
        parse_response_head(b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\n\r\n").expect("parses");
    assert!(!image.is_css_or_script());
}

/// A body the portal rewrote names the portal's ports of this run: no validator or cache life
/// of the upstream's may outlive them (a revalidation after a restart would answer 304 and keep
/// a body pointing at a dead port). A body passed through keeps them.
#[test]
fn a_rewritten_body_is_never_cached() {
    let head = parse_response_head(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\nETag: \"abc\"\r\n\
          Last-Modified: Tue, 17 Nov 2026 10:00:00 GMT\r\nCache-Control: max-age=3600\r\n\
          Expires: Wed, 18 Nov 2026 10:00:00 GMT\r\nContent-Length: 3\r\n\r\n",
    )
    .expect("parses");
    let text = response_head(&head, &map(), OutBody::Length(5));
    let lines = lines(&text);
    assert!(lines.contains(&"Cache-Control: no-store"), "{text}");
    for gone in ["ETag", "Last-Modified", "max-age", "Expires"] {
        assert!(!text.contains(gone), "{gone}: {text}");
    }
    let text = response_head(&head, &map(), OutBody::Upstream);
    assert!(text.contains("ETag: \"abc\"\r\n"), "{text}");
    assert!(text.contains("Cache-Control: max-age=3600\r\n"), "{text}");
    assert!(!text.contains("no-store"), "{text}");
}

/// RB-090-69: a JSON answer (an API naming its images) is rewritten like a script.
#[test]
fn json_bodies_are_recognised() {
    for mime in [
        "application/json",
        "application/json; charset=utf-8",
        "Application/JSON",
        "application/ld+json",
        "application/vnd.api+json",
    ] {
        let head = parse_response_head(
            format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\n\r\n").as_bytes(),
        )
        .expect("parses");
        assert!(head.is_json(), "{mime}");
    }
    for mime in [
        "text/plain",
        "image/png",
        "application/jsonx",
        "text/javascript",
    ] {
        let head = parse_response_head(
            format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\n\r\n").as_bytes(),
        )
        .expect("parses");
        assert!(!head.is_json(), "{mime}");
    }
}

/// RB-090-69 review: only a JSON body of a known length within the bound is read whole; a
/// chunked one or one of unknown length streams.
#[test]
fn only_a_known_length_within_the_bound_is_read_whole() {
    let head = |extra: &str| {
        parse_response_head(
            format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{extra}\r\n").as_bytes(),
        )
        .expect("parses")
    };
    assert!(head("Content-Length: 10\r\n").length_within(10));
    assert!(!head("Content-Length: 11\r\n").length_within(10));
    assert!(!head("").length_within(10));
    assert!(!head("Transfer-Encoding: chunked\r\n").length_within(10));
    assert!(!head("Content-Length: 3\r\nTransfer-Encoding: chunked\r\n").length_within(10));
}

/// RB-090-72: a body rewritten as it streams goes out chunked anew, never cached, without the
/// upstream's length and validators.
#[test]
fn a_streamed_rewritten_body_goes_out_chunked_and_never_cached() {
    let head = parse_response_head(
        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 30\r\n\
          ETag: \"abc\"\r\nLast-Modified: Tue, 17 Nov 2026 10:00:00 GMT\r\n\r\n",
    )
    .expect("parses");
    let text = response_head(&head, &map(), OutBody::Chunked);
    let lines = lines(&text);
    assert!(lines.contains(&"Transfer-Encoding: chunked"), "{text}");
    assert!(lines.contains(&"Cache-Control: no-store"), "{text}");
    for gone in ["Content-Length", "ETag", "Last-Modified"] {
        assert!(!text.contains(gone), "{gone}: {text}");
    }
}

/// RB-090-72: a device's cached copy may predate the portal's rewrite (or name another run's
/// ports): a request for anything that may be rewritten asks the upstream for no `304`.
#[test]
fn a_request_asks_for_no_revalidation() {
    let request = parse_head(
        b"GET /media/a.webp HTTP/1.1\r\nHost: 192.168.1.20:4000\r\nIf-None-Match: \"abc\"\r\n\
          If-Modified-Since: Tue, 17 Nov 2026 10:00:00 GMT\r\nAccept: image/webp\r\n\r\n",
    )
    .expect("parses");
    let text = request_head(&request, &upstream(), &map(), false);
    assert!(!text.contains("If-None-Match"), "{text}");
    assert!(!text.contains("If-Modified-Since"), "{text}");
    assert!(text.contains("Accept: image/webp\r\n"), "{text}");
}

/// RB-090-72 review: an image, a font or a medium is never rewritten - it keeps its validators,
/// and the device may revalidate it (`Sec-Fetch-Dest`); a document, a script, a style sheet or a
/// request without the header may be rewritten and asks for no `304`.
#[test]
fn a_request_for_what_is_never_rewritten_keeps_its_revalidation() {
    let ask = |dest: &str| {
        let request = parse_head(
            format!(
                "GET /a HTTP/1.1\r\nHost: 192.168.1.20:4000\r\n{dest}If-None-Match: \"abc\"\r\n\
                 If-Modified-Since: Tue, 17 Nov 2026 10:00:00 GMT\r\n\r\n"
            )
            .as_bytes(),
        )
        .expect("parses");
        request_head(&request, &upstream(), &map(), false)
    };
    for dest in ["image", "font", "audio", "video", "track", "IMAGE"] {
        let text = ask(&format!("Sec-Fetch-Dest: {dest}\r\n"));
        assert!(
            text.contains("If-None-Match: \"abc\"\r\n"),
            "{dest}: {text}"
        );
        assert!(text.contains("If-Modified-Since: "), "{dest}: {text}");
    }
    for dest in [
        "Sec-Fetch-Dest: script\r\n",
        "Sec-Fetch-Dest: style\r\n",
        "Sec-Fetch-Dest: empty\r\n",
        "",
    ] {
        let text = ask(dest);
        assert!(!text.contains("If-None-Match"), "{dest}: {text}");
        assert!(!text.contains("If-Modified-Since"), "{dest}: {text}");
    }
}

/// RB-090-72 review: a body chunked anew goes out as `HTTP/1.1` - the portal is the device's
/// server - also when the upstream speaks `HTTP/1.0` (which knows no chunks).
#[test]
fn a_body_chunked_anew_goes_out_as_http_1_1() {
    let head = parse_response_head(b"HTTP/1.0 200 OK\r\nContent-Type: text/javascript\r\n\r\n")
        .expect("parses");
    let text = response_head(&head, &map(), OutBody::Chunked);
    assert!(text.starts_with("HTTP/1.1 200 OK\r\n"), "{text}");
    assert!(text.contains("Transfer-Encoding: chunked\r\n"), "{text}");
    let text = response_head(&head, &map(), OutBody::Upstream);
    assert!(
        text.starts_with("HTTP/1.0 200 OK\r\n"),
        "as it came: {text}"
    );
}

/// RB-096-05: in the presentation's traffic no cookie goes either way, no Cloudflare header
/// reaches the dev server and no server detail the viewer; the answer asks not to be indexed.
#[test]
fn the_public_map_keeps_cookies_and_server_details_at_the_portal() {
    let public = OriginMap::new(
        "http://localhost:5173",
        "https://quick-brown-fox.trycloudflare.com",
    )
    .public();
    assert!(public.is_public());
    assert!(!map().is_public());
    let head = parse_head(
        b"GET /app HTTP/1.1\r\nHost: quick-brown-fox.trycloudflare.com\r\n\
          Cookie: __rbportal=secret; app=1\r\nCf-Connecting-Ip: 203.0.113.9\r\nCf-Ray: 1\r\n\
          Cdn-Loop: cloudflare\r\nAccept: text/html\r\n\r\n",
    )
    .expect("parses");
    let text = request_head(&head, &upstream(), &public, false);
    assert!(!text.contains("Cookie"), "{text}");
    assert!(!text.contains("203.0.113.9"), "{text}");
    assert!(!text.to_ascii_lowercase().contains("cf-"), "{text}");
    assert!(!text.contains("Cdn-Loop"), "{text}");
    assert!(text.contains("Accept: text/html\r\n"), "{text}");
    let response = parse_response_head(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nSet-Cookie: session=presenter\r\n\
          Server: nginx/1.27 (Ubuntu)\r\nX-Powered-By: PHP/8.4\r\nX-Debug-Token: abc\r\n\r\n",
    )
    .expect("parses");
    let out = response_head(&response, &public, OutBody::Upstream);
    for gone in ["Set-Cookie", "presenter", "nginx", "PHP", "X-Debug-Token"] {
        assert!(!out.contains(gone), "{gone}: {out}");
    }
    assert!(out.contains("Content-Type: text/html\r\n"), "{out}");
    assert!(out.contains("X-Robots-Tag: noindex, nofollow\r\n"), "{out}");
    let lan = response_head(&response, &map(), OutBody::Upstream);
    assert!(
        lan.contains("Set-Cookie: session=presenter"),
        "the LAN portal keeps them"
    );
    assert!(!lan.contains("X-Robots-Tag"), "{lan}");
}

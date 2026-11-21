use std::net::SocketAddr;

use rb_plugin_types::{HttpRequest, codes};
use ureq::http::Uri;

use super::{RESPONSE_HEADERS, admitted, agent, fetch_blocking, passed_value};
use crate::plugins::services::net::Origin;

fn addresses(list: &[&str]) -> Vec<SocketAddr> {
    list.iter()
        .map(|text| text.parse().expect("an address"))
        .collect()
}

fn uri(text: &str) -> Uri {
    text.parse().expect("a URI")
}

#[test]
fn a_name_outside_the_project_reaches_only_its_public_addresses() {
    let mixed = addresses(&["10.0.0.5:443", "93.184.216.34:443", "[::1]:443"]);
    let kept = admitted(&uri("https://site.test/"), &mixed, &[]).expect("one public");
    assert_eq!(&kept[..], &addresses(&["93.184.216.34:443"])[..]);

    let internal = addresses(&["127.0.0.1:80", "169.254.169.254:80"]);
    let refused = admitted(&uri("http://metadata.test/"), &internal, &[]).expect_err("none");
    assert!(matches!(refused, ureq::Error::Other(_)));
}

#[test]
fn the_projects_own_origin_may_be_local() {
    let local = vec![Origin::of("http://localhost:3000/").expect("origin")];
    let loopback = addresses(&["127.0.0.1:3000", "[::1]:3000"]);
    let kept = admitted(&uri("http://localhost:3000/robots.txt"), &loopback, &local)
        .expect("the project's own");
    assert_eq!(kept.len(), 2);
    // Another port of the same host is another origin.
    let other = addresses(&["127.0.0.1:5432"]);
    assert!(admitted(&uri("http://localhost:5432/"), &other, &local).is_err());
}

#[test]
fn a_refused_address_is_the_url_denied_code_and_nothing_connects() {
    let agent = agent(std::sync::Arc::default());
    let refused = fetch_blocking(
        &agent,
        &HttpRequest {
            method: "GET".to_owned(),
            url: "http://127.0.0.1:9/".to_owned(),
            ..HttpRequest::default()
        },
        1024,
    )
    .expect_err("loopback");
    assert_eq!(refused.code(), Some(codes::HTTP_URL_DENIED));
}

#[test]
fn the_security_headers_reach_the_plugins_and_never_a_cookie_value() {
    for name in [
        "content-security-policy",
        "strict-transport-security",
        "x-frame-options",
        "x-content-type-options",
        "referrer-policy",
        "permissions-policy",
        "access-control-allow-origin",
        "access-control-allow-credentials",
        "date",
        "set-cookie",
    ] {
        assert!(RESPONSE_HEADERS.contains(&name), "{name}");
    }
    assert!(!RESPONSE_HEADERS.contains(&"cookie"));
    assert_eq!(
        passed_value(
            "set-cookie",
            "sid=SECRET123; Path=/; Secure; HttpOnly; SameSite=Lax"
        ),
        "sid=; Path=/; Secure; HttpOnly; SameSite=Lax"
    );
    assert_eq!(passed_value("set-cookie", "token=a=b=c"), "token=");
    assert_eq!(passed_value("set-cookie", "SECRETONLY"), "");
    assert_eq!(
        passed_value("content-security-policy", "default-src 'self'"),
        "default-src 'self'"
    );
}

#[test]
fn a_set_cookie_passes_only_its_name_and_known_attributes() {
    // A folded second cookie, a quoted value with `;` and an unknown token leak nothing.
    assert_eq!(
        passed_value("set-cookie", "a=1; Path=/, b=SECRET; Secure"),
        "a=; Secure"
    );
    assert_eq!(
        passed_value("set-cookie", "sid=\"x;SECRET\"; HttpOnly"),
        "sid=; HttpOnly"
    );
    assert_eq!(
        passed_value(
            "set-cookie",
            "id=1; Expires=Thu, 01 Jan 2030 00:00:00 GMT; Max-Age=3600; Domain=example.test; SameSite=Lax; Partitioned; X-Secret=abc"
        ),
        "id=; Expires=Thu, 01 Jan 2030 00:00:00 GMT; Max-Age=3600; Domain=example.test; SameSite=Lax; Partitioned"
    );
    assert_eq!(
        passed_value(
            "set-cookie",
            "id=1; Expires=Thu, 01 Jan 2030 00:00:00 GMT, b=SECRET"
        ),
        "id="
    );
}

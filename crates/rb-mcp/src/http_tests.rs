use axum::http::{HeaderMap, HeaderValue, StatusCode, header};

use super::{check, is_loopback_authority, is_loopback_origin, nonce_of};
use crate::McpToken;

fn headers(host: &str, auth: Option<&str>, origin: Option<&str>) -> HeaderMap {
    let mut map = HeaderMap::new();
    map.insert(header::HOST, HeaderValue::from_str(host).expect("host"));
    if let Some(auth) = auth {
        map.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(auth).expect("auth"),
        );
    }
    if let Some(origin) = origin {
        map.insert(
            header::ORIGIN,
            HeaderValue::from_str(origin).expect("origin"),
        );
    }
    map
}

#[test]
fn the_token_decides_on_a_loopback_request() {
    let token = McpToken::from_text("t".repeat(64));
    let good = format!("Bearer {}", "t".repeat(64));
    assert!(check(&token, &headers("127.0.0.1:4000", Some(&good), None)).is_ok());
    let lower = format!("bearer {}", "t".repeat(64));
    assert!(check(&token, &headers("localhost:4000", Some(&lower), None)).is_ok());
    for auth in [
        None,
        Some("Bearer wrong"),
        Some("Basic dDp0"),
        Some("Bearer"),
    ] {
        let refusal = check(&token, &headers("127.0.0.1:4000", auth, None)).expect_err("refused");
        assert_eq!(refusal.status, StatusCode::UNAUTHORIZED);
    }
}

#[test]
fn a_foreign_host_or_origin_is_refused_before_the_token() {
    let token = McpToken::from_text("t".repeat(64));
    let good = format!("Bearer {}", "t".repeat(64));
    let refusal = check(&token, &headers("evil.test:4000", Some(&good), None)).expect_err("host");
    assert_eq!(refusal.code, "http.host_rejected");
    let refusal = check(
        &token,
        &headers("127.0.0.1:4000", Some(&good), Some("https://evil.test")),
    )
    .expect_err("origin");
    assert_eq!(refusal.code, "http.origin_rejected");
    assert_eq!(refusal.body()["legacyCode"], "origin_rejected");
    assert!(
        check(
            &token,
            &headers("127.0.0.1:4000", Some(&good), Some("http://localhost:5173"))
        )
        .is_ok()
    );
}

#[test]
fn loopback_names_and_origins() {
    assert!(is_loopback_authority("127.0.0.1"));
    assert!(is_loopback_authority("LOCALHOST:80"));
    assert!(is_loopback_authority("[::1]:8080"));
    assert!(!is_loopback_authority("127.0.0.1.evil.test"));
    assert!(!is_loopback_authority("0.0.0.0:80"));
    assert!(is_loopback_origin("http://127.0.0.1:3000"));
    assert!(is_loopback_origin("https://localhost"));
    assert!(!is_loopback_origin("null"));
    assert!(!is_loopback_origin("http://user@localhost"));
    assert!(!is_loopback_origin("file://localhost"));
    assert!(!is_loopback_origin("http://localhost.evil.test"));
}

#[test]
fn the_nonce_is_32_to_128_hex_digits() {
    assert_eq!(
        nonce_of(&format!("nonce={}", "a".repeat(32))),
        Some("a".repeat(32).as_str())
    );
    assert_eq!(nonce_of("nonce=abc"), None);
    assert_eq!(nonce_of(&format!("nonce={}", "z".repeat(40))), None);
    assert_eq!(nonce_of(&format!("x=1&nonce={}", "0".repeat(129))), None);
}

fn tool(name: &str, schema: serde_json::Value) -> crate::tools::plugins::PluginToolInfo {
    crate::tools::plugins::PluginToolInfo {
        name: name.to_owned(),
        description: "A tool".to_owned(),
        input_schema: schema,
    }
}

#[test]
fn the_tools_revision_changes_with_a_plugin_tool_s_arguments() {
    // RB-092-06: a plugin updated live may keep a tool's name and change its arguments; the
    // client hears `notifications/tools/list_changed` and lists the tools again.
    let backend = crate::testing::FakeBackend::new(Vec::new());
    let empty = super::tools_revision(&backend);
    let v1 = serde_json::json!({ "type": "object", "properties": { "url": { "type": "string" } } });
    backend.set_plugin_tools(vec![tool("seo_audit", v1.clone())]);
    let first = super::tools_revision(&backend);
    assert_ne!(first, empty);
    backend.set_plugin_tools(vec![tool("seo_audit", v1.clone())]);
    assert_eq!(super::tools_revision(&backend), first, "the same list");
    let v2 =
        serde_json::json!({ "type": "object", "properties": { "depth": { "type": "integer" } } });
    backend.set_plugin_tools(vec![tool("seo_audit", v2)]);
    assert_ne!(super::tools_revision(&backend), first);
    // The order the tools come in is no change.
    backend.set_plugin_tools(vec![tool("a_one", v1.clone()), tool("b_two", v1.clone())]);
    let ordered = super::tools_revision(&backend);
    backend.set_plugin_tools(vec![tool("b_two", v1.clone()), tool("a_one", v1)]);
    assert_eq!(super::tools_revision(&backend), ordered);
}

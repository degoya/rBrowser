//! The acceptance fixtures of RB-093-03, run through the plugin's native traits against
//! `MemoryHost` (the host's rules for the capabilities, the project paths and the URLs): a
//! fixture site whose answers the host's `http` capability hands over, a project config with
//! its environments, Chromium views showing it.
//!
//! 1. A page without HSTS, with CSP `script-src 'unsafe-inline'` and a cookie without
//!    `Secure`/`SameSite`: exactly these three findings, the cookie's value in none of them.
//! 2. Under `http://localhost` a missing HSTS is not reported.
//! 3. Two environments with a different CSP: the comparison shows exactly that row as a
//!    difference.

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, CheckContext, HttpResponse, ViewInfo};
use security_headers::SecurityHeaders;
use serde_json::{Value, json};

const PROD: &str = "https://example.test/";
const STAGING: &str = "https://staging.example.test/";
const LOCAL: &str = "http://localhost:3000/";
const COOKIE_VALUE: &str = "c2Vzc2lvbi12YWx1ZS0xMjM0NQ";

/// Every header right but the three the first criterion names.
fn fixture_headers(csp: &str) -> Vec<(String, String)> {
    [
        ("Date", "Fri, 20 Nov 2026 10:00:00 GMT"),
        ("Content-Type", "text/html; charset=utf-8"),
        ("Content-Security-Policy", csp),
        ("X-Frame-Options", "SAMEORIGIN"),
        ("X-Content-Type-Options", "nosniff"),
        ("Referrer-Policy", "strict-origin-when-cross-origin"),
        ("Permissions-Policy", "geolocation=(), camera=()"),
    ]
    .iter()
    .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
    .collect()
}

fn answer(headers: Vec<(String, String)>) -> HttpResponse {
    HttpResponse {
        status: 200,
        headers,
        body: Vec::new(),
    }
}

fn view(id: u32, label: &str, width: u32, url: &str) -> ViewInfo {
    ViewInfo {
        id,
        label: label.to_owned(),
        breakpoint: label.to_lowercase(),
        engine: "chromium".to_owned(),
        width,
        height: 800,
        dpr: 1.0,
        url: url.to_owned(),
        title: "Header fixture".to_owned(),
    }
}

fn host() -> MemoryHost {
    let config = json!({ "environments": [
        { "id": "local", "url": "http://localhost:3000" },
        { "id": "staging", "url": "https://staging.example.test" },
        { "id": "prod", "url": "https://example.test" }
    ]});
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Cdp)
        .grant(Capability::Http)
        .grant(Capability::ProjectRead)
        .grant(Capability::Findings)
        .with_file(".rbrowser/config.json", config.to_string().as_bytes());
    host.cdp_domains = vec!["Network".to_owned()];
    host.cdp_answer = json!({ "cookies": [] }).to_string();
    host
}

fn call(host: &mut MemoryHost, tool: &str, args: &Value) -> Value {
    let text = SecurityHeaders
        .call_tool(host, tool, &args.to_string())
        .expect("answer");
    serde_json::from_str(&text).expect("json")
}

fn rules_of(findings: &Value) -> Vec<String> {
    findings
        .as_array()
        .expect("findings")
        .iter()
        .filter_map(|finding| finding["rule"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn a_page_without_hsts_with_unsafe_inline_and_a_loose_cookie_has_exactly_three_findings() {
    let mut host = host();
    let mut headers =
        fixture_headers("script-src 'self' 'unsafe-inline'; object-src 'none'; base-uri 'self'");
    headers.push((
        "Set-Cookie".to_owned(),
        format!("session={COOKIE_VALUE}; Path=/; HttpOnly"),
    ));
    host.responses.insert(PROD.to_owned(), answer(headers));
    host.views = vec![view(1, "Mobile", 390, PROD), view(2, "Desktop", 1440, PROD)];
    host.active = Some(1);

    let checked = call(&mut host, "check", &json!({}));
    assert_eq!(
        rules_of(&checked["findings"]),
        ["csp.unsafe", "hsts.missing", "cookie.flags"]
    );
    assert_eq!(checked["environment"], "prod");
    assert_eq!(
        checked["findings"][2]["params"]["missing"],
        "Secure, SameSite"
    );

    // The check run of `checks_run` over both views: still one finding per rule, no view.
    let ctx = CheckContext {
        view: None,
        url: None,
        rules: Vec::new(),
        args: "{}".to_owned(),
    };
    let findings = SecurityHeaders.run(&mut host, &ctx).expect("findings");
    assert_eq!(findings.len(), 3);
    assert!(findings.iter().all(|finding| finding.view.is_none()));

    // The command reports them; the cookie's value is in no finding and no answer.
    let reported = SecurityHeaders
        .run_command(&mut host, "check", "{}")
        .expect("reported");
    assert_eq!(host.findings.len(), 3);
    let everything = format!(
        "{checked}{findings:?}{reported}{:?}{:?}",
        host.findings, host.logs
    );
    assert!(!everything.contains(COOKIE_VALUE));
}

#[test]
fn a_missing_hsts_is_not_reported_under_http_localhost() {
    let mut host = host();
    host.responses.insert(
        LOCAL.to_owned(),
        answer(fixture_headers(
            "script-src 'self'; object-src 'none'; base-uri 'self'",
        )),
    );
    host.views = vec![view(1, "Desktop", 1440, LOCAL)];
    host.active = Some(1);
    let checked = call(&mut host, "check", &json!({}));
    assert_eq!(checked["environment"], "local");
    assert_eq!(rules_of(&checked["findings"]), Vec::<String>::new());
}

#[test]
fn two_environments_with_a_different_csp_differ_in_exactly_that_row() {
    let mut host = host();
    host.responses.insert(
        STAGING.to_owned(),
        answer(with_hsts(fixture_headers(
            "script-src 'self' 'unsafe-inline'; object-src 'none'; base-uri 'self'",
        ))),
    );
    host.responses.insert(
        PROD.to_owned(),
        answer(with_hsts(fixture_headers(
            "script-src 'self'; object-src 'none'; base-uri 'self'",
        ))),
    );
    host.views = vec![view(1, "Desktop", 1440, PROD)];
    host.active = Some(1);
    let table = call(
        &mut host,
        "compare",
        &json!({ "environments": ["staging", "prod"] }),
    );
    let differing: Vec<&str> = table["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter(|row| row["differs"] == json!(true))
        .filter_map(|row| row["header"].as_str())
        .collect();
    assert_eq!(differing, ["content-security-policy"]);
    assert_eq!(table["differences"], 1);
}

fn with_hsts(mut headers: Vec<(String, String)>) -> Vec<(String, String)> {
    headers.push((
        "Strict-Transport-Security".to_owned(),
        "max-age=31536000".to_owned(),
    ));
    headers
}

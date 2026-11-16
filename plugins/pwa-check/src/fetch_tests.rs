use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, HttpResponse};

use super::*;

fn answer(status: u16, headers: &[(&str, &str)], body: &[u8]) -> HttpResponse {
    HttpResponse {
        status,
        headers: headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        body: body.to_vec(),
    }
}

fn url(text: &str) -> Url {
    Url::parse(text).expect("url")
}

#[test]
fn a_same_origin_get_answers_with_lower_case_headers() {
    let mut host = MemoryHost::new().grant(Capability::Http);
    host.responses.insert(
        "https://a.test/m.json".to_owned(),
        answer(200, &[("Content-Type", "application/json")], b"{}"),
    );
    let budget = Budget::new(&mut host, 30_000);
    let page = url("https://a.test/");
    let Fetched::Ok(response) = get(
        &mut host,
        &budget,
        &page,
        &url("https://a.test/m.json"),
        ACCEPT_JSON,
    ) else {
        panic!("answered");
    };
    assert_eq!(response.status, 200);
    assert_eq!(response.header("content-type"), Some("application/json"));
    assert_eq!(host.fetched[0].method, "GET");
    assert_eq!(host.fetched[0].headers[0].0, "accept");
}

#[test]
fn another_origin_is_never_requested() {
    let mut host = MemoryHost::new().grant(Capability::Http);
    let budget = Budget::new(&mut host, 30_000);
    let fetched = get(
        &mut host,
        &budget,
        &url("https://a.test/"),
        &url("https://cdn.test/icon.png"),
        ACCEPT_IMAGE,
    );
    assert_eq!(
        fetched,
        Fetched::OtherOrigin("https://cdn.test/icon.png".to_owned())
    );
    assert!(host.fetched.is_empty());
}

#[test]
fn redirects_are_followed_within_the_origin_only() {
    let mut host = MemoryHost::new().grant(Capability::Http);
    host.responses.insert(
        "https://a.test/start".to_owned(),
        answer(302, &[("location", "/app/")], b""),
    );
    host.responses.insert(
        "https://a.test/app/".to_owned(),
        answer(200, &[], b"<html>"),
    );
    host.responses.insert(
        "https://a.test/away".to_owned(),
        answer(301, &[("location", "https://b.test/")], b""),
    );
    host.responses.insert(
        "https://a.test/loop".to_owned(),
        answer(302, &[("location", "/loop")], b""),
    );
    let page = url("https://a.test/");
    let budget = Budget::new(&mut host, 30_000);
    let Fetched::Ok(response) = get(
        &mut host,
        &budget,
        &page,
        &url("https://a.test/start"),
        ACCEPT_HTML,
    ) else {
        panic!("followed");
    };
    assert_eq!(response.url, "https://a.test/app/");
    assert_eq!(response.status, 200);
    assert_eq!(
        get(
            &mut host,
            &budget,
            &page,
            &url("https://a.test/away"),
            ACCEPT_HTML
        ),
        Fetched::OtherOrigin("https://b.test/".to_owned())
    );
    assert_eq!(
        get(
            &mut host,
            &budget,
            &page,
            &url("https://a.test/loop"),
            ACCEPT_HTML
        ),
        Fetched::Failed("redirects".to_owned())
    );
}

#[test]
fn a_refusal_keeps_the_hosts_code_and_the_budget_stops_requests() {
    let mut host = MemoryHost::new();
    let budget = Budget::new(&mut host, 30_000);
    let page = url("https://a.test/");
    assert_eq!(
        get(
            &mut host,
            &budget,
            &page,
            &url("https://a.test/x"),
            ACCEPT_HTML
        ),
        Fetched::Failed(rb_plugin_api::codes::CAPABILITY_DENIED.to_owned())
    );
    let mut host = MemoryHost::new().grant(Capability::Http);
    host.now = 100;
    let budget = Budget::new(&mut host, 5_000);
    host.now = 106;
    assert_eq!(
        get(
            &mut host,
            &budget,
            &page,
            &url("https://a.test/x"),
            ACCEPT_HTML
        ),
        Fetched::Failed("time".to_owned())
    );
    assert!(host.fetched.is_empty());
}

use rb_plugin_api::codes;
use serde_json::json;

use super::*;
use crate::fake::FakeHost;

fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", "https://site.test/")
        .page(1, &json!({}));
    host
}

fn urls(list: &[&str]) -> Vec<String> {
    list.iter().map(|url| (*url).to_owned()).collect()
}

fn at_host_pace() -> Settings {
    Settings {
        rps: MAX_RPS,
        ..Settings::default()
    }
}

#[test]
fn a_404_is_broken_and_a_200_ok() {
    let mut host = host();
    host.respond("HEAD", "https://site.test/ok", 200);
    let results = check(
        &mut host,
        &at_host_pace(),
        &urls(&["https://site.test/ok", "https://site.test/missing.html"]),
        Some(1),
    );
    assert_eq!(results[0].state, State::Ok);
    assert_eq!(results[1].state, State::Broken);
    assert_eq!(results[1].status, Some(404));
    assert!(results[1].is_broken());
    assert_eq!(results[1].describe(), "404");
}

#[test]
fn head_refused_falls_back_to_get() {
    let mut host = host();
    host.respond("HEAD", "https://site.test/a", 405)
        .respond("GET", "https://site.test/a", 200)
        .refuse("HEAD", "https://site.test/b", codes::HTTP_FAILED)
        .respond("GET", "https://site.test/b", 200);
    let results = check(
        &mut host,
        &at_host_pace(),
        &urls(&["https://site.test/a", "https://site.test/b"]),
        Some(1),
    );
    assert_eq!(results[0].state, State::Ok);
    assert_eq!(results[1].state, State::Ok);
    assert_eq!(
        host.fetched
            .iter()
            .map(|(method, _)| method.as_str())
            .collect::<Vec<_>>(),
        ["HEAD", "GET", "HEAD", "GET"]
    );
}

#[test]
fn redirects_are_followed_but_never_into_the_local_network() {
    let mut host = host();
    host.redirect("HEAD", "https://site.test/old", "/new")
        .respond("HEAD", "https://site.test/new", 200)
        .redirect("HEAD", "https://site.test/sneaky", "http://127.0.0.1/admin")
        .redirect("HEAD", "https://site.test/loop", "https://site.test/loop");
    let results = check(
        &mut host,
        &at_host_pace(),
        &urls(&[
            "https://site.test/old",
            "https://site.test/sneaky",
            "https://site.test/loop",
        ]),
        Some(1),
    );
    assert_eq!(results[0].state, State::Ok);
    assert_eq!(
        results[0].final_url.as_deref(),
        Some("https://site.test/new")
    );
    assert_eq!(results[1].state, State::RedirectError);
    assert_eq!(results[2].state, State::RedirectError);
    assert!(
        !host
            .fetched
            .iter()
            .any(|(_, url)| url.contains("127.0.0.1"))
    );
}

#[test]
fn a_denied_address_is_not_broken_and_other_schemes_are_skipped() {
    let mut host = host();
    host.refuse("HEAD", "https://intranet.test/", codes::HTTP_URL_DENIED);
    let results = check(
        &mut host,
        &at_host_pace(),
        &urls(&[
            "https://intranet.test/",
            "mailto:a@b.test",
            "https://site.test/x#a",
            "https://site.test/x#b",
        ]),
        Some(1),
    );
    assert_eq!(results.len(), 3, "fragments are one link");
    assert_eq!(results[0].state, State::Denied);
    assert!(!results[0].is_broken());
    assert_eq!(results[1].state, State::Skipped);
}

#[test]
fn below_the_host_pace_each_further_request_waits_a_whole_interval() {
    let mut host = host();
    let settings = Settings {
        rps: 2,
        ..Settings::default()
    };
    check(
        &mut host,
        &settings,
        &urls(&[
            "https://site.test/a",
            "https://site.test/b",
            "https://site.test/c",
        ]),
        Some(1),
    );
    assert_eq!(host.fetched.len(), 3);
    // Two waits of 500 ms between three requests: at most two requests a second.
    let waits = host.waits();
    assert_eq!(waits.len(), 2);
    assert!(
        waits
            .iter()
            .all(|wait| wait.contains("setTimeout(done, 500)"))
    );
}

#[test]
fn at_the_host_pace_nothing_waits() {
    let mut host = host();
    check(
        &mut host,
        &at_host_pace(),
        &urls(&["https://site.test/a", "https://site.test/b"]),
        Some(1),
    );
    assert!(host.waits().is_empty());
}

#[test]
fn without_a_view_to_wait_in_a_lower_pace_requests_nothing() {
    let mut host = host();
    let settings = Settings {
        rps: 1,
        ..Settings::default()
    };
    let results = check(&mut host, &settings, &urls(&["https://site.test/a"]), None);
    assert!(host.fetched.is_empty());
    assert_eq!(results[0].state, State::Unchecked);
    assert_eq!(results[0].reason.as_deref(), Some("pace"));
}

#[test]
fn the_budget_stops_further_requests() {
    let mut host = host();
    host.tick = 2;
    let settings = Settings {
        rps: MAX_RPS,
        timeout_ms: 3_000,
        ..Settings::default()
    };
    let results = check(
        &mut host,
        &settings,
        &urls(&[
            "https://site.test/a",
            "https://site.test/b",
            "https://site.test/c",
        ]),
        Some(1),
    );
    // Readings 0 (start), 2, 4: the third request would start after 4 s.
    assert_eq!(host.fetched.len(), 2);
    assert_eq!(results[2].state, State::Unchecked);
    assert_eq!(results[2].reason.as_deref(), Some("budget"));
}

#[test]
fn distinct_caps_the_list() {
    let many: Vec<String> = (0..MAX_LINKS + 20)
        .map(|n| format!("https://site.test/{n}"))
        .collect();
    assert_eq!(distinct(&many).len(), MAX_LINKS);
}

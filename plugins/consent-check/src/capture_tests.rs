use rb_plugin_api::Capability;
use rb_plugin_api::test_support::MemoryHost;
use serde_json::json;

use super::*;
use crate::testing::{self, PAGE, YEAR};

#[test]
fn the_probe_capture_is_read_without_any_cookie_value() {
    let args = testing::probe_args(true);
    let run = from_probe(&args).expect("a probe capture");
    assert_eq!(run.mode, Mode::Probe);
    assert_eq!(run.url, PAGE);
    assert_eq!(run.view, None);
    assert_eq!(run.before.requests.len(), 3);
    assert_eq!(run.before.cookies.len(), 1);
    let cookie = &run.before.cookies[0];
    assert_eq!(cookie.name, "_track");
    assert_eq!(cookie.domain, ".shop.example.test");
    assert_eq!(cookie.expires, Some(testing::NOW + YEAR));
    assert!(
        !format!("{run:?}").contains(testing::SECRET),
        "no value is kept"
    );
    assert_eq!(run.banner.cmp.as_deref(), Some("onetrust"));
    assert!(run.banner.rejected);
    assert_eq!(run.session_discarded, Some(true));
    assert!(run.after_reject.is_some());
}

#[test]
fn no_probe_capture_without_the_hosts_key() {
    assert!(from_probe(&json!({})).is_none());
    assert!(from_probe(&json!({ PROBE_KEY: "nonsense" })).is_none());
    assert!(
        from_probe(&json!({ PROBE_KEY: { "before": {} } })).is_none(),
        "a URL is needed"
    );
}

#[test]
fn a_session_cookie_has_no_expiry_and_long_lists_are_cut() {
    let cookies: Vec<_> = (0..MAX_COOKIES + 5)
        .map(|n| json!({ "name": format!("c{n}"), "domain": "example.test", "expires": -1, "session": true }))
        .collect();
    let args = json!({ PROBE_KEY: { "url": PAGE, "before": { "cookies": cookies } } });
    let run = from_probe(&args).expect("a probe capture");
    assert_eq!(run.before.cookies.len(), MAX_COOKIES);
    assert!(
        run.before
            .cookies
            .iter()
            .all(|cookie| cookie.expires.is_none())
    );
    assert!(run.after_reject.is_none());
}

fn view_host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Evaluate)
        .grant(Capability::Cdp);
    host.cdp_domains = vec!["Network".to_owned()];
    testing::view(&mut host, 3, PAGE);
    host.cdp_answer = json!({ "cookies": [
        { "name": "_track", "value": testing::SECRET, "domain": ".shop.example.test",
          "path": "/", "expires": testing::NOW + YEAR, "session": false },
    ] })
    .to_string();
    host.evaluate_answer = testing::collected(PAGE, &["_track_id"], Some("onetrust"));
    host
}

#[test]
fn the_view_capture_reads_the_current_documents_requests_its_cookies_and_storage_keys() {
    let mut host = view_host();
    let view = host.views[0].clone();
    let run = from_view(&mut host, &view, &testing::feed_args(3), testing::NOW).expect("a run");
    assert_eq!(run.mode, Mode::View);
    assert_eq!(run.view, Some(3));
    let urls: Vec<&str> = run
        .before
        .requests
        .iter()
        .map(|request| request.url.as_str())
        .collect();
    assert!(
        !urls.iter().any(|url| url.contains("old-page")),
        "a request of the document before is left out: {urls:?}"
    );
    assert!(urls.contains(&"https://cdn.tracker.test/t.js"));
    assert_eq!(run.before.cookies.len(), 1);
    assert_eq!(run.before.storage, vec!["_track_id".to_owned()]);
    assert_eq!(run.banner.cmp.as_deref(), Some("onetrust"));
    assert!(!run.banner.rejected);
    assert!(run.after_reject.is_none());
    assert!(!format!("{run:?}").contains(testing::SECRET));
}

#[test]
fn the_view_capture_only_reads_the_view() {
    let mut host = view_host();
    let view = host.views[0].clone();
    from_view(&mut host, &view, &testing::feed_args(3), testing::NOW).expect("a run");
    assert_eq!(host.cdp_sent.len(), 1);
    assert_eq!(host.cdp_sent[0].1, "Network.getCookies");
    let params: serde_json::Value =
        serde_json::from_str(&host.cdp_sent[0].2).expect("params are JSON");
    assert_eq!(params, json!({ "urls": [PAGE] }));
    assert_eq!(host.evaluated.len(), 1);
    assert!(
        host.evaluated[0].1.contains("\"mode\":\"read\""),
        "the view's banner is never clicked"
    );
}

#[test]
fn unread_cookies_are_a_caveat_not_a_failure() {
    let mut host = view_host();
    host.cdp_domains.clear();
    let view = host.views[0].clone();
    let run = from_view(&mut host, &view, &testing::feed_args(3), testing::NOW).expect("a run");
    assert!(run.before.cookies.is_empty());
    assert!(run.cookies_unread);
}

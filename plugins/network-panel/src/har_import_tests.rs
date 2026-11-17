use serde_json::json;

use super::*;
use crate::fake_host::FakeHost;
use crate::har;

#[test]
fn a_url_escapes_into_a_glob_matching_only_itself() {
    assert_eq!(
        escape(r"http://a.test/x?a=*&b=\"),
        r"http://a.test/x\?a=\*&b=\\"
    );
}

#[test]
fn an_exported_har_imports_as_replay_mocks() {
    let mut host = har::tests::host();
    let exported =
        har::export(&mut host, &har::tests::capture(), None, true, true).expect("written");
    let path = exported["path"].as_str().expect("a path").to_owned();
    let answer = import(&mut host, &path).expect("imported");
    assert_eq!(
        answer["path"],
        ".rbrowser/out/network-panel/mocks/har-2024-11-11T04-40-00Z.json"
    );
    assert_eq!(
        answer["rules"], 1,
        "one rule per method and URL, the failure skipped"
    );
    assert_eq!(answer["skipped"], 1);
    let text = host
        .out("mocks/har-2024-11-11T04-40-00Z.json")
        .expect("written");
    let file = mocks::parse_file(text.as_bytes(), "p", "own:har", true);
    assert!(file.problems.is_empty(), "{:?}", file.problems);
    let rule = &file.rules[0];
    assert_eq!(rule.url_pattern, r"http://127.0.0.1:8080/api/user\?id=7&x");
    assert_eq!(rule.method.as_deref(), Some("GET"));
    assert_eq!(
        rule.kind,
        Kind::Mock {
            status: 200,
            headers: vec![("content-type".to_owned(), "application/json".to_owned())],
            body: Body::Text("{\"name\":\"Ada\"}".to_owned()),
            delay_ms: 0,
        }
    );
}

#[test]
fn base64_bodies_and_dropped_headers() {
    let har = json!({ "log": { "version": "1.2", "entries": [{
        "request": { "method": "GET", "url": "https://a.test/logo.png" },
        "response": { "status": 200, "headers": [
            { "name": "Content-Encoding", "value": "gzip" },
            { "name": "Content-Type", "value": "image/png" },
            { "name": "Content-Length", "value": "4" } ],
          "content": { "mimeType": "image/png", "text": "iVBO", "encoding": "base64" } } }] } });
    let (rules, skipped) = rules_of(har.to_string().as_bytes()).expect("a HAR");
    assert_eq!(skipped, 0);
    assert_eq!(
        rules[0].kind,
        Kind::Mock {
            status: 200,
            headers: vec![("Content-Type".to_owned(), "image/png".to_owned())],
            body: Body::Base64("iVBO".to_owned()),
            delay_ms: 0,
        }
    );
}

#[test]
fn only_a_har_below_the_rbrowser_folder_imports() {
    let mut host = FakeHost::default();
    for path in [
        "site.har",
        ".rbrowser/../x.har",
        ".rbrowser/x.json",
        "/tmp/a.har",
        ".rbrowser/.har",
    ] {
        let refused = import(&mut host, path).expect_err(path);
        assert_eq!(
            refused.code.as_deref(),
            Some("plugin.network-panel.errors.har_path"),
            "{path}"
        );
    }
    let missing = import(&mut host, ".rbrowser/har/missing.har").expect_err("missing");
    assert_eq!(
        missing.code.as_deref(),
        Some("plugin.network-panel.errors.har_read")
    );
    host.file(".rbrowser/har/bad.har", "{}");
    let bad = import(&mut host, ".rbrowser/har/bad.har").expect_err("no HAR");
    assert_eq!(
        bad.code.as_deref(),
        Some("plugin.network-panel.errors.har_invalid")
    );
}

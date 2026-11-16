use serde_json::json;

use super::*;
use crate::fake_host::FakeHost;

const API: &str = r#"{ "rules": [
  { "id": "user-500", "match": { "urlPattern": "*/api/user", "method": "get" },
    "mock": { "status": 500, "headers": { "content-type": "application/json" },
              "body": { "error": "down" }, "delayMs": 800 } },
  { "id": "no-tracking", "enabled": false, "match": { "urlPattern": "*/tracking.js" }, "block": true },
  { "id": "header", "match": { "urlPattern": "^https?://[^/]+/api/", "regex": true },
    "rewrite": { "headers": { "x-test": "1" } } },
  { "id": "file", "match": { "urlPattern": "*/data.json" }, "mock": { "bodyFile": "bodies/data.json" } }
] }"#;

#[test]
fn a_mock_file_reads_its_three_kinds() {
    let file = parse_file(
        API.as_bytes(),
        ".rbrowser/mocks/api.json",
        "api.json",
        false,
    );
    assert!(file.problems.is_empty(), "{:?}", file.problems);
    let [mock, block, rewrite, from_file] = file.rules.as_slice() else {
        panic!("four rules");
    };
    assert_eq!(file.qualified(mock), "api.json#user-500");
    assert_eq!(mock.method.as_deref(), Some("GET"));
    assert_eq!(
        mock.kind,
        Kind::Mock {
            status: 500,
            headers: vec![("content-type".to_owned(), "application/json".to_owned())],
            body: Body::Text(r#"{"error":"down"}"#.to_owned()),
            delay_ms: 800,
        }
    );
    assert!(!block.enabled && block.kind == Kind::Block);
    assert!(rewrite.regex);
    assert_eq!(
        from_file.kind,
        Kind::Mock {
            status: 200,
            headers: vec![],
            body: Body::File("bodies/data.json".to_owned()),
            delay_ms: 0
        }
    );
}

#[test]
fn a_body_file_must_stay_inside_the_mocks_folder() {
    for bad in [
        "../secret.json",
        "/etc/passwd",
        "C:/x",
        "a/../../b",
        "a\\b",
        "./a",
        "a//b",
        "",
    ] {
        let raw = json!({ "id": "x", "match": { "urlPattern": "*" }, "mock": { "bodyFile": bad } });
        assert!(parse_rule(&raw, 0).is_err(), "{bad}");
    }
    assert!(relative_path("bodies/user.json"));
}

#[test]
fn bad_rules_are_named_and_the_others_read() {
    let text = json!({ "rules": [
        { "id": "a", "match": { "urlPattern": "*" }, "block": true },
        { "id": "a", "match": { "urlPattern": "*" }, "block": true },
        { "id": "both", "match": { "urlPattern": "*" }, "block": true, "mock": {} },
        { "id": "none", "match": { "urlPattern": "*" } },
        { "id": "status", "match": { "urlPattern": "*" }, "mock": { "status": 700 } },
        { "id": "bad id!", "match": { "urlPattern": "*" }, "block": true },
        { "id": "nourl", "match": {}, "block": true },
    ] })
    .to_string();
    let file = parse_file(text.as_bytes(), "m.json", "m.json", false);
    assert_eq!(file.rules.len(), 1);
    assert_eq!(file.problems.len(), 6, "{:?}", file.problems);
    assert!(parse_file(b"[]", "x", "x", false).problems[0].contains("no JSON object"));
}

#[test]
fn a_rule_written_out_reads_back_the_same() {
    let file = parse_file(API.as_bytes(), "p", "p", false);
    let again = parse_file(
        file_json(&file.rules).to_string().as_bytes(),
        "p",
        "p",
        false,
    );
    assert_eq!(again.rules, file.rules);
}

#[test]
fn the_project_and_the_own_files_load_and_become_host_rules() {
    let mut host = FakeHost::default();
    host.file(".rbrowser/mocks/api.json", API);
    host.file(".rbrowser/mocks/bodies/data.json", "[1,2]");
    host.file(".rbrowser/mocks/readme.txt", "not a mock");
    host.file(
        ".rbrowser/out/network-panel/mocks/added.json",
        r#"{"rules":[{"id":"r1","match":{"urlPattern":"*/x"},"block":true}]}"#,
    );
    let files = load(&mut host, ".rbrowser/mocks");
    let prefixes: Vec<&str> = files.iter().map(|file| file.prefix.as_str()).collect();
    assert_eq!(prefixes, ["api.json", "own:added.json"]);
    let rule = host_rule(&mut host, &files[0], &files[0].rules[3], ".rbrowser/mocks")
        .expect("the body file reads");
    assert_eq!(rule["id"], "api.json#file");
    assert_eq!(rule["origin"], "project");
    assert_eq!(rule["action"]["bodyBase64"], "WzEsMl0=");
    let mock = host_rule(&mut host, &files[0], &files[0].rules[0], ".rbrowser/mocks").expect("ok");
    assert_eq!(
        mock["action"],
        json!({ "type": "mock", "status": 500, "headers": [["content-type", "application/json"]],
                "bodyBase64": "eyJlcnJvciI6ImRvd24ifQ==", "delayMs": 800 })
    );
    assert_eq!(
        mock["match"],
        json!({ "urlPattern": "*/api/user", "regex": false, "method": "GET" })
    );
}

#[test]
fn a_missing_body_file_is_a_problem_of_its_rule() {
    let mut host = FakeHost::default();
    let file = parse_file(API.as_bytes(), "p", "p", false);
    assert!(host_rule(&mut host, &file, &file.rules[3], ".rbrowser/mocks").is_err());
}

#[test]
fn file_names_become_safe_prefixes() {
    assert_eq!(prefix_of("my mocks (1).json"), "my_mocks__1_.json");
}

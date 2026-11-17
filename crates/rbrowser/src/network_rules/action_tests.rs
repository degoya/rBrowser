use serde_json::json;

use super::*;

fn set(name: &str, value: &str) -> HeaderOp {
    HeaderOp::Set(name.to_owned(), value.to_owned())
}

#[test]
fn a_legacy_rewrite_reads_its_header_pairs_as_set_operations() {
    let action = parse(&json!({ "type": "rewrite", "headers": [["x-test", "1"]] })).expect("reads");
    assert_eq!(
        action,
        Action::Rewrite(Rewrite {
            url: None,
            headers: vec![set("x-test", "1")],
            response_headers: Vec::new(),
            query: Vec::new(),
            body: None,
        })
    );
    assert!(
        parse(&json!({ "type": "rewrite" })).is_err(),
        "changes nothing"
    );
}

#[test]
fn an_extended_rewrite_reads_header_query_and_body_operations() {
    let action = parse(&json!({
        "type": "rewrite",
        "requestHeaders": [{ "op": "add", "name": "x-a", "value": "1" }, { "op": "remove", "name": "x-old" }],
        "responseHeaders": [{ "op": "remove", "name": "content-security-policy" }],
        "query": [{ "op": "set", "name": "debug", "value": "1" }, { "op": "remove", "name": "utm_source" }],
        "body": { "regex": "Hello (\\w+)", "replacement": "Bye $1" },
    }))
    .expect("reads");
    let Action::Rewrite(rewrite) = action else {
        panic!("a rewrite");
    };
    assert_eq!(
        rewrite.headers,
        vec![
            HeaderOp::Add("x-a".to_owned(), "1".to_owned()),
            HeaderOp::Remove("x-old".to_owned())
        ]
    );
    assert_eq!(
        rewrite.response_headers,
        vec![HeaderOp::Remove("content-security-policy".to_owned())]
    );
    assert_eq!(
        rewrite.query,
        vec![
            QueryOp::Set("debug".to_owned(), "1".to_owned()),
            QueryOp::Remove("utm_source".to_owned())
        ]
    );
    let body = rewrite.body.as_ref().expect("a body replacement");
    assert_eq!(body.regex.as_str(), "Hello (\\w+)");
    assert_eq!(body.replacement, "Bye $1");
    assert!(rewrite.acts_on_response() && rewrite.acts_on_request());
}

#[test]
fn a_rewrite_with_response_parts_only_acts_on_the_response_alone() {
    let Action::Rewrite(rewrite) = parse(&json!({ "type": "rewrite",
        "responseHeaders": [{ "op": "set", "name": "x-served-by", "value": "rbrowser" }] }))
    .expect("reads") else {
        panic!("a rewrite");
    };
    assert!(rewrite.acts_on_response() && !rewrite.acts_on_request());
}

#[test]
fn bad_rewrite_operations_are_refused() {
    for action in [
        json!({ "type": "rewrite", "requestHeaders": [{ "op": "shout", "name": "x" }] }),
        json!({ "type": "rewrite", "requestHeaders": [{ "op": "set", "name": "x" }] }),
        json!({ "type": "rewrite", "requestHeaders": [{ "op": "set", "name": "bad name", "value": "1" }] }),
        json!({ "type": "rewrite", "query": [{ "op": "set", "name": "", "value": "1" }] }),
        json!({ "type": "rewrite", "body": { "regex": "(", "replacement": "" } }),
        json!({ "type": "rewrite", "body": { "replacement": "x" } }),
        json!({ "type": "rewrite", "url": "ftp://x" }),
    ] {
        assert!(parse(&action).is_err(), "{action}");
    }
}

#[test]
fn map_local_reads_a_folder_or_a_file_with_an_optional_mime() {
    assert_eq!(
        parse(&json!({ "type": "mapLocal", "folder": "fixtures/api" })).expect("reads"),
        Action::MapLocal(MapLocal {
            folder: Some("fixtures/api".to_owned()),
            file: None,
            mime: None,
            resolved: None,
        })
    );
    assert_eq!(
        parse(&json!({ "type": "mapLocal", "file": "C:\\sites\\shop\\config.json", "mime": "application/json" }))
            .expect("reads"),
        Action::MapLocal(MapLocal {
            folder: None,
            file: Some("C:\\sites\\shop\\config.json".to_owned()),
            mime: Some("application/json".to_owned()),
            resolved: None,
        })
    );
    for action in [
        json!({ "type": "mapLocal" }),
        json!({ "type": "mapLocal", "folder": "a", "file": "b" }),
        json!({ "type": "mapLocal", "folder": "" }),
        json!({ "type": "mapLocal", "folder": "a\u{0}b" }),
        json!({ "type": "mapLocal", "folder": "a", "mime": "no slash" }),
    ] {
        assert!(parse(&action).is_err(), "{action}");
    }
}

#[test]
fn map_remote_reads_both_endpoints() {
    let action = parse(&json!({ "type": "mapRemote",
        "from": { "scheme": "https", "host": "api.example.com", "path": "/v1" },
        "to": { "scheme": "http", "host": "localhost", "port": 3001, "path": "/api" } }))
    .expect("reads");
    assert_eq!(
        action,
        Action::MapRemote {
            from: Endpoint {
                scheme: Some("https".to_owned()),
                host: "api.example.com".to_owned(),
                port: None,
                path: Some("/v1".to_owned()),
            },
            to: Endpoint {
                scheme: Some("http".to_owned()),
                host: "localhost".to_owned(),
                port: Some(3001),
                path: Some("/api".to_owned()),
            },
        }
    );
    for action in [
        json!({ "type": "mapRemote", "from": { "host": "a" } }),
        json!({ "type": "mapRemote", "from": { "host": "" }, "to": { "host": "b" } }),
        json!({ "type": "mapRemote", "from": { "host": "a" }, "to": { "host": "b", "port": 70000 } }),
        json!({ "type": "mapRemote", "from": { "host": "a", "scheme": "ftp" }, "to": { "host": "b" } }),
        json!({ "type": "mapRemote", "from": { "host": "a", "path": "no-slash" }, "to": { "host": "b" } }),
        json!({ "type": "mapRemote", "from": { "host": "a b" }, "to": { "host": "b" } }),
    ] {
        assert!(parse(&action).is_err(), "{action}");
    }
}

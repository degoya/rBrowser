use serde_json::json;

use super::*;

#[test]
fn an_extended_rewrite_reads_and_translates_both_ways() {
    let raw = json!({ "rewrite": {
        "url": "https://b.test/x", "headers": { "x-set": "1" },
        "requestHeaders": [{ "op": "remove", "name": "x-old" }],
        "responseHeaders": [{ "op": "set", "name": "x-r", "value": 7 }],
        "query": [{ "op": "set", "name": "debug", "value": "1" }, { "op": "remove", "name": "utm" }],
        "body": { "regex": "a", "replacement": "b" },
    } });
    let kind = parse(&raw).expect("reads");
    let Kind::Rewrite {
        response_headers, ..
    } = &kind
    else {
        panic!("a rewrite");
    };
    assert_eq!(response_headers[0].value.as_deref(), Some("7"));
    let action = host_action(&kind, "");
    assert_eq!(action["type"], "rewrite");
    assert_eq!(action["url"], "https://b.test/x");
    assert_eq!(action["headers"], json!([["x-set", "1"]]));
    assert_eq!(
        action["requestHeaders"],
        json!([{ "op": "remove", "name": "x-old" }])
    );
    assert_eq!(
        action["responseHeaders"],
        json!([{ "op": "set", "name": "x-r", "value": "7" }])
    );
    assert_eq!(action["query"][1], json!({ "op": "remove", "name": "utm" }));
    assert_eq!(action["body"], json!({ "regex": "a", "replacement": "b" }));
    let mut out = json!({});
    write_json(&kind, &mut out);
    assert_eq!(parse(&out).expect("reads back"), kind);
}

#[test]
fn map_local_and_map_remote_read_and_translate() {
    let local =
        parse(&json!({ "mapLocal": { "folder": "fixtures/api", "mime": "application/json" } }))
            .expect("reads");
    assert_eq!(
        host_action(&local, ""),
        json!({ "type": "mapLocal", "folder": "fixtures/api", "mime": "application/json" })
    );
    let remote = parse(&json!({ "mapRemote": {
        "from": { "host": "api.example.com", "path": "/v1" },
        "to": { "scheme": "http", "host": "localhost", "port": 3001, "ignored": true } } }))
    .expect("reads");
    let action = host_action(&remote, "");
    assert_eq!(action["type"], "mapRemote");
    assert_eq!(
        action["to"],
        json!({ "scheme": "http", "host": "localhost", "port": 3001 })
    );
    for kind in [&local, &remote] {
        let mut out = json!({});
        write_json(kind, &mut out);
        assert_eq!(&parse(&out).expect("reads back"), kind);
    }
}

#[test]
fn bad_kinds_name_their_problem() {
    let bad = |raw: Value| parse(&raw).expect_err("refused");
    assert!(bad(json!({})).contains("exactly one"));
    assert!(bad(json!({ "block": true, "mock": {} })).contains("exactly one"));
    assert!(bad(json!({ "mapLocal": {} })).contains("one of folder, file"));
    assert!(
        bad(json!({ "mapLocal": { "folder": "a", "file": "b" } })).contains("one of folder, file")
    );
    assert!(bad(json!({ "mapRemote": { "from": { "host": "a" } } })).contains("mapRemote.to"));
    assert!(
        bad(json!({ "mapRemote": { "from": {}, "to": { "host": "b" } } }))
            .contains("host is missing")
    );
    assert!(
        bad(json!({ "rewrite": { "query": [{ "op": "set", "name": "x" }] } }))
            .contains("needs a value")
    );
    assert!(
        bad(json!({ "rewrite": { "requestHeaders": [{ "op": "shout", "name": "x" }] } }))
            .contains("op is")
    );
    assert!(bad(json!({ "rewrite": { "body": { "replacement": "x" } } })).contains("regex"));
    assert!(bad(json!({ "mock": { "status": 42 } })).contains("status"));
}

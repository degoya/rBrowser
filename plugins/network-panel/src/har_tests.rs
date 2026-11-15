use serde_json::json;

use super::*;
use crate::fake_host::{FakeHost, view};

pub(crate) fn capture() -> Value {
    json!({ "$console": { "views": [
        { "view": 1, "network": [
            { "id": "1.1", "url": "http://127.0.0.1:8080/api/user?id=7&x", "method": "GET",
              "resourceType": "Fetch", "status": 200, "mimeType": "application/json",
              "size": 13.0, "time": 1_731_300_000_123.0, "durationMs": 41.5, "fromCache": false },
            { "id": "1.2", "url": "http://127.0.0.1:8080/tracking.js", "method": "GET",
              "resourceType": "Script", "time": 1_731_300_000_200.0, "durationMs": 1.0,
              "fromCache": false, "failed": "net::ERR_BLOCKED_BY_CLIENT" },
        ] },
        { "view": 2, "network": [
            { "id": "2.1", "url": "http://127.0.0.1:8080/api/user?id=7&x", "method": "GET",
              "resourceType": "Fetch", "status": 200, "mimeType": "application/json",
              "size": 13.0, "time": 1_731_300_000_100.0, "durationMs": 40.0, "fromCache": false },
        ] },
    ] } })
}

pub(crate) fn host() -> FakeHost {
    let mut host = FakeHost::with_views(vec![view(1, "Mobile", 390), view(2, "Desktop", 1440)]);
    host.bodies.insert(
        (1, "1.1".to_owned()),
        json!({ "body": "{\"name\":\"Ada\"}", "base64Encoded": false }).to_string(),
    );
    host
}

/// The fields HAR 1.2 requires (`http://www.softwareishard.com/blog/har-12-spec/`), with their
/// types; a custom field starts with `_`.
pub(crate) fn assert_har_1_2(har: &Value) {
    let log = &har["log"];
    assert_eq!(log["version"], "1.2");
    assert!(log["creator"]["name"].is_string() && log["creator"]["version"].is_string());
    for entry in log["entries"].as_array().expect("entries") {
        let date = entry["startedDateTime"].as_str().expect("startedDateTime");
        assert!(
            date.len() == 24 && date.ends_with('Z') && date.as_bytes()[10] == b'T',
            "{date}"
        );
        assert!(entry["time"].as_f64().is_some_and(|t| t >= 0.0));
        let request = &entry["request"];
        for key in ["method", "url", "httpVersion"] {
            assert!(request[key].is_string(), "request.{key}");
        }
        for key in ["cookies", "headers", "queryString"] {
            assert!(request[key].is_array(), "request.{key}");
        }
        assert!(request["headersSize"].is_number() && request["bodySize"].is_number());
        let response = &entry["response"];
        assert!(response["status"].is_u64());
        for key in ["statusText", "httpVersion", "redirectURL"] {
            assert!(response[key].is_string(), "response.{key}");
        }
        assert!(response["cookies"].is_array() && response["headers"].is_array());
        assert!(response["content"]["size"].is_number());
        assert!(response["content"]["mimeType"].is_string());
        assert!(response["headersSize"].is_number() && response["bodySize"].is_number());
        assert!(entry["cache"].is_object());
        for key in ["send", "wait", "receive"] {
            assert!(
                entry["timings"][key].as_f64().is_some_and(|t| t >= -1.0),
                "timings.{key}"
            );
        }
        for key in entry.as_object().expect("object").keys() {
            assert!(
                [
                    "startedDateTime",
                    "time",
                    "request",
                    "response",
                    "cache",
                    "timings",
                    "pageref",
                    "serverIPAddress",
                    "connection",
                    "comment"
                ]
                .contains(&key.as_str())
                    || key.starts_with('_'),
                "unknown field {key}"
            );
        }
    }
}

#[test]
fn dates_and_stamps_are_utc() {
    assert_eq!(iso8601(0.0), "1970-01-01T00:00:00.000Z");
    assert_eq!(iso8601(1_731_300_000_123.0), "2024-11-11T04:40:00.123Z");
    assert_eq!(iso8601(951_782_400_000.0), "2000-02-29T00:00:00.000Z");
    assert_eq!(stamp(1_731_300_000), "2024-11-11T04-40-00Z");
}

#[test]
fn a_merged_export_is_a_valid_har_with_every_views_requests_in_time_order() {
    let mut host = host();
    let answer = export(&mut host, &capture(), None, true, true).expect("written");
    assert_eq!(
        answer["path"],
        ".rbrowser/out/network-panel/2024-11-11T04-40-00Z.har"
    );
    assert_eq!(answer["entries"], 3);
    let har: Value = serde_json::from_str(&host.out("2024-11-11T04-40-00Z.har").expect("the file"))
        .expect("JSON");
    assert_har_1_2(&har);
    let entries = har["log"]["entries"].as_array().expect("entries");
    let views: Vec<u64> = entries.iter().filter_map(|e| e["_view"].as_u64()).collect();
    assert_eq!(views, [2, 1, 1]);
    assert_eq!(entries[1]["_viewLabel"], "Mobile");
    assert_eq!(
        entries[1]["response"]["content"]["text"],
        "{\"name\":\"Ada\"}"
    );
    assert_eq!(
        entries[1]["request"]["queryString"],
        json!([{ "name": "id", "value": "7" }, { "name": "x", "value": "" }])
    );
    assert_eq!(entries[2]["_error"], "net::ERR_BLOCKED_BY_CLIENT");
    assert_eq!(entries[2]["response"]["status"], 0);
}

#[test]
fn an_export_per_view_writes_one_file_each() {
    let mut host = host();
    let answer = export(&mut host, &capture(), None, false, false).expect("written");
    assert_eq!(
        answer["paths"],
        json!([
            ".rbrowser/out/network-panel/2024-11-11T04-40-00Z-view1.har",
            ".rbrowser/out/network-panel/2024-11-11T04-40-00Z-view2.har"
        ])
    );
    assert!(host.cdp_calls.is_empty(), "no bodies asked for");
}

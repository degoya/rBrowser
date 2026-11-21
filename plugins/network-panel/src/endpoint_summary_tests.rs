use serde_json::json;

use super::{MAX_REQUESTS, summary};
use crate::calls::Response;
use crate::endpoints::tests::call;
use crate::endpoints::{Options, group};

#[test]
fn a_summary_counts_times_sizes_statuses_caching_and_compression() {
    let mut first = call(
        1,
        "GET",
        "https://a.test/api/users/1?token=secret",
        1.0,
        100.0,
    );
    first.size = Some(400.0);
    first.decoded_size = Some(2000.0);
    first.response = Some(Response {
        ttfb_ms: Some(80.0),
        encoding: Some("br".to_owned()),
        cache_control: Some("no-cache".to_owned()),
        etag: true,
        ..Response::default()
    });
    let mut second = call(2, "GET", "https://a.test/api/users/2", 2.0, 300.0);
    second.status = Some(404);
    second.size = Some(100.0);
    second.response = Some(Response::default());
    let mut third = call(2, "GET", "https://a.test/api/users/3", 3.0, 0.0);
    third.status = None;
    third.failed = true;
    third.duration_ms = None;
    third.from_cache = true;
    let found = [first, second, third];
    let groups = group(&found, &Options::default());
    let answer = summary(&groups[0]);
    assert_eq!(answer["key"], "GET https://a.test/api/users/{id}");
    assert_eq!(answer["calls"], 3);
    assert_eq!(answer["failed"], 1);
    assert_eq!(answer["fromCache"], 1);
    assert_eq!(
        answer["durationMs"],
        json!({ "median": 200.0, "p95": 300.0 })
    );
    assert_eq!(answer["ttfbMs"], json!({ "median": 80.0, "p95": 80.0 }));
    assert_eq!(
        answer["transferred"],
        json!({ "total": 500.0, "median": 250.0 })
    );
    assert_eq!(
        answer["decoded"],
        json!({ "total": 2100.0, "median": 1050.0 })
    );
    assert_eq!(
        answer["statuses"],
        json!({ "200": 1, "404": 1, "failed": 1 })
    );
    assert_eq!(answer["encodings"], json!({ "br": 1, "none": 1 }));
    assert_eq!(answer["cacheControl"], json!(["no-cache"]));
    assert_eq!(answer["validators"], 1);
    assert_eq!(
        answer["views"][0],
        json!({ "view": 1, "calls": 1, "durationMs": { "median": 100.0, "p95": 100.0 }, "ttfbMs": { "median": 80.0, "p95": 80.0 } })
    );
    assert_eq!(answer["views"][1]["calls"], 2);
    // Newest first; a token in the query never shows.
    assert_eq!(answer["requests"][0]["url"], "https://a.test/api/users/3");
    let text = answer.to_string();
    assert!(!text.contains("secret"), "{text}");
    assert_eq!(
        answer["apiClient"],
        json!({ "method": "GET", "url": "https://a.test/api/users/3",
                "view": 2, "id": answer["requests"][0]["id"] })
    );
    assert_eq!(answer["apiClient"]["view"], answer["requests"][0]["view"]);
}

#[test]
fn a_summary_lists_the_newest_requests_only() {
    let found: Vec<_> = (0..40)
        .map(|at| {
            call(
                1,
                "GET",
                &format!("https://a.test/api/items/{at}"),
                f64::from(at),
                1.0,
            )
        })
        .collect();
    let groups = group(&found, &Options::default());
    let answer = summary(&groups[0]);
    let requests = answer["requests"].as_array().expect("requests");
    assert_eq!(requests.len(), MAX_REQUESTS);
    assert_eq!(requests[0]["url"], "https://a.test/api/items/39");
}

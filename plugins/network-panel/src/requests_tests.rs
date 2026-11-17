use serde_json::json;

use super::*;

fn entry(id: &str, url: &str, kind: &str, status: u64, size: f64, time: f64) -> Value {
    json!({ "id": id, "url": url, "method": "GET", "resourceType": kind, "status": status,
            "size": size, "time": time, "durationMs": 12.0, "fromCache": false })
}

/// Mobile (view 1) loads the small `srcset` image, desktop (view 2) the large one.
fn srcset_capture() -> Value {
    json!({ "$console": { "views": [
        { "view": 1, "network": [
            entry("1.1", "http://127.0.0.1:8080/network/index.html", "Document", 200, 900.0, 1.0),
            entry("1.2", "http://127.0.0.1:8080/network/img/hero-480.png", "Image", 200, 4000.0, 2.0),
            entry("1.3", "http://127.0.0.1:8080/api/user", "Fetch", 500, 30.0, 3.0),
        ] },
        { "view": 2, "network": [
            entry("2.1", "http://127.0.0.1:8080/network/index.html#top", "Document", 200, 900.0, 1.0),
            entry("2.2", "http://127.0.0.1:8080/network/img/hero-1600.png", "Image", 200, 40000.0, 2.0),
            entry("2.3", "http://127.0.0.1:8080/api/user", "Fetch", 200, 30.0, 3.0),
        ] },
        { "view": 3, "network": [] },
    ] } })
}

fn by_url<'a>(capture: &'a Capture, tail: &str) -> &'a Row {
    capture
        .rows
        .iter()
        .find(|row| row.url.ends_with(tail))
        .expect("a row")
}

#[test]
fn images_loaded_in_only_one_view_are_partial() {
    let capture = capture(&srcset_capture());
    assert!(capture.captured);
    assert_eq!(
        capture.views,
        BTreeSet::from([1, 2]),
        "a view without requests counts not"
    );
    assert_eq!(capture.rows.len(), 4, "the fragment is no other request");
    let small = by_url(&capture, "hero-480.png");
    assert!(small.partial && small.views.keys().eq([1].iter()));
    let large = by_url(&capture, "hero-1600.png");
    assert!(large.partial && large.views.keys().eq([2].iter()));
    let page = by_url(&capture, "index.html");
    assert!(!page.partial && !page.differs);
}

#[test]
fn a_status_seen_differently_differs() {
    let capture = capture(&srcset_capture());
    assert!(by_url(&capture, "/api/user").differs);
}

#[test]
fn sizes_differ_only_beyond_both_thresholds() {
    let seen = |size| Seen {
        id: String::new(),
        status: Some(200),
        size: Some(size),
        duration_ms: None,
        mime_type: None,
        from_cache: false,
        failed: None,
        time: 0.0,
        count: 1,
    };
    assert!(
        !differs([seen(10_000.0), seen(10_200.0)].iter()),
        "under 256 bytes"
    );
    assert!(
        !differs([seen(100_000.0), seen(100_400.0)].iter()),
        "under 5 %"
    );
    assert!(differs([seen(10_000.0), seen(20_000.0)].iter()));
}

#[test]
fn repeated_requests_count_in_their_view() {
    let args = json!({ "$console": { "views": [{ "view": 1, "network": [
        entry("a", "http://a.test/poll", "XHR", 200, 1.0, 1.0),
        entry("b", "http://a.test/poll", "XHR", 200, 1.0, 2.0),
    ] }] } });
    let capture = capture(&args);
    assert_eq!(capture.rows[0].views[&1].count, 2);
    assert_eq!(capture.rows[0].views[&1].id, "b", "the newest");
}

#[test]
fn the_filter_narrows_by_text_type_status_and_differences() {
    let capture = capture(&srcset_capture());
    let shown = |filter: &Filter| -> Vec<String> {
        page(&capture, filter, 100, 1 << 20)
            .0
            .iter()
            .filter_map(|row| {
                row["url"]
                    .as_str()
                    .map(|url| url.rsplit('/').next().unwrap_or(url).to_owned())
            })
            .collect()
    };
    let images = Filter {
        types: BTreeSet::from(["Image".to_owned()]),
        ..Filter::default()
    };
    assert_eq!(shown(&images), ["hero-480.png", "hero-1600.png"]);
    let errors = Filter {
        status: Some("error".to_owned()),
        ..Filter::default()
    };
    assert_eq!(shown(&errors), ["user"]);
    let differences = Filter {
        differences: true,
        ..Filter::default()
    };
    assert_eq!(shown(&differences).len(), 3);
    let search = Filter {
        search: Some("index".to_owned()),
        ..Filter::default()
    };
    assert_eq!(shown(&search), ["index.html"]);
}

#[test]
fn a_page_keeps_to_its_budget_and_says_how_many_passed() {
    let capture = capture(&srcset_capture());
    let (rows, total) = page(&capture, &Filter::default(), 100, 700);
    assert_eq!(total, 4);
    assert!(rows.len() < 4 && !rows.is_empty());
    let (rows, _) = page(&capture, &Filter::default(), 1, 1 << 20);
    assert_eq!(rows.len(), 1);
}

#[test]
fn without_a_capture_there_are_no_rows() {
    let capture = capture(&json!({}));
    assert!(!capture.captured && capture.rows.is_empty());
}

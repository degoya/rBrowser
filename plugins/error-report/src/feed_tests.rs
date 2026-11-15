use serde_json::json;

use super::*;

#[test]
fn no_capture_without_the_key() {
    let feed = read(&json!({ "target": "all" })).expect("reads");
    assert!(!feed.captured);
    assert!(feed.views.is_empty());
}

#[test]
fn the_lists_of_each_view_are_read_and_issues_stay_absent_when_not_sent() {
    let feed = read(&json!({ FEED_KEY: { "views": [
        { "view": 1,
          "console": [{ "level": "error", "source": "security", "text": "x", "time": 3.0 }],
          "errors": [{ "text": "Uncaught Error: a", "url": "https://a.test/a.js", "line": 2,
                       "column": 5, "time": 4.0 }],
          "network": [{ "id": "1", "url": "https://a.test/x.png", "method": "GET",
                        "resourceType": "Image", "status": 404, "fromCache": false,
                        "time": 5.0 }],
          "issues": [{ "code": "DeprecationIssue", "kind": "XHRSynchronous", "time": 9.0 }] },
        { "view": 2 }
    ] } }))
    .expect("reads");
    assert!(feed.captured);
    assert_eq!(feed.views.len(), 2);
    let one = &feed.views[0];
    assert_eq!(one.console[0].source, "security");
    assert_eq!(one.errors[0].column, Some(5));
    assert_eq!(one.network[0].status, Some(404));
    assert_eq!(one.network[0].resource_type.as_deref(), Some("Image"));
    let issues = one.issues.as_ref().expect("sent");
    assert_eq!(issues[0].kind.as_deref(), Some("XHRSynchronous"));
    assert_eq!(feed.views[1].issues, None);
    assert!((feed.newest() - 9.0).abs() < f64::EPSILON);
}

#[test]
fn a_capture_of_another_shape_is_an_error() {
    assert!(read(&json!({ FEED_KEY: { "views": "nope" } })).is_err());
    assert!(read(&json!({ FEED_KEY: { "views": [{ "view": -1 }] } })).is_err());
}

use serde_json::json;

use super::*;

#[test]
fn cdp_cookies_read_with_their_flags_and_a_bad_entry_is_left_out() {
    let answer = json!({ "cookies": [
        { "name": "sid", "value": "hunter2", "domain": "127.0.0.1", "path": "/",
          "expires": -1, "size": 10, "httpOnly": true, "secure": false, "session": true,
          "sameSite": "Lax", "priority": "Medium", "sourceScheme": "NonSecure" },
        { "name": "chip", "value": "x", "domain": ".example.com", "path": "/",
          "expires": 1_900_000_000.5, "httpOnly": false, "secure": true, "session": false,
          "partitionKey": { "topLevelSite": "https://example.com", "hasCrossSiteAncestor": false } },
        { "name": 42 },
    ]});
    let cookies = cookies_of(&answer);
    assert_eq!(cookies.len(), 2);
    assert!(cookies[0].http_only && cookies[0].session);
    assert_eq!(cookies[0].same_site.as_deref(), Some("Lax"));
    assert!(cookies[1].partition_key.is_some());
    assert!((cookies[1].expires - 1_900_000_000.5).abs() < f64::EPSILON);
    assert!(cookies_of(&json!({})).is_empty());
}

#[test]
fn a_cookie_value_never_shows() {
    let cookies = cookies_of(&json!({ "cookies": [
        { "name": "sid", "value": "hunter2", "domain": "localhost", "path": "/" }
    ]}));
    let shown = format!("{cookies:?}");
    assert!(shown.contains("sid"), "{shown}");
    assert!(!shown.contains("hunter2"), "{shown}");
}

#[test]
fn the_arguments_take_a_session_or_a_view_and_one_or_many_targets() {
    let args: CopyArgs = serde_json::from_value(json!({
        "source": "admin", "target": "all", "domains": ["example.com"]
    }))
    .expect("parses");
    assert_eq!(args.source, Some(Source::Session("admin".to_owned())));
    assert_eq!(args.target, Some(Targets::One(Target::All)));
    let args: CopyArgs =
        serde_json::from_value(json!({ "source": 3, "target": [4, 5] })).expect("parses");
    assert_eq!(args.source, Some(Source::View(ViewId(3))));
    let targets = args.target.expect("a target");
    assert!(targets.covers(ViewId(5)) && !targets.covers(ViewId(3)));
    assert!(Targets::One(Target::View(ViewId(2))).covers(ViewId(2)));
    assert!(serde_json::from_value::<CopyArgs>(json!({ "values": true })).is_err());
}

#[test]
fn the_report_counts_without_values() {
    let report = CopyReport {
        copied: 2,
        views: vec![ViewReport {
            view: ViewId(4),
            engine: EngineKind::Firefox,
            session: "default".to_owned(),
            copied: 2,
            refused: 1,
            skipped: BTreeMap::from([(SkipReason::OtherDomain, 3), (SkipReason::Partitioned, 1)]),
            reloaded: true,
            error: None,
        }],
        message: None,
    };
    assert_eq!(report.skipped(), 5);
    assert_eq!(report.failed(), 0);
    assert_eq!(
        serde_json::to_value(&report).expect("serializes")["views"][0]["skipped"],
        json!({ "otherDomain": 3, "partitioned": 1 })
    );
}

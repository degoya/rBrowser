use serde_json::json;

use super::*;
use crate::fake::FakeHost;

const PAGE: &str = "https://a.test/";

fn bare(host: &mut FakeHost) {
    host.state(&json!({ "url": PAGE, "secure": true, "themeColor": true,
        "appleTouchIcon": "https://a.test/apple.png", "sw": { "supported": true } }));
}

#[test]
fn each_page_is_checked_once_and_its_findings_reach_every_view() {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", PAGE)
        .view(2, "Mobile", PAGE)
        .view(3, "Other", "https://b.test/");
    bare(&mut host);
    let views = host.inner.views.clone();
    let run = run(&mut host, &Settings::default(), &views, &[]);
    assert_eq!(run.pages.len(), 2);
    assert_eq!(run.pages[0].views, vec![1, 2]);
    assert_eq!(run.pages[1].views, vec![3]);
    let evaluated: Vec<ViewId> = host.inner.evaluated.iter().map(|(view, _)| *view).collect();
    assert_eq!(
        evaluated,
        vec![1, 3],
        "the second desktop-page view is not asked"
    );
    let rules: Vec<(String, Option<ViewId>)> = run
        .findings_per_view()
        .into_iter()
        .map(|f| (f.rule, f.view))
        .collect();
    assert!(rules.contains(&("manifest.missing".to_owned(), Some(1))));
    assert!(rules.contains(&("manifest.missing".to_owned(), Some(2))));
    assert!(rules.contains(&("manifest.missing".to_owned(), Some(3))));
    assert_eq!(host.inner.progress.last(), Some(&(3, 3)));
}

#[test]
fn firefox_and_webkit_views_are_skipped_and_named() {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", PAGE).view(2, "Firefox", PAGE);
    host.inner.views[1].engine = "firefox".to_owned();
    bare(&mut host);
    let views = host.inner.views.clone();
    let run = run(&mut host, &Settings::default(), &views, &[]);
    assert_eq!(run.pages[0].views, vec![1]);
    assert_eq!(
        run.skipped,
        vec![Skipped {
            view: 2,
            reason: codes::NOT_AVAILABLE.to_owned()
        }]
    );
}

#[test]
fn targets_are_all_one_view_or_refused() {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", PAGE).view(2, "Mobile", PAGE);
    assert_eq!(targets(&mut host, &Value::Null).expect("all").len(), 2);
    assert_eq!(targets(&mut host, &json!("all")).expect("all").len(), 2);
    assert_eq!(targets(&mut host, &json!(2)).expect("one")[0].id, 2);
    let missing = targets(&mut host, &json!(9)).expect_err("missing");
    assert_eq!(missing.code(), Some(codes::NOT_FOUND));
    let bad = targets(&mut host, &json!("desktop")).expect_err("bad");
    assert_eq!(bad.code(), Some(codes::INVALID));
}

#[test]
fn a_check_run_covers_the_view_it_names_with_the_rules_asked_for() {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", PAGE).view(2, "Mobile", PAGE);
    bare(&mut host);
    let ctx = CheckContext {
        view: Some(2),
        url: None,
        rules: vec!["sw.missing".to_owned()],
        args: "{}".to_owned(),
    };
    let findings = check(&mut host, &ctx).expect("findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "sw.missing");
    assert_eq!(findings[0].view, Some(2));
    assert!(host.inner.cdp_sent.is_empty(), "install.error not asked");
    let gone = CheckContext {
        view: Some(7),
        ..ctx.clone()
    };
    assert!(check(&mut host, &gone).is_err());
    // A WebKit view named alone: skipped for its engine, not "nothing found" (RB-080-98).
    host.inner.views[1].engine = "webkit".to_owned();
    let failure = check(&mut host, &ctx).expect_err("skipped");
    assert_eq!(failure.unserved_engine_in(2), Some("webkit"));
}

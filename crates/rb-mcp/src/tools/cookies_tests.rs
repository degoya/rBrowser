use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{CookiesCopyParams, SourceArg};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, answer_json, server, view};

fn params(
    source: Option<SourceArg>,
    target: Option<TargetArg>,
    domains: Option<Vec<String>>,
) -> Parameters<CookiesCopyParams> {
    Parameters(CookiesCopyParams {
        source,
        target,
        domains,
    })
}

#[tokio::test]
async fn cookies_copy_runs_cookies_copy_with_the_views_of_its_target() {
    let fake = Arc::new(FakeBackend::new(vec![
        view(1, "desktop", "http://127.0.0.1:18766/"),
        view(2, "mobile", "http://127.0.0.1:18766/"),
        view(3, "mobile", "http://127.0.0.1:18766/"),
    ]));
    let report = json!({ "copied": 1, "views": [
        { "view": 2, "engine": "firefox", "session": "default", "copied": 1, "refused": 0,
          "skipped": { "otherDomain": 2 }, "reloaded": true }
    ]});
    fake.answer("cookies.copy", Ok(report.clone()));
    let server = server(&fake);
    for call in [
        params(None, None, None),
        params(
            Some(SourceArg::Session("admin".to_owned())),
            Some(TargetArg::all()),
            Some(vec!["127.0.0.1".to_owned()]),
        ),
        params(
            Some(SourceArg::View(1)),
            Some(TargetArg::Name("mobile".to_owned())),
            None,
        ),
    ] {
        let (error, answer) = answer_json(&server.cookies_copy(call).await.expect("answer"));
        assert!(!error);
        assert_eq!(answer, report);
    }
    assert_eq!(
        fake.calls(),
        vec![
            ("cookies.copy".to_owned(), json!({})),
            (
                "cookies.copy".to_owned(),
                json!({ "source": "admin", "domains": ["127.0.0.1"] })
            ),
            (
                "cookies.copy".to_owned(),
                json!({ "source": 1, "target": [2, 3] })
            ),
        ]
    );
}

#[tokio::test]
async fn an_unknown_target_is_named_and_the_command_never_runs() {
    let fake = Arc::new(FakeBackend::default());
    let (error, _) = answer_json(
        &server(&fake)
            .cookies_copy(params(None, Some(TargetArg::Id(42)), None))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert!(fake.calls().is_empty());
}

#[test]
fn the_arguments_are_checked() {
    let parsed: CookiesCopyParams =
        serde_json::from_value(json!({ "source": 3, "target": "all" })).expect("parses");
    assert_eq!(parsed.source, Some(SourceArg::View(3)));
    assert!(serde_json::from_value::<CookiesCopyParams>(json!({ "value": "x" })).is_err());
}

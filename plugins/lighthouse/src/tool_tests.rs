use rb_plugin_api::{Failure, FailureKind, codes};
use serde_json::Value;

use super::*;
use crate::fake::FakeHost;

#[test]
fn reads_the_run_arguments() {
    let request = run_request(
        r#"{"breakpoints":["mobile"],"categories":["seo"],"url":"https://example.com/","target":4}"#,
    )
    .expect("reads");
    assert_eq!(request.breakpoints, Some(vec!["mobile".to_owned()]));
    assert_eq!(request.categories, Some(vec!["seo".to_owned()]));
    assert_eq!(request.url.as_deref(), Some("https://example.com/"));
    assert_eq!(request.view, Some(4));
    assert_eq!(run_request("").expect("empty").view, None);
    assert_eq!(run_request(r#"{"target":"all"}"#).expect("all").view, None);
}

#[test]
fn refuses_arguments_out_of_bounds() {
    for args in [
        r#"{"categories":["pwa"]}"#,
        r#"{"url":"file:///C:/x.html"}"#,
        r#"{"breakpoints":[""]}"#,
        r#"{"target":"active"}"#,
        r#"{"target":-1}"#,
        r#"{"other":1}"#,
        "[",
    ] {
        let failure = run_request(args).expect_err(args);
        assert_eq!(
            failure.code(),
            Some("plugin.lighthouse.errors.bad_args"),
            "{args}"
        );
    }
}

#[test]
fn status_names_an_unavailable_sidecar_and_the_last_run() {
    let mut host = FakeHost::with_views(&[(1, "mobile", 375)]).done();
    let answer: Value = serde_json::from_str(&status(&mut host).expect("status")).expect("JSON");
    assert_eq!(answer["sidecar"]["available"], true);
    assert_eq!(answer["latest"], Value::Null);
    run(&mut host, "{}", false).expect("runs");
    host.sidecar_failure = Some(Failure::coded(
        FailureKind::NotSupported,
        codes::NOT_AVAILABLE,
    ));
    let answer: Value = serde_json::from_str(&status(&mut host).expect("status")).expect("JSON");
    assert_eq!(answer["sidecar"]["available"], false);
    assert_eq!(
        answer["sidecar"]["error"],
        "plugin.lighthouse.errors.no_sidecar"
    );
    assert_eq!(answer["latest"]["breakpoints"][0]["breakpoint"], "mobile");
}

#[test]
fn cancel_asks_the_sidecar() {
    let mut host = FakeHost::with_views(&[]);
    assert_eq!(cancel(&mut host).expect("cancels"), r#"{"cancelled":0}"#);
    assert_eq!(host.methods(), ["lighthouse.cancel"]);
}

use rb_plugin_api::{CheckContext, Failure, FailureKind, codes};

use super::*;
use crate::fake::FakeHost;

/// Three views: two narrow ones (the nameless menu button shows) and a wide one.
fn three_views() -> FakeHost {
    FakeHost::with_views(&[
        (1, "mobile", "narrow"),
        (2, "tablet", "narrow"),
        (3, "desktop", "wide"),
    ])
}

fn every_rule() -> Vec<String> {
    rules::all().iter().map(|rule| rule.id.clone()).collect()
}

fn context(view: Option<ViewId>, args: &str) -> CheckContext {
    CheckContext {
        view,
        url: None,
        rules: every_rule(),
        args: args.to_owned(),
    }
}

/// The options JSON of the last script sent.
fn options(host: &FakeHost) -> serde_json::Value {
    let (_, script) = host.evaluated.last().expect("a script was sent");
    let start = script.rfind("rbA11yAxe.start(").expect("the call") + "rbA11yAxe.start(".len();
    let end = script.rfind(");\n})()").expect("the end");
    serde_json::from_str(&script[start..end]).expect("options JSON")
}

#[test]
fn describe_offers_every_axe_rule() {
    assert_eq!(A11yAxe.describe().len(), 105);
}

#[test]
fn the_button_finding_belongs_to_the_narrow_views_only() {
    let mut host = three_views();
    let mut views_with_button = Vec::new();
    for view in [1, 2, 3] {
        let found = A11yAxe
            .run(&mut host, &context(Some(view), "{}"))
            .expect("the check runs");
        assert!(found.iter().all(|finding| finding.view == Some(view)));
        if found.iter().any(|finding| finding.rule == "button-name") {
            views_with_button.push(view);
        }
        assert!(found.iter().any(|finding| finding.rule == "image-alt"));
    }
    assert_eq!(views_with_button, [1, 2]);
}

#[test]
fn without_a_view_every_view_runs_with_progress() {
    let mut host = three_views();
    let found = A11yAxe
        .run(&mut host, &context(None, ""))
        .expect("the check runs");
    assert_eq!(found.len(), 5);
    assert_eq!(
        host.evaluated
            .iter()
            .map(|(view, _)| *view)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(host.progress, [(1, 3), (2, 3), (3, 3)]);
}

#[test]
fn the_script_asks_for_the_default_level_and_the_settings() {
    let mut host = three_views()
        .set("exclude", r##"["#ads"]"##)
        .set("reportIncomplete", "true");
    A11yAxe
        .run(&mut host, &context(Some(3), "{}"))
        .expect("the check runs");
    let sent = options(&host);
    assert_eq!(sent["rules"].as_array().map(Vec::len), Some(64));
    assert_eq!(sent["exclude"], serde_json::json!(["#ads"]));
    assert_eq!(sent["incomplete"], true);
}

#[test]
fn the_tags_of_the_arguments_win_over_the_setting() {
    let mut host = three_views();
    A11yAxe
        .run(
            &mut host,
            &context(
                Some(1),
                r#"{"a11y": {"tags": ["wcag2a"]}, "seo": {"x": 1}}"#,
            ),
        )
        .expect("the check runs");
    assert_eq!(options(&host)["rules"].as_array().map(Vec::len), Some(57));
}

#[test]
fn malformed_arguments_are_refused() {
    let mut host = three_views();
    for args in [
        "not json",
        r#"{"a11y": {"tags": ["a b"]}}"#,
        r#"{"a11y": {"x": 1}}"#,
    ] {
        let failure = A11yAxe
            .run(&mut host, &context(Some(1), args))
            .expect_err(args);
        assert_eq!(
            failure.code(),
            Some("plugin.a11y-axe.errors.bad_args"),
            "{args}"
        );
    }
}

#[test]
fn no_rule_left_on_sends_nothing() {
    let mut host = three_views();
    let ctx = CheckContext {
        rules: vec!["region".to_owned()],
        ..context(Some(1), "{}")
    };
    assert!(A11yAxe.run(&mut host, &ctx).expect("runs").is_empty());
    assert!(host.evaluated.is_empty());
}

#[test]
fn a_failing_view_fails_its_own_run() {
    let mut host = three_views();
    host.answers.insert(
        2,
        Err(Failure::coded(
            FailureKind::NotSupported,
            codes::NOT_AVAILABLE,
        )),
    );
    let failure = A11yAxe
        .run(&mut host, &context(Some(2), "{}"))
        .expect_err("view 2 fails");
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
}

#[test]
fn over_all_views_a_failing_view_is_logged_and_the_rest_reported() {
    let mut host = three_views();
    host.answers.insert(
        2,
        Err(Failure::coded(
            FailureKind::NotSupported,
            codes::NOT_AVAILABLE,
        )),
    );
    let found = A11yAxe
        .run(&mut host, &context(None, "{}"))
        .expect("the others run");
    assert!(found.iter().all(|finding| finding.view != Some(2)));
    assert_eq!(host.logs.len(), 1);
    for view in [1, 3] {
        host.answers.insert(
            view,
            Err(Failure::coded(
                FailureKind::NotSupported,
                codes::NOT_AVAILABLE,
            )),
        );
    }
    assert!(A11yAxe.run(&mut host, &context(None, "{}")).is_err());
}

#[test]
fn unknown_tools_and_commands_are_refused() {
    let mut host = three_views();
    let failure = A11yAxe
        .call_tool(&mut host, "nope", "{}")
        .expect_err("unknown");
    assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    let failure = A11yAxe
        .run_command(&mut host, "@page", "{}")
        .expect_err("unknown");
    assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
}

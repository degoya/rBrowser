use rb_plugin_api::check::{Check, valid_rule_id};
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{CheckContext, FailureKind, Severity};
use serde_json::{Value, json};

use super::*;

fn all_rules() -> Vec<String> {
    VisualRegression
        .describe()
        .into_iter()
        .map(|rule| rule.id)
        .collect()
}

fn comparison(view: u32, breakpoint: &str, status: &str) -> Value {
    json!({
        "view": view,
        "label": format!("View {view}"),
        "breakpoint": breakpoint,
        "slug": "index",
        "url": "http://localhost:5173/",
        "status": status,
        "differentPixels": 0,
        "totalPixels": 1000,
        "ratio": 0.0,
    })
}

fn context(view: Option<u32>, comparisons: Vec<Value>, fail_on_meta: bool) -> CheckContext {
    CheckContext {
        view,
        url: None,
        rules: all_rules(),
        args: json!({
            "visual": { "comparisons": comparisons, "failOnMetaMismatch": fail_on_meta }
        })
        .to_string(),
    }
}

fn run(ctx: &CheckContext) -> Vec<Finding> {
    VisualRegression
        .run(&mut MemoryHost::new(), ctx)
        .expect("the check runs")
}

#[test]
fn every_rule_has_a_valid_id_and_a_title_in_the_plugins_keys() {
    let rules = VisualRegression.describe();
    assert_eq!(rules.len(), 5);
    for rule in rules {
        assert!(valid_rule_id(&rule.id), "{}", rule.id);
        assert!(rule.title.starts_with("plugin.visual-regression.rules."));
    }
}

#[test]
fn unchanged_views_have_no_findings() {
    let ctx = context(
        None,
        vec![
            comparison(1, "mobile", "ok"),
            comparison(2, "desktop", "ok"),
        ],
        false,
    );
    assert!(run(&ctx).is_empty());
}

#[test]
fn a_differing_view_is_an_error_naming_its_share() {
    let mut differs = comparison(2, "desktop", "differs");
    differs["differentPixels"] = json!(160);
    differs["ratio"] = json!(0.0667);
    differs["regions"] = json!([{ "x": 10, "y": 10, "width": 20, "height": 8, "pixels": 160 }]);
    differs["diffPath"] = json!("/p/.rbrowser/out/screenshots/visual/run/index/desktop-diff.png");
    let found = run(&context(
        None,
        vec![comparison(1, "mobile", "ok"), differs],
        false,
    ));
    assert_eq!(found.len(), 1);
    let finding = &found[0];
    assert_eq!(finding.rule, DIFFERS);
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(finding.view, Some(2));
    assert_eq!(finding.message, "plugin.visual-regression.findings.differs");
    assert!(
        finding
            .params
            .contains(&("percent".to_owned(), "6.67".to_owned()))
    );
    assert!(
        finding
            .params
            .contains(&("areas".to_owned(), "1".to_owned()))
    );
    assert_eq!(finding.path.as_deref(), Some("visual/index/desktop"));
    let data: Value = serde_json::from_str(finding.data.as_deref().expect("data")).expect("json");
    assert_eq!(data["regions"][0]["width"], json!(20));
    assert!(
        data["diffPath"]
            .as_str()
            .is_some_and(|path| path.ends_with("desktop-diff.png"))
    );
}

#[test]
fn a_run_for_one_view_reports_only_that_view() {
    let ctx = context(
        Some(1),
        vec![
            comparison(1, "mobile", "differs"),
            comparison(2, "desktop", "differs"),
        ],
        false,
    );
    let found = run(&ctx);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].view, Some(1));
}

#[test]
fn size_mismatch_new_and_failed_have_their_own_rules() {
    let mut size = comparison(1, "mobile", "sizeMismatch");
    size["baselineSize"] = json!({ "width": 393, "height": 852 });
    size["actualSize"] = json!({ "width": 393, "height": 900 });
    let mut failed = comparison(3, "tablet", "failed");
    failed["error"] = json!("view 3 is not attached yet");
    let found = run(&context(
        None,
        vec![size, comparison(2, "desktop", "new"), failed],
        false,
    ));
    let rules: Vec<(&str, Severity)> = found
        .iter()
        .map(|finding| (finding.rule.as_str(), finding.severity))
        .collect();
    assert_eq!(
        rules,
        [
            (SIZE_MISMATCH, Severity::Error),
            (NO_BASELINE, Severity::Info),
            (FAILED, Severity::Warning),
        ]
    );
    assert!(
        found[0]
            .params
            .contains(&("actual".to_owned(), "393x900".to_owned()))
    );
}

#[test]
fn another_engine_version_is_an_unreliable_warning_or_an_error_when_asked() {
    let mut other = comparison(1, "mobile", "ok");
    other["metaMismatches"] = json!([
        { "field": "engineVersion", "baseline": "155.0", "current": "156.0" }
    ]);
    let found = run(&context(None, vec![other.clone()], false));
    assert_eq!(found.len(), 1);
    assert_eq!(
        (found[0].rule.as_str(), found[0].severity),
        (UNRELIABLE, Severity::Warning)
    );
    assert!(found[0].params.contains(&(
        "fields".to_owned(),
        "engineVersion (155.0 / 156.0)".to_owned()
    )));
    let strict = run(&context(None, vec![other], true));
    assert_eq!(strict[0].severity, Severity::Error);
}

#[test]
fn rules_switched_off_report_nothing() {
    let mut ctx = context(None, vec![comparison(1, "mobile", "differs")], false);
    ctx.rules = vec![NO_BASELINE.to_owned()];
    assert!(run(&ctx).is_empty());
}

#[test]
fn a_general_check_run_without_comparisons_is_empty() {
    for args in ["", "{}"] {
        let ctx = CheckContext {
            args: args.to_owned(),
            rules: all_rules(),
            ..CheckContext::default()
        };
        assert!(run(&ctx).is_empty(), "{args:?}");
    }
}

#[test]
fn unreadable_arguments_are_bad_data_in_the_plugins_key() {
    let ctx = CheckContext {
        args: "{ not json".to_owned(),
        rules: all_rules(),
        ..CheckContext::default()
    };
    let failure = VisualRegression
        .run(&mut MemoryHost::new(), &ctx)
        .expect_err("refused");
    assert_eq!(failure.kind, FailureKind::BadData);
    assert_eq!(
        failure.code(),
        Some("plugin.visual-regression.errors.bad_args")
    );
}

#[test]
fn every_message_key_is_in_the_locales() {
    let locales: Value =
        serde_json::from_str(include_str!("../locales/en.json")).expect("locales/en.json");
    let texts = &locales["plugin"]["visual-regression"];
    for rule in VisualRegression.describe() {
        let name = rule.title.rsplit('.').next().expect("a name");
        assert!(texts["rules"][name].is_string(), "rules.{name}");
        assert!(texts["findings"][name].is_string(), "findings.{name}");
    }
    assert!(texts["errors"]["bad_args"].is_string());
}

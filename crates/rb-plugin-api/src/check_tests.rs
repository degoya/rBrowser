use super::{Check, MAX_FINDINGS, MAX_RULES, valid_rule_id};
use crate::test_support::MemoryHost;
use crate::{Capability, CheckContext, Failure, Finding, PluginHost, RuleInfo, Severity};

/// Finds a page without a `<title>`: evaluates in the view and turns the answer into findings.
struct TitleCheck;

impl Check for TitleCheck {
    fn describe(&self) -> Vec<RuleInfo> {
        vec![RuleInfo {
            id: "title.missing".to_owned(),
            title: "plugin.title-check.rules.title_missing".to_owned(),
            severity: Severity::Error,
            shared_id: Some("document-title".to_owned()),
            help_url: None,
        }]
    }

    fn run(
        &mut self,
        host: &mut dyn PluginHost,
        ctx: &CheckContext,
    ) -> Result<Vec<Finding>, Failure> {
        if !ctx.rules.iter().any(|rule| rule == "title.missing") {
            return Ok(Vec::new());
        }
        let view = ctx.view.unwrap_or(1);
        host.report_progress(0, 1);
        let title = host.evaluate(view, "document.title")?;
        host.report_progress(1, 1);
        if title != "\"\"" {
            return Ok(Vec::new());
        }
        Ok(vec![Finding {
            rule: "title.missing".to_owned(),
            severity: Severity::Error,
            message: "plugin.title-check.findings.missing".to_owned(),
            view: Some(view),
            selector: Some("head".to_owned()),
            path: Some("html > head".to_owned()),
            ..Finding::default()
        }])
    }
}

fn context(view: u32, rules: &[&str]) -> CheckContext {
    CheckContext {
        view: Some(view),
        url: Some("https://site.test/".to_owned()),
        rules: rules.iter().map(|rule| (*rule).to_owned()).collect(),
        args: "{}".to_owned(),
    }
}

#[test]
fn a_check_answers_findings_from_what_it_evaluated() {
    let mut host = MemoryHost::new().grant(Capability::Evaluate);
    host.evaluate_answer = "\"\"".to_owned();
    let findings = TitleCheck
        .run(&mut host, &context(4, &["title.missing"]))
        .expect("ran");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].view, Some(4));
    assert_eq!(findings[0].path.as_deref(), Some("html > head"));
    assert_eq!(host.evaluated, vec![(4, "document.title".to_owned())]);
    assert_eq!(host.progress, vec![(0, 1), (1, 1)]);
    assert!(findings.len() <= MAX_FINDINGS);
}

#[test]
fn a_check_runs_only_the_rules_it_is_given() {
    let mut host = MemoryHost::new().grant(Capability::Evaluate);
    host.evaluate_answer = "\"\"".to_owned();
    let findings = TitleCheck.run(&mut host, &context(4, &[])).expect("ran");
    assert!(findings.is_empty());
    assert!(host.evaluated.is_empty());
}

#[test]
fn a_check_describes_its_rules() {
    let rules = TitleCheck.describe();
    assert_eq!(rules[0].id, "title.missing");
    assert_eq!(rules[0].shared_id.as_deref(), Some("document-title"));
    assert!(rules.len() <= MAX_RULES);
}

#[test]
fn a_rule_id_is_a_short_settings_key() {
    for id in ["title.missing", "img-alt", "a_b", "x1"] {
        assert!(valid_rule_id(id), "{id}");
    }
    let long = "a".repeat(65);
    for id in [
        "",
        ".dot",
        "-dash",
        "with space",
        "a/b",
        "\u{e4}",
        long.as_str(),
    ] {
        assert!(!valid_rule_id(id), "{id}");
    }
}

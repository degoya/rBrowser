use rb_plugin_types::{
    CheckTarget, FINDINGS_VERSION, FindingMessage, FindingsFile, FindingsSummary, MergedFinding,
    Severity,
};
use serde_json::json;

use super::{
    CHECKS_RUN_DESCRIPTION, CHECKS_RUN_TOOL, ChecksRunAnswer, ChecksRunArgs, MAX_ANSWER_FINDINGS,
};

fn finding(index: usize) -> MergedFinding {
    MergedFinding {
        id: format!("{index:016x}"),
        rule_id: "seo.images".to_owned(),
        plugins: vec!["seo".to_owned()],
        severity: Severity::Warning,
        message: FindingMessage {
            key: "plugin.seo.findings.alt".to_owned(),
            params: Default::default(),
        },
        views: vec![1],
        breakpoints: vec!["mobile".to_owned()],
        url: None,
        element: None,
        help_url: None,
        data: None,
        shared_id: None,
    }
}

#[test]
fn the_tool_is_named_for_the_core() {
    assert_eq!(CHECKS_RUN_TOOL, "checks_run");
    assert!(CHECKS_RUN_DESCRIPTION.contains("\"all\""));
}

#[test]
fn the_arguments_default_to_all_and_are_checked() {
    let args = ChecksRunArgs::parse(&json!({})).expect("empty is all");
    assert_eq!(args.target, CheckTarget::All);
    assert_eq!(args.plugins, None);
    let args = ChecksRunArgs::parse(&json!({
        "target": 2, "plugins": ["seo", "a11y-axe"], "url": "https://site.test/"
    }))
    .expect("valid");
    assert_eq!(args.target, CheckTarget::View(2));
    assert_eq!(args.plugins.as_deref().map(<[String]>::len), Some(2));
    for bad in [
        json!({ "target": "some" }),
        json!({ "url": "file:///etc/passwd" }),
        json!({ "plugins": ["Not An Id"] }),
        json!({ "extra": true }),
    ] {
        assert!(ChecksRunArgs::parse(&bad).is_err(), "{bad}");
    }
}

#[test]
fn the_answer_carries_findings_and_summary_and_says_when_cut() {
    let findings: Vec<MergedFinding> = (0..MAX_ANSWER_FINDINGS + 3).map(finding).collect();
    let file = FindingsFile {
        version: FINDINGS_VERSION,
        run_id: "run-0000000000001".to_owned(),
        started_at: 1,
        target: CheckTarget::All,
        url: None,
        plugins: vec!["seo".to_owned()],
        views: Vec::new(),
        summary: FindingsSummary::of(&findings),
        findings,
        failures: Vec::new(),
        skipped: Vec::new(),
    };
    let answer = ChecksRunAnswer::from_file(&file);
    assert_eq!(answer.findings.len(), MAX_ANSWER_FINDINGS);
    assert!(answer.truncated);
    assert_eq!(answer.summary.total, MAX_ANSWER_FINDINGS + 3);
    let value = json!(answer);
    assert_eq!(value["runId"], "run-0000000000001");
    assert_eq!(value["summary"]["byPlugin"]["seo"], MAX_ANSWER_FINDINGS + 3);
}

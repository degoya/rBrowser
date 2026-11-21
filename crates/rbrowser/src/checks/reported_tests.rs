use std::collections::BTreeMap;

use rb_plugin_types::{
    CheckTarget, CheckedView, FINDINGS_VERSION, FindingMessage, FindingsFile, FindingsSummary,
    MergedFinding, Severity,
};

use super::shown_run;

fn run(id: &str, plugin: &str, views: &[u32], rule: &str) -> FindingsFile {
    let findings = vec![MergedFinding {
        id: format!("{plugin}-{rule}"),
        rule_id: format!("{plugin}.{rule}"),
        plugins: vec![plugin.to_owned()],
        severity: Severity::Error,
        message: FindingMessage {
            key: format!("plugin.{plugin}.findings.{rule}"),
            params: BTreeMap::new(),
        },
        views: views.to_vec(),
        breakpoints: Vec::new(),
        url: None,
        element: None,
        help_url: None,
        data: None,
        shared_id: None,
    }];
    FindingsFile {
        version: FINDINGS_VERSION,
        run_id: id.to_owned(),
        started_at: 1,
        target: CheckTarget::All,
        url: None,
        plugins: vec![plugin.to_owned()],
        views: views
            .iter()
            .map(|view| CheckedView {
                id: *view,
                breakpoint: None,
            })
            .collect(),
        summary: FindingsSummary::of(&findings),
        findings,
        failures: Vec::new(),
        skipped: Vec::new(),
    }
}

/// Every message key of `file`, with its key as the text.
fn texts(file: &FindingsFile) -> BTreeMap<String, String> {
    file.findings
        .iter()
        .map(|finding| (finding.message.key.clone(), finding.message.key.clone()))
        .collect()
}

#[test]
fn the_texts_of_the_last_run_and_the_plugins_run_are_both_kept_when_merged() {
    let last = run("run-1", "seo", &[1, 2], "title");
    let last_texts = BTreeMap::from([(
        "plugin.seo.findings.title".to_owned(),
        "No title".to_owned(),
    )]);
    let next = run("run-2", "console", &[1, 2], "error");
    let (shown, messages, merged) = shown_run(Some((last, last_texts)), &next, "console", texts);
    assert!(merged);
    assert_eq!(shown.findings.len(), 2);
    assert_eq!(
        messages
            .get("plugin.seo.findings.title")
            .map(String::as_str),
        Some("No title")
    );
    assert!(messages.contains_key("plugin.console.findings.error"));
}

#[test]
fn without_a_last_run_or_over_other_views_the_plugins_run_is_shown_alone() {
    let next = run("run-2", "console", &[1, 2], "error");
    let (shown, messages, merged) = shown_run(None, &next, "console", texts);
    assert!(!merged);
    assert_eq!(shown, next);
    assert_eq!(messages.len(), 1);

    let other = run("run-1", "seo", &[3], "title");
    let (shown, messages, merged) =
        shown_run(Some((other, BTreeMap::new())), &next, "console", texts);
    assert!(!merged);
    assert_eq!(shown, next);
    assert!(!messages.contains_key("plugin.seo.findings.title"));
}

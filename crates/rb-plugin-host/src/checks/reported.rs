//! Findings a plugin reports on its own (`findings.report`, Charge 6): the same rules as a check
//! run's answer, as one run of that plugin. The rule ids are qualified with the plugin's id, the
//! person's `checks.rules` and `checks.ignore` hold, the reports merge across views, and the run
//! goes where every run goes (written, shown in the findings panel). A check plugin answers its
//! findings from `check.run`; `findings.report` is for the findings of a feature plugin's
//! command or tool.

use rb_plugin_types::{
    CheckedView, FINDINGS_VERSION, Finding, FindingsFile, FindingsSummary, ViewId,
};

use super::{
    merge::{Report, merge},
    policy::{ChecksPolicy, ChecksSettings},
    runner::RunRequest,
};

/// The run of `findings`, reported by plugin `plugin`, over the views of `request`.
#[must_use]
pub fn reported_run(
    settings: &ChecksSettings,
    plugin: &str,
    findings: Vec<Finding>,
    request: RunRequest,
) -> FindingsFile {
    let policy = ChecksPolicy::new(settings);
    let view_of =
        |id: Option<ViewId>| id.and_then(|id| request.views.iter().find(|view| view.id == id));
    let reports = findings
        .into_iter()
        .filter_map(|mut finding| {
            let rule_id = format!("{plugin}.{}", finding.rule);
            if !policy.enabled(&rule_id) {
                return None;
            }
            let severity = policy.severity(&rule_id, finding.severity)?;
            let view = view_of(finding.view);
            if finding.url.is_none() {
                finding.url = request
                    .url
                    .clone()
                    .or_else(|| view.and_then(|view| view.url.clone()));
            }
            if policy.ignored(
                &rule_id,
                finding.url.as_deref(),
                finding.selector.as_deref(),
            ) {
                return None;
            }
            Some(Report {
                plugin: plugin.to_owned(),
                rule_id,
                shared_id: None,
                help_url: None,
                severity,
                view: finding.view,
                breakpoint: view.and_then(|view| view.breakpoint.clone()),
                finding,
            })
        })
        .collect();
    let findings = merge(reports);
    FindingsFile {
        version: FINDINGS_VERSION,
        run_id: request.run_id,
        started_at: request.started_at,
        target: request.target,
        url: request.url,
        plugins: vec![plugin.to_owned()],
        views: request
            .views
            .iter()
            .map(|view| CheckedView {
                id: view.id,
                breakpoint: view.breakpoint.clone(),
            })
            .collect(),
        summary: FindingsSummary::of(&findings),
        findings,
        failures: Vec::new(),
        skipped: Vec::new(),
    }
}

#[cfg(test)]
#[path = "reported_tests.rs"]
mod tests;

use std::collections::BTreeMap;

use rb_ipc::{CheckJob, CheckJobState, ChecksProgress, ViewId as IpcViewId};
use rb_plugin_types::{
    CheckFailure, CheckSkip, CheckTarget, CheckedView, FINDINGS_VERSION, FindingMessage,
    FindingsFile, FindingsSummary, MergedFinding, SKIP_ENGINE, Severity,
};

use super::{plugin_progress, plugin_run, plugins_run, replace_rows};

fn finding(id: &str, plugins: &[&str], views: &[u32]) -> MergedFinding {
    MergedFinding {
        id: id.to_owned(),
        rule_id: format!("{}.{id}", plugins[0]),
        plugins: plugins.iter().map(|plugin| (*plugin).to_owned()).collect(),
        severity: Severity::Warning,
        message: FindingMessage {
            key: format!("plugin.{}.findings.{id}", plugins[0]),
            params: BTreeMap::new(),
        },
        views: views.to_vec(),
        breakpoints: Vec::new(),
        url: None,
        element: None,
        help_url: None,
        data: None,
        shared_id: None,
    }
}

fn run(id: &str, plugins: &[&str], views: &[u32], findings: Vec<MergedFinding>) -> FindingsFile {
    FindingsFile {
        version: FINDINGS_VERSION,
        run_id: id.to_owned(),
        started_at: 1,
        target: CheckTarget::All,
        url: None,
        plugins: plugins.iter().map(|plugin| (*plugin).to_owned()).collect(),
        views: views
            .iter()
            .map(|view| CheckedView {
                id: *view,
                breakpoint: Some(format!("bp{view}")),
            })
            .collect(),
        summary: FindingsSummary::of(&findings),
        findings,
        failures: Vec::new(),
        skipped: Vec::new(),
    }
}

fn failure(plugin: &str, view: Option<u32>) -> CheckFailure {
    CheckFailure {
        plugin: plugin.to_owned(),
        view,
        code: Some(format!("plugin.{plugin}.errors.broken")),
        params: BTreeMap::from([("reason".to_owned(), "it broke".to_owned())]),
    }
}

/// A full run of `seo` and `a11y` over views 1 and 2: `seo` with two findings and a failure in
/// view 2, `a11y` with one finding and a skip in view 2.
fn full() -> FindingsFile {
    let mut file = run(
        "run-1",
        &["seo", "a11y"],
        &[1, 2],
        vec![
            finding("title", &["seo"], &[1]),
            finding("meta", &["seo"], &[1, 2]),
            finding("alt", &["a11y"], &[1]),
        ],
    );
    file.failures = vec![failure("seo", Some(2)), failure("a11y", None)];
    file.skipped = vec![
        CheckSkip {
            plugin: "seo".to_owned(),
            view: Some(1),
            reason: SKIP_ENGINE.to_owned(),
            engine: Some("webkit".to_owned()),
        },
        CheckSkip {
            plugin: "a11y".to_owned(),
            view: Some(2),
            reason: SKIP_ENGINE.to_owned(),
            engine: Some("webkit".to_owned()),
        },
    ];
    file
}

#[test]
fn a_plugin_run_over_the_same_views_replaces_only_that_plugins_part() {
    let next = run(
        "run-2",
        &["seo"],
        &[2, 1],
        vec![finding("canonical", &["seo"], &[2])],
    );
    let merged = plugin_run(&full(), &next, "seo").expect("the same views merge");
    let ids: Vec<&str> = merged.findings.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(ids, vec!["alt", "canonical"]);
    assert_eq!(merged.plugins, vec!["seo".to_owned(), "a11y".to_owned()]);
    assert_eq!(merged.failures, vec![failure("a11y", None)]);
    assert_eq!(merged.skipped.len(), 1);
    assert_eq!(merged.skipped[0].plugin, "a11y");
    assert_eq!(merged.summary, FindingsSummary::of(&merged.findings));
    assert_eq!(merged.summary.total, 2);
    assert_eq!(
        merged.summary.by_plugin,
        BTreeMap::from([("a11y".to_owned(), 1), ("seo".to_owned(), 1)])
    );
    // The merged run is the newest state: the new run's identity.
    assert_eq!(merged.run_id, "run-2");
}

#[test]
fn a_plugin_new_to_the_last_run_is_added_to_its_plugins() {
    let next = run(
        "run-2",
        &["console"],
        &[1, 2],
        vec![finding("error", &["console"], &[1])],
    );
    let merged = plugin_run(&full(), &next, "console").expect("the same views merge");
    assert_eq!(
        merged.plugins,
        vec!["seo".to_owned(), "a11y".to_owned(), "console".to_owned()]
    );
    assert_eq!(merged.findings.len(), 4);
    assert_eq!(merged.failures.len(), 2);
    assert_eq!(merged.summary.total, 4);
}

#[test]
fn a_shared_finding_loses_only_the_plugin_that_ran_again() {
    let mut previous = full();
    previous
        .findings
        .push(finding("img-alt", &["seo", "a11y"], &[1]));
    let next = run("run-2", &["seo"], &[1, 2], Vec::new());
    let merged = plugin_run(&previous, &next, "seo").expect("the same views merge");
    let shared = merged
        .findings
        .iter()
        .find(|f| f.id == "img-alt")
        .expect("a11y still reports it");
    assert_eq!(shared.plugins, vec!["a11y".to_owned()]);
    assert_eq!(merged.summary.by_plugin.get("seo"), None);
}

#[test]
fn a_plugin_run_over_other_views_does_not_merge() {
    let next = run("run-2", &["seo"], &[1, 3], Vec::new());
    assert!(plugin_run(&full(), &next, "seo").is_none());
    let fewer = run("run-2", &["seo"], &[1], Vec::new());
    assert!(plugin_run(&full(), &fewer, "seo").is_none());
}

fn job(plugin: &str, view: Option<u32>, state: CheckJobState) -> CheckJob {
    CheckJob {
        plugin: plugin.to_owned(),
        view: view.map(IpcViewId),
        breakpoint: view.map(|view| format!("bp{view}")),
        state,
        findings: None,
        reason: None,
        reason_params: BTreeMap::new(),
        elapsed_ms: Some(10),
    }
}

fn finished() -> ChecksProgress {
    ChecksProgress {
        run_id: "run-1".to_owned(),
        elapsed_ms: 4200,
        estimate_ms: Some(4000),
        jobs: vec![
            job("a11y", Some(1), CheckJobState::Done),
            job("seo", Some(1), CheckJobState::Done),
            job("seo", Some(2), CheckJobState::Failed),
            job("a11y", Some(2), CheckJobState::Done),
        ],
        finished: true,
        cancelled: false,
    }
}

#[test]
fn the_progress_of_a_merged_run_replaces_that_plugins_rows_in_place() {
    let mut next = run(
        "run-2",
        &["seo"],
        &[1, 2],
        vec![
            finding("canonical", &["seo"], &[1]),
            finding("lang", &["seo"], &[1]),
        ],
    );
    next.failures = vec![failure("seo", Some(2))];
    let progress = plugin_progress(Some(&finished()), &next, "seo");
    let rows: Vec<(&str, Option<u32>, CheckJobState, Option<u32>)> = progress
        .jobs
        .iter()
        .map(|job| {
            (
                job.plugin.as_str(),
                job.view.map(|view| view.0),
                job.state,
                job.findings,
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            ("a11y", Some(1), CheckJobState::Done, None),
            ("seo", Some(1), CheckJobState::Done, Some(2)),
            ("seo", Some(2), CheckJobState::Failed, None),
            ("a11y", Some(2), CheckJobState::Done, None),
        ]
    );
    let failed = &progress.jobs[2];
    assert_eq!(failed.reason.as_deref(), Some("plugin.seo.errors.broken"));
    assert_eq!(
        failed.reason_params.get("reason").map(String::as_str),
        Some("it broke")
    );
    assert_eq!(failed.breakpoint.as_deref(), Some("bp2"));
    assert_eq!(progress.run_id, "run-1");
    assert_eq!(progress.elapsed_ms, 4200);
    assert!(progress.finished);
}

#[test]
fn a_plugin_new_to_the_progress_gets_its_rows_at_the_end() {
    let next = run("run-2", &["console"], &[1, 2], Vec::new());
    let progress = plugin_progress(Some(&finished()), &next, "console");
    assert_eq!(progress.jobs.len(), 6);
    let last = &progress.jobs[5];
    assert_eq!(last.plugin, "console");
    assert_eq!(last.state, CheckJobState::Done);
    assert_eq!(last.findings, Some(0));
}

#[test]
fn a_replaced_run_gets_the_progress_of_its_plugin_alone() {
    let mut next = run(
        "run-2",
        &["seo"],
        &[1, 3],
        vec![finding("canonical", &["seo"], &[1, 3])],
    );
    next.skipped = vec![CheckSkip {
        plugin: "seo".to_owned(),
        view: Some(3),
        reason: SKIP_ENGINE.to_owned(),
        engine: Some("webkit".to_owned()),
    }];
    let progress = plugin_progress(None, &next, "seo");
    assert_eq!(progress.run_id, "run-2");
    assert!(progress.finished);
    assert_eq!(progress.estimate_ms, None);
    assert_eq!(progress.jobs.len(), 2);
    assert_eq!(progress.jobs[0].state, CheckJobState::Done);
    assert_eq!(progress.jobs[0].findings, Some(1));
    assert_eq!(progress.jobs[1].state, CheckJobState::Skipped);
    assert_eq!(
        progress.jobs[1].reason.as_deref(),
        Some("checks.skip.engine")
    );
    assert_eq!(
        progress.jobs[1]
            .reason_params
            .get("engine")
            .map(String::as_str),
        Some("WebKit")
    );
}

#[test]
fn a_run_without_views_is_one_row_for_the_plugin() {
    let next = run(
        "run-2",
        &["seo"],
        &[],
        vec![finding("canonical", &["seo"], &[])],
    );
    let progress = plugin_progress(None, &next, "seo");
    assert_eq!(progress.jobs.len(), 1);
    assert_eq!(progress.jobs[0].view, None);
    assert_eq!(progress.jobs[0].findings, Some(1));
}

#[test]
fn a_plugin_wide_failure_is_one_failed_row() {
    let mut next = run("run-2", &["seo"], &[1, 2], Vec::new());
    next.failures = vec![failure("seo", None)];
    let progress = plugin_progress(None, &next, "seo");
    assert_eq!(progress.jobs.len(), 1);
    assert_eq!(progress.jobs[0].view, None);
    assert_eq!(progress.jobs[0].state, CheckJobState::Failed);
}

/// RB-094-17: a run of chosen checks replaces each one's part, the others stay.
#[test]
fn a_run_of_two_plugins_replaces_both_parts_and_keeps_the_rest() {
    let mut previous = full();
    previous.plugins.push("console".to_owned());
    previous
        .findings
        .push(finding("error", &["console"], &[1, 2]));
    let next = run(
        "run-2",
        &["seo", "a11y"],
        &[1, 2],
        vec![finding("canonical", &["seo"], &[2])],
    );
    let replaced = ["seo".to_owned(), "a11y".to_owned()];
    let merged = plugins_run(&previous, &next, &replaced).expect("the same views merge");
    let ids: Vec<&str> = merged.findings.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(ids, vec!["error", "canonical"]);
    assert!(
        merged.failures.is_empty(),
        "both plugins' failures replaced"
    );
    assert!(merged.skipped.is_empty(), "both plugins' skips replaced");
    assert_eq!(
        merged.plugins,
        vec!["seo".to_owned(), "a11y".to_owned(), "console".to_owned()]
    );
    assert_eq!(merged.summary.total, 2);
    assert!(
        plugins_run(
            &previous,
            &run("run-2", &["seo"], &[1], Vec::new()),
            &replaced
        )
        .is_none()
    );
}

#[test]
fn the_rows_of_several_plugins_are_replaced_where_each_stood() {
    let mut previous = finished().jobs;
    previous.push(job("console", Some(1), CheckJobState::Done));
    let rows = vec![
        job("seo", Some(1), CheckJobState::Done),
        job("console", Some(1), CheckJobState::Failed),
        job("seo", Some(2), CheckJobState::Done),
        job("links", Some(1), CheckJobState::Done),
    ];
    let replaced = ["seo".to_owned(), "console".to_owned(), "links".to_owned()];
    let jobs = replace_rows(&previous, rows, &replaced);
    let shown: Vec<(&str, Option<u32>, CheckJobState)> = jobs
        .iter()
        .map(|job| (job.plugin.as_str(), job.view.map(|view| view.0), job.state))
        .collect();
    assert_eq!(
        shown,
        vec![
            ("a11y", Some(1), CheckJobState::Done),
            ("seo", Some(1), CheckJobState::Done),
            ("seo", Some(2), CheckJobState::Done),
            ("a11y", Some(2), CheckJobState::Done),
            ("console", Some(1), CheckJobState::Failed),
            ("links", Some(1), CheckJobState::Done),
        ]
    );
}

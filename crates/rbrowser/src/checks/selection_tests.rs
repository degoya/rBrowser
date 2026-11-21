use std::collections::BTreeMap;

use rb_ipc::{CheckJob, CheckJobState, ChecksProgress, CoreToShell, ViewId as IpcViewId};
use rb_plugin_host::{RunRequest, RunView};
use rb_plugin_types::{CheckTarget, FindingsFile};
use serde_json::json;

use super::{Base, shown};

/// A run of `plugins` over `views`, one finding per plugin in every view.
fn file(id: &str, plugins: &[&str], views: &[u32], url: Option<&str>) -> FindingsFile {
    let findings: Vec<_> = plugins
        .iter()
        .map(|plugin| {
            json!({
                "id": format!("{plugin}-1"), "ruleId": format!("{plugin}.rule"), "plugins": [plugin],
                "severity": "warning", "message": { "key": format!("plugin.{plugin}.findings.one") },
                "views": views, "breakpoints": []
            })
        })
        .collect();
    let mut file: FindingsFile = serde_json::from_value(json!({
        "version": 1, "runId": id, "startedAt": 1, "target": "all",
        "plugins": plugins,
        "views": views.iter().map(|view| json!({ "id": view })).collect::<Vec<_>>(),
        "findings": findings,
        "summary": { "total": 0, "error": 0, "warning": 0, "info": 0, "byPlugin": {} }
    }))
    .expect("a findings file");
    file.url = url.map(str::to_owned);
    file.summary = rb_plugin_types::FindingsSummary::of(&file.findings);
    file
}

fn request(plugins: Option<&[&str]>, views: &[u32], url: Option<&str>) -> RunRequest {
    RunRequest {
        run_id: "run-2".to_owned(),
        started_at: 2,
        target: CheckTarget::All,
        views: views
            .iter()
            .map(|view| RunView {
                id: *view,
                breakpoint: None,
                url: Some("https://site.test/".to_owned()),
            })
            .collect(),
        url: url.map(str::to_owned),
        plugins: plugins.map(|ids| ids.iter().map(|id| (*id).to_owned()).collect()),
        args: "{}".to_owned(),
    }
}

fn last() -> Option<(FindingsFile, BTreeMap<String, String>)> {
    Some((
        file("run-1", &["seo", "a11y", "console"], &[1, 2], None),
        BTreeMap::from([("plugin.seo.findings.one".to_owned(), "One".to_owned())]),
    ))
}

fn ids(plugins: &[&str]) -> Vec<String> {
    plugins.iter().map(|id| (*id).to_owned()).collect()
}

fn row(plugin: &str, view: u32, state: CheckJobState) -> CheckJob {
    CheckJob {
        plugin: plugin.to_owned(),
        view: Some(IpcViewId(view)),
        breakpoint: None,
        state,
        findings: None,
        reason: None,
        reason_params: BTreeMap::new(),
        elapsed_ms: None,
    }
}

fn progress(finished: bool) -> ChecksProgress {
    ChecksProgress {
        run_id: "run-1".to_owned(),
        elapsed_ms: 900,
        estimate_ms: None,
        jobs: vec![
            row("seo", 1, CheckJobState::Done),
            row("a11y", 1, CheckJobState::Done),
            row("console", 1, CheckJobState::Done),
        ],
        finished,
        cancelled: false,
    }
}

#[test]
fn a_run_of_every_check_replaces_the_last_run() {
    let asked = request(None, &[1, 2], None);
    assert!(Base::of(last(), None, &asked, &ids(&["seo"])).is_none());
}

#[test]
fn a_run_of_chosen_checks_over_the_same_views_merges_into_the_last_run() {
    let asked = request(Some(&["seo", "a11y"]), &[2, 1], None);
    let base = Base::of(
        last(),
        Some(&progress(true)),
        &asked,
        &ids(&["seo", "a11y"]),
    )
    .expect("the same views and page");
    let next = file("run-2", &["seo"], &[1, 2], None);
    let texts = |_: &FindingsFile| {
        BTreeMap::from([("plugin.seo.findings.one".to_owned(), "Uno".to_owned())])
    };
    let (merged, messages) = base.run(&next, texts);
    let found: Vec<&str> = merged.findings.iter().map(|f| f.id.as_str()).collect();
    // a11y ran again and is not through yet: its old findings are gone, console's stay.
    assert_eq!(found, vec!["console-1", "seo-1"]);
    assert_eq!(merged.run_id, "run-2");
    assert_eq!(merged.summary.total, 2);
    assert_eq!(
        messages.get("plugin.seo.findings.one").map(String::as_str),
        Some("Uno"),
        "the new run's texts over the old"
    );
}

#[test]
fn other_views_or_another_page_replace_the_last_run() {
    let chosen = ids(&["seo"]);
    let fewer = request(Some(&["seo"]), &[1], None);
    assert!(Base::of(last(), None, &fewer, &chosen).is_none());
    let other_page = request(Some(&["seo"]), &[1, 2], Some("https://other.test/"));
    assert!(Base::of(last(), None, &other_page, &chosen).is_none());
    let same = request(Some(&["seo"]), &[1, 2], None);
    assert!(
        Base::of(None, None, &same, &chosen).is_none(),
        "no last run"
    );
}

#[test]
fn the_rows_of_the_chosen_checks_replace_theirs_in_the_finished_progress() {
    let asked = request(Some(&["a11y"]), &[1, 2], None);
    let base = Base::of(last(), Some(&progress(true)), &asked, &ids(&["a11y"])).expect("merges");
    let jobs = base.jobs(vec![row("a11y", 1, CheckJobState::Failed)]);
    let states: Vec<(&str, CheckJobState)> = jobs
        .iter()
        .map(|job| (job.plugin.as_str(), job.state))
        .collect();
    assert_eq!(
        states,
        vec![
            ("seo", CheckJobState::Done),
            ("a11y", CheckJobState::Failed),
            ("console", CheckJobState::Done),
        ]
    );
    // A progress that never finished is no table to merge into: the rows alone.
    let base = Base::of(last(), Some(&progress(false)), &asked, &ids(&["a11y"])).expect("merges");
    assert_eq!(
        base.jobs(vec![row("a11y", 1, CheckJobState::Done)]).len(),
        1
    );
}

#[test]
fn only_a_finished_run_shown_is_a_base() {
    let message = |partial: bool| CoreToShell::Findings {
        run: serde_json::to_value(file("run-1", &["seo"], &[1], None)).expect("serializes"),
        messages: BTreeMap::new(),
        project_off: Default::default(),
        partial,
    };
    assert!(shown(Some(&message(false))).is_some());
    assert!(shown(Some(&message(true))).is_none(), "a run still going");
    assert!(shown(Some(&CoreToShell::Ack)).is_none());
    assert!(shown(None).is_none());
}

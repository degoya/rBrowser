use std::sync::{Arc, atomic::Ordering};
use std::time::Duration;

use rb_plugin_types::{CheckSkip, CheckTarget, Failure, SKIP_ENGINE, Severity, codes};
use serde_json::json;

use super::{CheckRunner, RunRequest, RunView};
use crate::checks::testing::{FakeSource, Gauge, finding, rule};
use crate::checks::{CheckSource, ChecksSettings, JobState, RunControl, SkipReason};
use crate::plugin::{LoadedPlugin, PluginOptions};
use crate::test_support::{KEY_ID, ManifestBuilder, signing_key};
use crate::testbed::Bed;
use crate::{PluginVerifier, VerifiedPackage};

fn views(breakpoints: &[&str]) -> Vec<RunView> {
    breakpoints
        .iter()
        .zip(1..)
        .map(|(breakpoint, id)| RunView {
            id,
            breakpoint: Some((*breakpoint).to_owned()),
            url: Some("https://site.test/".to_owned()),
        })
        .collect()
}

fn request(views: Vec<RunView>) -> RunRequest {
    RunRequest {
        run_id: "run-0000000000001".to_owned(),
        started_at: 1,
        target: CheckTarget::All,
        views,
        url: None,
        plugins: None,
        args: "{}".to_owned(),
    }
}

fn runner(settings: serde_json::Value) -> CheckRunner {
    CheckRunner::new(&ChecksSettings::from_value(Some(&settings)))
}

fn sources(list: Vec<FakeSource>) -> Vec<Arc<dyn CheckSource>> {
    list.into_iter()
        .map(|source| Arc::new(source) as Arc<dyn CheckSource>)
        .collect()
}

fn verified(bed: &Bed) -> VerifiedPackage {
    let key = signing_key(1);
    let builder = ManifestBuilder::default().kinds(&["check-plugin"]);
    let package = builder.package(&bed.build(), &key, true, &[]);
    let verifier = PluginVerifier::new(false);
    verifier
        .trust_key(KEY_ID.to_owned(), key.verifying_key())
        .expect("trust");
    verifier.verify_bytes(&package).expect("verifies")
}

#[tokio::test]
async fn a_check_plugins_two_findings_keep_their_ids_across_runs() {
    let plugin = LoadedPlugin::load(
        &verified(&Bed::check_finding_two()),
        PluginOptions::default(),
    )
    .expect("loads");
    let sources: Vec<Arc<dyn CheckSource>> = vec![Arc::new(plugin)];
    let runner = runner(json!({}));
    let first = runner.run(&sources, request(views(&["mobile"]))).await;
    let second = runner.run(&sources, request(views(&["mobile"]))).await;
    assert_eq!(first.findings.len(), 2, "{:?}", first.failures);
    assert_eq!(first.findings[0].rule_id, "sample.basic");
    assert_eq!(first.findings[0].plugins, vec!["sample"]);
    assert_eq!(first.findings[0].views, vec![1], "the run's view fills in");
    assert_eq!(first.findings[0].shared_id.as_deref(), Some("shared-basic"));
    let ids = |file: &rb_plugin_types::FindingsFile| -> Vec<String> {
        file.findings
            .iter()
            .map(|finding| finding.id.clone())
            .collect()
    };
    assert_eq!(ids(&first), ids(&second));
    assert_ne!(ids(&first)[0], ids(&first)[1]);
    assert_eq!(first.summary.warning, 2);
}

#[tokio::test]
async fn a_finding_in_two_of_three_views_names_those_breakpoints() {
    let mut source = FakeSource::new(
        "seo",
        vec![rule("images", None)],
        vec![finding("images", Severity::Warning, "img")],
    );
    source.only_views = vec![1, 3];
    let source = Arc::new(source);
    let file = runner(json!({}))
        .run(
            &[source.clone() as Arc<dyn CheckSource>],
            request(views(&["mobile", "tablet", "desktop"])),
        )
        .await;
    assert_eq!(file.findings.len(), 1);
    assert_eq!(file.findings[0].views, vec![1, 3]);
    assert_eq!(file.findings[0].breakpoints, vec!["mobile", "desktop"]);
    assert_eq!(file.views.len(), 3, "what only-in is measured against");
    assert_eq!(file.views[1].breakpoint.as_deref(), Some("tablet"));
    let contexts = source.contexts();
    assert_eq!(contexts.len(), 3, "every view is checked");
    assert!(
        contexts
            .iter()
            .all(|ctx| ctx.rules == vec!["images".to_owned()])
    );
}

#[tokio::test]
async fn a_rule_switched_off_is_not_run_and_error_raises_the_rest() {
    let source = Arc::new(FakeSource::new(
        "seo",
        vec![rule("images", None), rule("title", None)],
        vec![
            finding("images", Severity::Warning, "img"),
            finding("title", Severity::Warning, "title"),
        ],
    ));
    let file = runner(json!({ "rules": { "seo.images": "off", "seo.title": "error" } }))
        .run(
            &[source.clone() as Arc<dyn CheckSource>],
            request(views(&["mobile"])),
        )
        .await;
    assert_eq!(file.findings.len(), 1);
    assert_eq!(file.findings[0].rule_id, "seo.title");
    assert_eq!(file.findings[0].severity, Severity::Error);
    assert_eq!(source.contexts()[0].rules, vec!["title".to_owned()]);
}

#[tokio::test]
async fn findings_of_undescribed_rules_and_ignored_ones_are_dropped() {
    let mut stray = finding("unlisted", Severity::Error, "div");
    stray.rule = "unlisted".to_owned();
    let mut source = FakeSource::new(
        "seo",
        vec![rule("images", None), rule("ads", None)],
        vec![
            finding("images", Severity::Warning, "img"),
            finding("ads", Severity::Warning, "img.ad-top"),
        ],
    );
    source.findings.push(stray);
    let file = runner(json!({ "ignore": [{ "selector": "img.ad*" }] }))
        .run(&sources(vec![source]), request(views(&["mobile"])))
        .await;
    let rules: Vec<&str> = file
        .findings
        .iter()
        .map(|finding| finding.rule_id.as_str())
        .collect();
    assert_eq!(rules, vec!["seo.images"]);
}

#[tokio::test]
async fn a_failing_plugin_is_a_failure_of_the_run_not_its_end() {
    let mut broken = FakeSource::new("broken", vec![rule("x", None)], Vec::new());
    broken.fail = true;
    let fine = FakeSource::new(
        "seo",
        vec![rule("images", None)],
        vec![finding("images", Severity::Info, "img")],
    );
    let file = runner(json!({}))
        .run(&sources(vec![broken, fine]), request(views(&["mobile"])))
        .await;
    assert_eq!(file.findings.len(), 1);
    assert_eq!(file.failures.len(), 1);
    assert_eq!(file.failures[0].plugin, "broken");
    assert_eq!(file.failures[0].view, Some(1));
    assert_eq!(file.failures[0].code.as_deref(), Some(codes::TIMED_OUT));
    assert_eq!(file.plugins, vec!["broken", "seo"]);
}

#[tokio::test]
async fn only_the_plugins_asked_for_run_and_an_unknown_one_is_named() {
    let seo = Arc::new(FakeSource::new(
        "seo",
        vec![rule("images", None)],
        Vec::new(),
    ));
    let axe = Arc::new(FakeSource::new(
        "a11y-axe",
        vec![rule("image-alt", None)],
        Vec::new(),
    ));
    let mut asked = request(views(&["mobile"]));
    asked.plugins = Some(vec!["a11y-axe".to_owned(), "nope".to_owned()]);
    let file = runner(json!({}))
        .run(&[seo.clone() as Arc<dyn CheckSource>, axe.clone()], asked)
        .await;
    assert!(seo.contexts().is_empty());
    assert_eq!(axe.contexts().len(), 1);
    assert_eq!(file.plugins, vec!["a11y-axe"]);
    assert_eq!(file.failures[0].plugin, "nope");
    assert_eq!(file.failures[0].code.as_deref(), Some(codes::NOT_FOUND));
}

#[tokio::test]
async fn without_views_one_run_checks_the_url() {
    let source = Arc::new(FakeSource::new(
        "seo",
        vec![rule("images", None)],
        Vec::new(),
    ));
    let mut asked = request(Vec::new());
    asked.url = Some("https://site.test/page".to_owned());
    runner(json!({}))
        .run(&[source.clone() as Arc<dyn CheckSource>], asked)
        .await;
    let contexts = source.contexts();
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].view, None);
    assert_eq!(contexts[0].url.as_deref(), Some("https://site.test/page"));
}

#[tokio::test]
async fn no_more_runs_than_the_limit_at_once() {
    let gauge = Arc::new(Gauge::default());
    let list: Vec<FakeSource> = (0..4)
        .map(|index| {
            let mut source = FakeSource::new(
                &format!("plugin-{index}"),
                vec![rule("x", None)],
                Vec::new(),
            );
            source.delay = Duration::from_millis(20);
            source.gauge = Arc::clone(&gauge);
            source
        })
        .collect();
    runner(json!({ "parallel": 2 }))
        .run(&sources(list), request(views(&["a", "b", "c"])))
        .await;
    let peak = gauge.peak.load(Ordering::SeqCst);
    assert!(peak <= 2, "at most two at once, saw {peak}");
    assert_eq!(peak, 2, "the limit is used");
}

/// What the host answers a page service in a view of another engine (`plugins::services::page`).
fn unserved(service: &str, view: u32, engine: &str) -> Failure {
    Failure::engine_unserved(service, view, engine)
}

#[tokio::test]
async fn a_view_whose_engine_the_plugin_cannot_serve_is_skipped_not_failed() {
    let mut axe = FakeSource::new(
        "a11y-axe",
        vec![rule("image-alt", None)],
        vec![finding("image-alt", Severity::Error, "img")],
    );
    axe.fail_in = vec![(1, unserved("evaluate", 1, "webkit"))];
    let outcome = runner(json!({}))
        .run_with(
            &sources(vec![axe]),
            request(views(&["mobile", "md"])),
            &RunControl::default(),
        )
        .await;
    assert!(
        outcome.file.failures.is_empty(),
        "no failure: {:?}",
        outcome.file.failures
    );
    assert_eq!(
        outcome.jobs[0].state,
        JobState::Skipped {
            reason: SkipReason::Engine,
            engine: Some("webkit".to_owned()),
        }
    );
    assert_eq!(outcome.jobs[1].state, JobState::Done { findings: 1 });
    assert_eq!(outcome.file.findings.len(), 1);
    // The file says so too, so a reader of findings.json never takes mobile for clean.
    assert_eq!(
        outcome.file.skipped,
        vec![CheckSkip {
            plugin: "a11y-axe".to_owned(),
            view: Some(1),
            reason: SKIP_ENGINE.to_owned(),
            engine: Some("webkit".to_owned()),
        }]
    );
    let json = serde_json::to_value(&outcome.file).expect("json");
    assert_eq!(
        json["skipped"],
        json!([{ "plugin": "a11y-axe", "view": 1, "reason": "engine", "engine": "webkit" }])
    );
}

#[tokio::test]
async fn a_failure_keeps_the_parameters_its_sentence_needs() {
    let mut lighthouse = FakeSource::new("lighthouse", vec![rule("perf", None)], Vec::new());
    // Not available, but not for the view's engine: a failure, with its `{service}`.
    lighthouse.fail_in = vec![(1, crate::not_available("sidecar"))];
    let outcome = runner(json!({}))
        .run_with(
            &sources(vec![lighthouse]),
            request(views(&["mobile"])),
            &RunControl::default(),
        )
        .await;
    let failure = &outcome.file.failures[0];
    assert_eq!(failure.code.as_deref(), Some(codes::NOT_AVAILABLE));
    assert_eq!(
        failure.params.get("service").map(String::as_str),
        Some("sidecar")
    );
    let JobState::Failed { code, params } = &outcome.jobs[0].state else {
        panic!("a failure, not {:?}", outcome.jobs[0].state);
    };
    assert_eq!(code.as_deref(), Some(codes::NOT_AVAILABLE));
    assert_eq!(params.get("service").map(String::as_str), Some("sidecar"));
}

#[tokio::test]
async fn a_refusal_of_another_view_is_a_failure_of_this_one_not_its_skip() {
    let mut fonts = FakeSource::new("fonts", vec![rule("size", None)], Vec::new());
    // The job of Chromium view 1 reached for WebKit view 2: its findings are missing.
    fonts.fail_in = vec![(1, unserved("evaluate", 2, "webkit"))];
    let outcome = runner(json!({}))
        .run_with(
            &sources(vec![fonts]),
            request(views(&["md", "mobile"])),
            &RunControl::default(),
        )
        .await;
    assert!(
        matches!(outcome.jobs[0].state, JobState::Failed { .. }),
        "{:?}",
        outcome.jobs[0].state
    );
    assert_eq!(outcome.file.failures.len(), 1);
    assert_eq!(outcome.file.failures[0].view, Some(1));
}

#[tokio::test]
async fn a_job_without_a_view_is_never_skipped_for_an_engine() {
    let mut axe = FakeSource::new("a11y-axe", vec![rule("image-alt", None)], Vec::new());
    axe.fail_with = Some(unserved("evaluate", 1, "webkit"));
    let outcome = runner(json!({}))
        .run_with(
            &sources(vec![axe]),
            request(Vec::new()),
            &RunControl::default(),
        )
        .await;
    assert!(matches!(outcome.jobs[0].state, JobState::Failed { .. }));
    assert_eq!(outcome.file.failures.len(), 1);
}

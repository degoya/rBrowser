use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use async_trait::async_trait;
use rb_config::FindingsSettings;
use rb_engine::{EngineKind, Target, ViewId, ViewInfo};
use rb_plugin_host::{CheckSource, ChecksRunArgs, ChecksSettings, RunView, findings_path};
use rb_plugin_types::{
    CheckContext, CheckTarget, Failure, Finding, FindingsFile, RuleInfo, Severity,
};
use serde_json::json;

use super::*;

fn findings_settings(value: serde_json::Value) -> FindingsSettings {
    serde_json::from_value(value).expect("the test settings parse")
}

#[test]
fn the_project_keys_win_over_the_user_keys() {
    let project = findings_settings(json!({
        "parallel": 2,
        "rules": { "seo.images": "off" }
    }));
    let user = findings_settings(json!({
        "parallel": 8,
        "keepRuns": 3,
        "rules": { "seo.images": "error", "seo.title": "info" },
        "ignore": [{ "rule": "seo.*", "url": "https://site.test/*" }]
    }));
    let settings = settings(&project, &user);
    assert_eq!(settings.parallel(), 2);
    assert_eq!(settings.keep_runs(), 3);
    assert_eq!(settings.rules["seo.images"], json!("off"));
    assert_eq!(settings.rules["seo.title"], json!("info"));
    assert_eq!(settings.ignore.len(), 1);
    assert_eq!(settings.ignore[0].rule.as_deref(), Some("seo.*"));

    let defaults = settings_of_nothing();
    assert_eq!(defaults.parallel(), rb_plugin_host::DEFAULT_PARALLEL);
    assert_eq!(defaults.keep_runs(), rb_plugin_host::DEFAULT_KEEP_RUNS);
}

fn settings_of_nothing() -> ChecksSettings {
    settings(&FindingsSettings::default(), &FindingsSettings::default())
}

#[test]
fn a_check_target_is_an_engine_target() {
    assert_eq!(engine_target(CheckTarget::All), Target::All);
    assert_eq!(engine_target(CheckTarget::View(4)), Target::View(ViewId(4)));
}

fn info(id: u32, url: &str) -> ViewInfo {
    ViewInfo {
        id: ViewId(id),
        label: "Mobile".to_owned(),
        breakpoint: Some("mobile".to_owned()),
        cell_breakpoint: Some("mobile".to_owned()),
        engine: EngineKind::Chromium,
        width: 393,
        height: 852,
        dpr: 2.0,
        scale: 0.5,
        url: url.to_owned(),
        title: String::new(),
        loading: false,
        session: "default".to_owned(),
        cell: None,
    }
}

#[test]
fn a_view_passes_its_breakpoint_and_only_a_web_url() {
    assert_eq!(
        run_view(&info(2, "https://site.test/shop")),
        RunView {
            id: 2,
            breakpoint: Some("mobile".to_owned()),
            url: Some("https://site.test/shop".to_owned()),
        }
    );
    for url in ["", "about:blank", "data:text/html,x", "javascript:alert(1)"] {
        assert_eq!(run_view(&info(2, url)).url, None, "{url}");
    }
}

#[test]
fn the_request_carries_the_arguments_and_the_start() {
    let args = ChecksRunArgs {
        target: CheckTarget::View(2),
        plugins: Some(vec!["seo".to_owned()]),
        url: Some("https://site.test/".to_owned()),
    };
    let started = UNIX_EPOCH + Duration::from_millis(1_731_254_400_123);
    let request = request(args, vec![run_view(&info(2, ""))], started);
    assert_eq!(request.run_id, "run-1731254400123");
    assert!(rb_plugin_host::valid_run_id(&request.run_id));
    assert_eq!(request.started_at, 1_731_254_400);
    assert_eq!(request.target, CheckTarget::View(2));
    assert_eq!(request.plugins, Some(vec!["seo".to_owned()]));
    assert_eq!(request.url.as_deref(), Some("https://site.test/"));
    assert_eq!(request.args, "{}");
    assert_eq!(request.views.len(), 1);
}

/// A check plugin in memory: one rule, one finding per view.
struct Fake;

#[async_trait]
impl CheckSource for Fake {
    fn plugin_id(&self) -> &str {
        "seo"
    }

    async fn describe(&self) -> Result<Vec<RuleInfo>, Failure> {
        Ok(vec![RuleInfo {
            id: "images".to_owned(),
            title: "plugin.seo.rules.images".to_owned(),
            severity: Severity::Warning,
            shared_id: None,
            help_url: None,
        }])
    }

    async fn run(&self, _ctx: CheckContext) -> Result<Vec<Finding>, Failure> {
        Ok(vec![Finding {
            rule: "images".to_owned(),
            severity: Severity::Warning,
            message: "plugin.seo.findings.alt".to_owned(),
            selector: Some("img.hero".to_owned()),
            path: Some("html > body > img".to_owned()),
            ..Finding::default()
        }])
    }
}

fn fake_request() -> rb_plugin_host::RunRequest {
    let views = vec![
        run_view(&info(1, "https://site.test/")),
        run_view(&info(2, "https://site.test/")),
    ];
    request(
        ChecksRunArgs::default(),
        views,
        UNIX_EPOCH + Duration::from_secs(5),
    )
}

#[tokio::test]
async fn a_run_is_written_under_the_project() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let sources: Vec<Arc<dyn CheckSource>> = vec![Arc::new(Fake)];
    let (outcome, not_written) = execute(
        &sources,
        &settings_of_nothing(),
        fake_request(),
        Some(dir.path()),
        &RunControl::default(),
    )
    .await;
    let file = outcome.file;
    assert_eq!(not_written, None);
    assert_eq!(
        file.findings.len(),
        1,
        "one element in two views is one finding"
    );
    assert_eq!(file.findings[0].views, vec![1, 2]);
    let written = std::fs::read_to_string(findings_path(dir.path(), &file.run_id))
        .expect("findings.json is there");
    let back: FindingsFile = serde_json::from_str(&written).expect("it parses");
    assert_eq!(back, file);
}

#[tokio::test]
async fn without_a_project_nothing_is_written_and_no_plugin_is_an_empty_run() {
    let sources: Vec<Arc<dyn CheckSource>> = vec![Arc::new(Fake)];
    let none = RunControl::default();
    let (outcome, not_written) = execute(
        &sources,
        &settings_of_nothing(),
        fake_request(),
        None,
        &none,
    )
    .await;
    let file = outcome.file;
    assert_eq!(not_written, None);
    assert_eq!(file.summary.total, 1);

    let (empty, _) = execute(&[], &settings_of_nothing(), fake_request(), None, &none).await;
    let empty = empty.file;
    assert!(empty.findings.is_empty());
    assert!(empty.failures.is_empty());
    assert!(empty.plugins.is_empty());
}

fn run_with(findings: serde_json::Value) -> FindingsFile {
    serde_json::from_value(json!({
        "version": 1,
        "runId": "run-0000000000001",
        "startedAt": 1,
        "target": "all",
        "plugins": ["seo", "a11y-axe"],
        "views": [],
        "findings": findings,
        "summary": { "total": 0, "error": 0, "warning": 0, "info": 0, "byPlugin": {} }
    }))
    .expect("the test run parses")
}

fn finding(id: &str, plugins: &[&str], key: &str) -> serde_json::Value {
    json!({
        "id": id,
        "ruleId": "seo.images",
        "plugins": plugins,
        "severity": "warning",
        "message": { "key": key },
        "views": [1],
        "breakpoints": ["mobile"]
    })
}

#[test]
fn the_messages_come_from_the_plugins_that_reported_them() {
    let file = run_with(json!([
        finding("a", &["seo", "a11y-axe"], "plugin.seo.findings.alt"),
        finding("b", &["a11y-axe"], "plugin.a11y-axe.findings.label"),
        finding("c", &["a11y-axe"], "plugin.seo.findings.title"),
        finding("d", &["seo"], "plugin.seo.findings.missing"),
    ]));
    let texts = |plugin: &str| -> BTreeMap<String, String> {
        match plugin {
            "seo" => [
                ("plugin.seo.findings.alt", "{count} images without alt text"),
                ("plugin.seo.findings.title", "The title is short"),
            ]
            .into_iter()
            .map(|(key, text)| (key.to_owned(), text.to_owned()))
            .collect(),
            "a11y-axe" => [("plugin.a11y-axe.findings.label", "A field without a label")]
                .into_iter()
                .map(|(key, text)| (key.to_owned(), text.to_owned()))
                .collect(),
            _ => BTreeMap::new(),
        }
    };
    let messages = messages(&file, texts);
    assert_eq!(
        messages.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["plugin.a11y-axe.findings.label", "plugin.seo.findings.alt"],
        "a key only another plugin knows, and a missing key, are left out"
    );
    assert_eq!(
        messages["plugin.seo.findings.alt"],
        "{count} images without alt text"
    );
}

/// A plugin's own failure code is told in its words; the core's `plugins.error.*` are the
/// shell's catalogue (RB-080-89: the findings name why a plugin failed).
#[test]
fn a_failure_code_of_the_plugins_own_comes_with_its_text() {
    let mut file = run_with(json!([]));
    file.failures = serde_json::from_value(json!([
        { "plugin": "a11y-axe", "view": 1, "code": "plugin.a11y-axe.error.no_runner" },
        { "plugin": "seo", "code": "plugins.error.timed_out" },
        { "plugin": "seo" }
    ]))
    .expect("failures");
    let texts = |plugin: &str| -> BTreeMap<String, String> {
        match plugin {
            "a11y-axe" => [(
                "plugin.a11y-axe.error.no_runner".to_owned(),
                "The axe runner did not load".to_owned(),
            )]
            .into(),
            _ => [(
                "plugins.error.timed_out".to_owned(),
                "a plugin's copy".to_owned(),
            )]
            .into(),
        }
    };
    let messages = messages(&file, texts);
    assert_eq!(
        messages.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["plugin.a11y-axe.error.no_runner"]
    );
}

/// A checked-in config may hide findings: what it switched off over the user's settings is
/// named (batch-2 security review, H3).
#[test]
fn the_project_names_what_it_turned_off() {
    use rb_config::{FindingsSettings, IgnoreEntry, RuleSetting};
    let user = FindingsSettings {
        rules: [
            ("seo.title".to_owned(), RuleSetting::Error),
            ("a11y.contrast".to_owned(), RuleSetting::Off),
        ]
        .into(),
        ignore: vec![IgnoreEntry {
            rule: Some("seo.*".into()),
            ..IgnoreEntry::default()
        }],
        ..FindingsSettings::default()
    };
    let legacy = IgnoreEntry {
        url: Some("http://localhost:5173/legacy/*".into()),
        ..IgnoreEntry::default()
    };
    let project = FindingsSettings {
        rules: [
            ("seo.title".to_owned(), RuleSetting::Off),
            ("seo.images".to_owned(), RuleSetting::Off),
            ("a11y.contrast".to_owned(), RuleSetting::Off),
            ("seo.meta".to_owned(), RuleSetting::Warning),
        ]
        .into(),
        ignore: vec![legacy.clone()],
        ..FindingsSettings::default()
    };
    let off = project_off(&project, &user);
    // The user had a11y.contrast off already; seo.meta still runs.
    assert_eq!(off.rules, ["seo.images", "seo.title"]);
    assert_eq!(off.ignore, [legacy]);
    assert!(project_off(&FindingsSettings::default(), &user).is_empty());
}

// RB-091-06: `settings.get("checks.rules")` answers a plugin the levels of its own rules as the
// findings run applies them - the project's keys over the user's.
#[test]
fn a_plugin_reads_its_rule_levels_as_the_run_applies_them() {
    let project = findings_settings(json!({ "rules": { "outline.heading-skip": "off" } }));
    let user = findings_settings(json!({
        "rules": {
            "outline.heading-skip": "error",
            "outline.multiple-h1": "info",
            "seo.title": "error"
        }
    }));
    let text = rule_levels(&project, &user, "outline");
    let value: serde_json::Value = serde_json::from_str(&text).expect("JSON text");
    assert_eq!(
        value,
        json!({ "heading-skip": "off", "multiple-h1": "info" })
    );
    let none = rule_levels(
        &FindingsSettings::default(),
        &FindingsSettings::default(),
        "outline",
    );
    assert_eq!(none, "{}");
}

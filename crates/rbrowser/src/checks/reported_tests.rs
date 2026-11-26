use std::collections::BTreeMap;

use rb_plugin_types::{
    CheckTarget, CheckedView, FINDINGS_VERSION, FindingMessage, FindingsFile, FindingsSummary,
    MergedFinding, Severity,
};

use super::{Keep, mark, publish, remember, reported_since, shown_run, written_to};

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

/// RB-099-25: a person's check command names the findings of its own run - what the plugin
/// reported after the mark, its last report counting; nothing reported since is no count.
#[test]
fn the_count_of_a_run_is_the_plugins_last_report_after_the_mark() {
    let plugin = "count-check";
    let before = mark(plugin);
    assert_eq!(reported_since(plugin, before), None);
    remember(plugin, 4);
    assert_eq!(reported_since(plugin, before), Some(4));
    remember(plugin, 0);
    assert_eq!(
        reported_since(plugin, before),
        Some(0),
        "the last report counts"
    );
    let after = mark(plugin);
    assert_eq!(
        reported_since(plugin, after),
        None,
        "a later run reported nothing yet"
    );
    assert_eq!(reported_since("other-check", before), None);
}

#[tokio::test]
async fn a_published_report_is_counted() {
    use crate::cdp::fake::FakeBrowser;
    use crate::control::{Core, CoreSetup};
    use crate::host::Host;
    use crate::host::state::default_config;
    use crate::host::ui::recording::RecordingUi;
    let (_fake, router) = FakeBrowser::connect();
    let ui = std::sync::Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, tokio::runtime::Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(
        host,
        ui,
        tokio::runtime::Handle::current(),
        CoreSetup::default(),
    );
    let before = mark("published-check");
    publish(&core, "published-check", Vec::new());
    assert_eq!(reported_since("published-check", before), Some(0));
}

#[test]
fn a_shown_only_run_is_written_nowhere_and_a_run_under_the_project() {
    let root = std::path::PathBuf::from("/project");
    assert_eq!(written_to(Some(root.clone()), Keep::ShowOnly), None);
    assert_eq!(written_to(None, Keep::ShowOnly), None);
    assert_eq!(written_to(Some(root.clone()), Keep::Run), Some(root));
    assert_eq!(written_to(None, Keep::Run), None);
}

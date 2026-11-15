use std::sync::Arc;

use rb_config::OpenLinksIn;
use rb_engine::ViewHost;
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

fn setup() -> (Arc<RecordingUi>, Arc<Core>) {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(host, ui.clone(), Handle::current(), CoreSetup::default());
    (ui, core)
}

/// The keys of the notices the status bar got.
fn notices(ui: &RecordingUi) -> Vec<String> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::StatusEvent(script) => {
                let start = script.find(r#""messageKey":""#)? + r#""messageKey":""#.len();
                let end = start + script[start..].find('"')?;
                Some(script[start..end].to_owned())
            }
            _ => None,
        })
        .collect()
}

fn navigations(ui: &RecordingUi) -> Vec<String> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Navigate(_, url) => Some(url),
            _ => None,
        })
        .collect()
}

#[test]
fn the_target_is_the_one_asked_for_else_the_settings() {
    assert_eq!(target_of(None, OpenLinksIn::System), Where::System);
    assert_eq!(target_of(None, OpenLinksIn::ActiveView), Where::ActiveView);
    assert_eq!(
        target_of(None, OpenLinksIn::Ask),
        Where::System,
        "the shell asks; unanswered goes to the browser"
    );
    assert_eq!(
        target_of(Some(Where::ActiveView), OpenLinksIn::System),
        Where::ActiveView
    );
}

#[test]
fn the_command_is_a_help_chord_without_an_mcp_tool() {
    let defs = definitions().expect("valid");
    assert_eq!(defs[0].id.as_str(), "app.open_link");
    assert_eq!(
        defs[0].default_bindings,
        vec!["Ctrl+Shift+Space , H".parse::<Binding>().expect("binding")]
    );
    assert!(
        matches!(&defs[0].mcp, McpCoverage::Omitted(reason) if reason.starts_with("protected"))
    );
}

#[tokio::test]
async fn a_link_of_another_scheme_is_refused_with_a_sentence_and_opens_nothing() {
    let (ui, core) = setup();
    for (url, key) in [
        ("file:///C:/Windows/System32/calc.exe", "links.error.scheme"),
        ("javascript:alert(1)", "links.error.scheme"),
        ("https://user:pw@bank.example/", "links.error.invalid"),
    ] {
        let answer = open(&core, json!({ "url": url, "where": "activeView" })).expect("answers");
        assert_eq!(answer, json!({ "opened": false, "reason": key }), "{url}");
    }
    assert!(navigations(&ui).is_empty());
    assert_eq!(
        notices(&ui),
        [
            "links.error.scheme",
            "links.error.scheme",
            "links.error.invalid"
        ]
    );
}

#[tokio::test]
async fn bad_arguments_are_an_error_not_a_guess() {
    let (_ui, core) = setup();
    assert!(matches!(
        open(
            &core,
            json!({ "url": "https://site.test/", "where": "newWindow" })
        ),
        Err(CommandError::InvalidArgs(_))
    ));
    assert!(matches!(
        open(&core, json!({ "href": "https://site.test/" })),
        Err(CommandError::InvalidArgs(_))
    ));
}

#[tokio::test]
async fn a_link_opens_in_the_active_view_and_a_burst_is_cut() {
    let (ui, core) = setup();
    let active = core.host().active_view();
    let answer = open(
        &core,
        json!({ "url": "https://help.test/rule", "where": "activeView" }),
    )
    .expect("answers");
    if active.is_none() {
        assert_eq!(
            answer,
            json!({ "opened": false, "reason": "links.error.no_view" })
        );
        return;
    }
    assert_eq!(answer, json!({ "opened": true, "where": "activeView" }));
    assert_eq!(navigations(&ui), ["https://help.test/rule"]);
    assert_eq!(notices(&ui), ["links.notice.opened_view"]);
    for _ in 1..throttle::MAX_OPENS {
        open(
            &core,
            json!({ "url": "https://help.test/rule", "where": "activeView" }),
        )
        .expect("answers");
    }
    let cut = open(
        &core,
        json!({ "url": "https://help.test/rule", "where": "activeView" }),
    )
    .expect("answers");
    assert_eq!(
        cut,
        json!({ "opened": false, "reason": "links.error.busy" })
    );
}

/// A refused link uses none of the budget: only links that open count (review c20, 6).
#[tokio::test]
async fn links_refused_for_want_of_a_view_do_not_use_up_the_budget() {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    let core = Core::new(host, ui, Handle::current(), CoreSetup::default());
    assert_eq!(core.host().active_view(), None, "no project, no view");
    for _ in 0..throttle::MAX_OPENS + 1 {
        let answer = open(
            &core,
            json!({ "url": "https://help.test/rule", "where": "activeView" }),
        )
        .expect("answers");
        assert_eq!(
            answer,
            json!({ "opened": false, "reason": "links.error.no_view" })
        );
    }
    let opened = open(
        &core,
        json!({ "url": "https://help.test/rule", "where": "system" }),
    )
    .expect("answers");
    assert_eq!(opened, json!({ "opened": true, "where": "system" }));
}

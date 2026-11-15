use std::sync::Arc;

use rb_config::ProjectConfig;
use rb_engine::ViewId;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::nav::marker_url;
use crate::host::ui::recording::{RecordingUi, UiCall};

fn config(mobile_engine: &str) -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "startUrl": "http://127.0.0.1:18766/",
        "sessions": ["admin"],
        "breakpoints": [
            { "id": "desktop", "width": 1440, "height": 900 },
            { "id": "mobile", "width": 393, "height": 852, "session": "admin",
              "engine": mobile_engine }
        ],
        "layout": { "columns": 2, "rows": 1, "cells": [
            { "breakpoint": "desktop", "col": 1, "row": 1 },
            { "breakpoint": "mobile", "col": 2, "row": 1 }
        ] }
    }))
    .expect("the test config parses")
}

fn setup(mobile_engine: &str) -> (Arc<Core>, Arc<RecordingUi>) {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(config(mobile_engine), None)
        .expect("opens");
    for id in 1..=4 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    let core = Core::new(host, ui.clone(), Handle::current(), CoreSetup::default());
    core.shell_ready();
    (core, ui)
}

fn shell(ui: &RecordingUi) -> String {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Shell(script) => Some(script),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn without_a_firefox_or_webkit_view_the_answer_says_so_instead_of_failing() {
    let (core, ui) = setup("chromium");
    let report = copy_now(&core, CopyArgs::default(), true)
        .await
        .expect("an answer, not an error");
    assert!(report.views.is_empty());
    assert_eq!(report.copied, 0);
    assert!(report.message.is_some());
    assert!(shell(&ui).contains(NO_VIEWS_KEY));
}

#[tokio::test]
async fn a_firefox_view_whose_session_cannot_be_read_is_reported_and_notified() {
    let (core, ui) = setup("firefox");
    let report = copy_now(&core, CopyArgs::default(), true)
        .await
        .expect("a report");
    assert_eq!(report.views.len(), 1);
    let view = &report.views[0];
    assert_eq!(
        (view.engine, view.session.as_str()),
        (rb_config::EngineKind::Firefox, "admin")
    );
    assert!(view.error.is_some(), "{report:?}");
    assert!(shell(&ui).contains(FAILED_KEY));
}

#[tokio::test]
async fn bad_arguments_and_targets_are_refused_by_name() {
    let (core, _ui) = setup("firefox");
    assert!(matches!(
        run(&core, &json!({ "values": true })),
        Err(CommandError::InvalidArgs(_))
    ));
    let many: Vec<String> = (0..=MAX_DOMAINS).map(|n| format!("s{n}.example")).collect();
    assert!(matches!(
        run(&core, &json!({ "domains": many })),
        Err(CommandError::InvalidArgs(_))
    ));
    let chromium = copy_now(
        &core,
        serde_json::from_value(json!({ "target": 1 })).expect("args"),
        false,
    )
    .await;
    assert!(matches!(chromium, Err(CommandError::InvalidArgs(_))));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_command_waits_for_the_copy_on_a_thread_of_the_runtime() {
    let (core, _ui) = setup("chromium");
    let blocking = core.clone();
    let answer = tokio::task::spawn_blocking(move || run(&blocking, &Value::Null))
        .await
        .expect("joined")
        .expect("an answer");
    assert!(answer["message"].is_string(), "{answer}");
    assert_eq!(answer["copied"], 0);
}

#[test]
fn the_environments_host_is_a_site_of_the_filter() {
    let runtime = tokio::runtime::Runtime::new().expect("a runtime");
    let _entered = runtime.enter();
    let (_fake, router) = FakeBrowser::connect();
    let host = Host::new(
        Arc::new(RecordingUi::default()),
        router,
        runtime.handle().clone(),
    );
    let mut config = config("chromium");
    config.environments = serde_json::from_value(json!([
        { "id": "staging", "url": "https://Staging.Example.com:8443/app" }
    ]))
    .expect("environments");
    host.open_config(config, None).expect("opens");
    assert_eq!(environment_host(&host), None, "no environment chosen");
    host.update_shell_state(|shell| shell.environment = Some("staging".to_owned()));
    assert_eq!(
        environment_host(&host),
        Some("staging.example.com".to_owned())
    );
}

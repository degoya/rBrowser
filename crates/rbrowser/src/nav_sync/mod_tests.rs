use std::time::Duration;

use rb_config::Environment;

use super::*;
use crate::host::state::default_config;
use crate::host::ui::recording::UiCall;
use crate::sync_hub;
use crate::sync_hub::tests::{Setup, setup};

/// A window with views 1 desktop, 2 tablet, 3 mobile, a 20 ms settle time, the breakpoints
/// `decoupled` with `syncNavigation: false`, the SyncHub and navigation sync started.
pub(crate) fn started(decoupled: &[&str]) -> Setup {
    let setup = setup();
    let host = setup.core.host();
    let mut config = host.config();
    config.sync.navigation_settle_ms = 20;
    for breakpoint in &mut config.breakpoints {
        if decoupled.contains(&breakpoint.id.as_str()) {
            breakpoint.sync_navigation = false;
        }
    }
    host.open_config(config, None).expect("reopens");
    sync_hub::on_start(&setup.core);
    on_start(&setup.core);
    setup
}

/// The views `call` went to, of the calls `pick` takes.
pub(crate) fn views_of(setup: &Setup, pick: impl Fn(&UiCall) -> Option<ViewId>) -> Vec<ViewId> {
    setup.ui.calls().iter().filter_map(pick).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn navigation_sync_starts_once_per_window() {
    let setup = setup();
    assert!(for_host(setup.core.host()).is_none());
    assert_eq!(status(setup.core.host()), NavStatus::default());
    on_start(&setup.core);
    let first = for_host(setup.core.host()).expect("started");
    on_start(&setup.core);
    let second = for_host(setup.core.host()).expect("still there");
    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn the_options_come_from_the_project() {
    let mut config = default_config();
    config.sync.navigation_settle_ms = 400;
    config.sync.navigation_include_hash = true;
    config.environments = vec![Environment {
        id: "local".into(),
        url: "http://localhost:5173".into(),
        color: None,
        reload_interval_sec: None,
    }];
    let options = options_from(&config);
    assert_eq!(options.settle, Duration::from_millis(400));
    assert!(options.include_hash);
    assert_eq!(
        options.environments,
        vec!["http://localhost:5173".to_owned()]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_breakpoint_with_sync_navigation_false_is_decoupled() {
    let setup = started(&["mobile"]);
    let host = setup.core.host();
    assert_eq!(configured_decoupled(host, &host.config()), vec![ViewId(3)]);
    assert_eq!(status(host).decoupled, vec![ViewId(3)]);
    // The hub knows it too: the view follows no submit.
    let submit = rb_sync::parse_message(
        r#"{"kind":"event","v":1,"top":true,"seq":1,"time":0,"event":{"type":"submit","method":"post","action":"http://localhost/login","target":{"element":{"kind":"css","path":"form"},"offset":{"x":0.5,"y":0.5}}}}"#,
    )
    .expect("a message");
    let rb_sync::AgentMessage::Event(submit) = submit else {
        panic!("an event")
    };
    let sync = sync_hub::for_host(host).expect("started");
    let route = sync.hub().with(|hub| {
        hub.set_active(Some(ViewId(1)));
        hub.route(ViewId(1), &submit, std::time::Instant::now())
    });
    assert_eq!(
        route,
        rb_sync::Route::Follow {
            leader: ViewId(1),
            followers: vec![ViewId(2)]
        }
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_navigation_reaches_the_coupled_views() {
    let setup = started(&["mobile"]);
    let host = setup.core.host();
    assert_eq!(
        targets(host, Target::All).expect("views"),
        vec![ViewId(1), ViewId(2)]
    );
    assert_eq!(
        targets(host, Target::View(ViewId(3))).expect("a view"),
        vec![ViewId(3)],
        "its own address field"
    );
    let outcome = back(host, Target::All).expect("back");
    assert_eq!(outcome.views, vec![ViewId(1), ViewId(2)]);
    assert_eq!(
        outcome.skipped,
        vec![ViewId(3)],
        "named, not silently left out"
    );
    assert!(outcome.failed.is_empty());
    forward(host, Target::All).expect("forward");
    // Review batch 3, finding 2: an explicit reload reaches every view; the decoupled one
    // reloads its own page.
    let outcome = reload(host, Target::All, true).expect("reload");
    assert_eq!(outcome.views, vec![ViewId(1), ViewId(2), ViewId(3)]);
    assert!(outcome.skipped.is_empty());
    let backs = views_of(&setup, |call| match call {
        UiCall::Back(view) => Some(*view),
        _ => None,
    });
    assert_eq!(backs, vec![ViewId(1), ViewId(2)]);
    let reloads = views_of(&setup, |call| match call {
        UiCall::Reload(view, true) => Some(*view),
        _ => None,
    });
    assert_eq!(reloads, vec![ViewId(1), ViewId(2), ViewId(3)]);
    assert!(navigate(host, Target::All, "javascript:alert(1)").is_err());
}

/// Review batch 3, finding 9: a view that fails stops none of the others; only a failure in
/// every view is an error.
#[test]
fn one_view_s_failure_leaves_the_others_going() {
    let failing = |view: ViewId| {
        if view == ViewId(1) {
            Err(EngineError::UnknownView(view))
        } else {
            Ok(())
        }
    };
    let outcome = per_view(
        vec![ViewId(1), ViewId(2), ViewId(3)],
        vec![ViewId(4)],
        failing,
    )
    .expect("ran");
    assert_eq!(outcome.views, vec![ViewId(2), ViewId(3)]);
    assert_eq!(outcome.skipped, vec![ViewId(4)]);
    assert_eq!(
        outcome.failed,
        vec![ViewFailure {
            view: ViewId(1),
            error: "unknown view 1".to_owned(),
        }]
    );
    assert!(matches!(
        per_view(vec![ViewId(1)], Vec::new(), failing),
        Err(EngineError::UnknownView(_))
    ));
    assert_eq!(
        per_view(Vec::new(), vec![ViewId(2)], failing).expect("nothing to do"),
        NavOutcome {
            skipped: vec![ViewId(2)],
            ..NavOutcome::default()
        }
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn with_every_view_decoupled_all_is_the_active_view() {
    let setup = started(&["desktop", "tablet", "mobile"]);
    let host = setup.core.host();
    host.set_active_view(ViewId(2)).expect("a view");
    assert_eq!(targets(host, Target::All).expect("views"), vec![ViewId(2)]);
    let outcome = back(host, Target::All).expect("back");
    assert_eq!(outcome.skipped, vec![ViewId(1), ViewId(3)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn before_navigation_sync_runs_all_is_every_view() {
    let setup = setup();
    assert_eq!(
        targets(setup.core.host(), Target::All).expect("views"),
        vec![ViewId(1), ViewId(2), ViewId(3)]
    );
    assert!(couple(setup.core.host(), ViewId(1), false).is_err());
}

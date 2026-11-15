use std::sync::Arc;

use rb_config::EngineKind;
use rb_engine::external::{ExternalStatus, ExternalView, WindowRect};
use rb_engine::test_support::FakeEngine;
use serde_json::json;

use super::*;
use tokio::runtime::Handle;

use crate::cdp::fake::FakeBrowser;
use crate::control::{Core, CoreSetup};
use crate::engines::tests::until;
use crate::engines::{ARRANGE, HostEngines, for_host};
use crate::host::Host;
use crate::host::nav::marker_url;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

/// The core and its recorded window.
pub(super) struct Setup {
    pub(super) core: Arc<Core>,
    pub(super) ui: Arc<RecordingUi>,
    _fake: Arc<FakeBrowser>,
}

/// The default window: desktop, tablet and the mobile view (3) on `engine`.
pub(super) fn setup_on(engine: EngineKind) -> Setup {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    let mut config = default_config();
    config.breakpoints[2].engine = engine;
    host.open_config(config, None).expect("opens");
    for id in 1..=3 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    let core = Core::new(host, ui.clone(), Handle::current(), CoreSetup::default());
    Setup {
        core,
        ui,
        _fake: fake,
    }
}

fn external(status: ExternalStatus, embedded: bool) -> ExternalView {
    let mut view: ExternalView = serde_json::from_value(json!({
        "view": 3, "label": "Phone", "breakpoint": "phone", "engine": "firefox",
        "status": status, "url": "", "window": { "x": 1008, "y": 0, "width": 390, "height": 844 },
    }))
    .expect("an external view");
    view.embedded = embedded;
    view
}

fn hint() -> Option<NativeWindow> {
    Some(NativeWindow {
        process_id: 4242,
        window_class: None,
    })
}

fn embedding(on: bool) -> Embedding {
    Embedding {
        on,
        ..Embedding::default()
    }
}

#[test]
fn a_running_view_with_a_known_window_is_asked_into_its_cell_once() {
    let mut state = embedding(true);
    let request = EmbedRequest {
        view: ViewId(3),
        window: NativeWindow {
            process_id: 4242,
            window_class: None,
        },
        expected: Some(WindowRect {
            x: 1008,
            y: 0,
            width: 390,
            height: 844,
        }),
    };
    let view = external(ExternalStatus::Running, false);
    assert_eq!(decide(&state, &view, hint()), EmbedAction::Embed(request));
    state.requested.insert(ViewId(3));
    assert_eq!(decide(&state, &view, hint()), EmbedAction::Nothing);
    // Without a known window nothing can be asked.
    let fresh = embedding(true);
    assert_eq!(decide(&fresh, &view, None), EmbedAction::Nothing);
    // A view that does not run is left alone.
    for away in [
        ExternalStatus::Starting,
        ExternalStatus::Exited,
        ExternalStatus::Closed,
        ExternalStatus::Failed,
        ExternalStatus::Unavailable,
    ] {
        assert_eq!(
            decide(&fresh, &external(away, false), hint()),
            EmbedAction::Nothing,
            "{away:?}"
        );
    }
}

#[test]
fn a_view_asked_in_or_in_its_cell_is_released_when_it_should_not_be_there() {
    let view = external(ExternalStatus::Running, false);
    let in_cell = external(ExternalStatus::Running, true);
    // The setting off.
    let mut off = embedding(false);
    assert_eq!(decide(&off, &view, hint()), EmbedAction::Nothing);
    assert_eq!(decide(&off, &in_cell, hint()), EmbedAction::Release);
    off.requested.insert(ViewId(3));
    assert_eq!(decide(&off, &view, hint()), EmbedAction::Release);
    // Released by the person, or failed: not asked again, released when it is in.
    for mark in [
        |state: &mut Embedding| state.released.insert(ViewId(3)),
        |state: &mut Embedding| state.failed.insert(ViewId(3)),
    ] {
        let mut state = embedding(true);
        mark(&mut state);
        assert_eq!(decide(&state, &view, hint()), EmbedAction::Nothing);
        assert_eq!(decide(&state, &in_cell, hint()), EmbedAction::Release);
    }
}

/// A window whose mobile view (3) runs on a fake Firefox that names its process.
/// A core with the embedding switched on (off by default since 2026-11-13) and the Firefox
/// view running.
async fn running(process_id: Option<u32>) -> (Setup, Arc<HostEngines>, Arc<FakeEngine>) {
    running_on(EngineKind::Firefox, process_id).await
}

/// [`running`] with the mobile view on `engine`.
pub(super) async fn running_on(
    engine: EngineKind,
    process_id: Option<u32>,
) -> (Setup, Arc<HostEngines>, Arc<FakeEngine>) {
    let setup = setup_on(engine);
    let mut user = setup.core.user_settings();
    user.engines.external.embed = true;
    setup.core.replace_user_settings(user);
    // No sidecar: WebKit is unavailable too, until the fake engine is plugged in.
    crate::engines::start_with(&setup.core, None);
    let engines = for_host(setup.core.host()).expect("started");
    until(|| {
        engines
            .list()
            .first()
            .is_some_and(|view| view.status == ExternalStatus::Unavailable)
    })
    .await;
    let firefox = FakeEngine::new(engine);
    firefox.set_process_id(process_id);
    engines.windows().set_engine(firefox.clone());
    engines.windows().restart(engine, None).expect("restarts");
    until(|| {
        engines
            .list()
            .first()
            .is_some_and(|view| view.status == ExternalStatus::Running)
    })
    .await;
    (setup, engines, firefox)
}

/// Fails with `what` when `done` does not hold within two seconds.
pub(super) async fn wait(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..400 {
        if done() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("not reached: {what}");
}

pub(super) fn embeds(setup: &Setup) -> Vec<EmbedRequest> {
    setup
        .ui
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Embed(request) => Some(request),
            _ => None,
        })
        .collect()
}

pub(super) fn releases(setup: &Setup) -> Vec<ViewId> {
    setup
        .ui
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Release(view, _) => Some(view),
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_running_view_is_asked_into_its_cell_and_the_window_confirms() {
    let (setup, engines, _firefox) = running(Some(4242)).await;
    until(|| embeds(&setup).len() == 1).await;
    let request = embeds(&setup).remove(0);
    assert_eq!(request.view, ViewId(3));
    assert_eq!(request.window.process_id, 4242);
    assert_eq!(request.window.window_class, None);
    // Asked once; not in its cell until the window says so.
    engines.publish();
    assert_eq!(embeds(&setup).len(), 1);
    assert!(engines.is_embedded(ViewId(3)), "asked in");
    assert!(!engines.list()[0].embedded);
    engines.embedded_bound(ViewId(3));
    assert!(engines.list()[0].embedded);
    assert!(
        setup.core.host().shell_state().externals[0].embedded,
        "the badge knows"
    );
    assert!(releases(&setup).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_view_without_a_known_window_stays_a_window_of_its_own() {
    let (setup, engines, _firefox) = running(None).await;
    engines.publish();
    assert!(embeds(&setup).is_empty());
    assert!(!engines.is_embedded(ViewId(3)));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_embedding_is_not_asked_again_until_the_next_window() {
    let (setup, engines, _firefox) = running(Some(4242)).await;
    until(|| embeds(&setup).len() == 1).await;
    engines.embed_failed(ViewId(3), "no window of process 4242");
    engines.publish();
    assert_eq!(embeds(&setup).len(), 1, "not asked again");
    assert!(!engines.is_embedded(ViewId(3)));
    // A restart is a new window: asked in afresh.
    engines
        .windows()
        .restart(EngineKind::Firefox, Some(ViewId(3)))
        .expect("restarts");
    until(|| embeds(&setup).len() == 2).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_setting_off_releases_every_embedded_window_at_its_placement() {
    let (setup, engines, _firefox) = running(Some(4242)).await;
    until(|| embeds(&setup).len() == 1).await;
    engines.embedded_bound(ViewId(3));
    let mut settings = setup.core.user_settings().engines.external;
    settings.embed = false;
    engines.apply_settings(&settings);
    assert_eq!(releases(&setup), [ViewId(3)]);
    assert!(!engines.list()[0].embedded);
    assert!(!engines.is_embedded(ViewId(3)));
    // On again: asked in again.
    settings.embed = true;
    engines.apply_settings(&settings);
    assert_eq!(embeds(&setup).len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn pop_out_shows_an_external_view_as_its_own_window_and_pop_in_brings_it_back() {
    let (setup, engines, _firefox) = running(Some(4242)).await;
    until(|| embeds(&setup).len() == 1).await;
    engines.embedded_bound(ViewId(3));
    let answer = crate::pop_out::commands::run_with(
        &setup.core,
        crate::pop_out::commands::POP_OUT,
        &json!({ "view": 3 }),
        None,
    )
    .expect("pops out");
    // The grid never pops the stand-in out: `detached` is the host's word, as in views_list.
    assert_eq!(
        answer,
        json!({ "view": 3, "detached": false, "changed": true })
    );
    assert_eq!(releases(&setup), [ViewId(3)]);
    assert!(!engines.is_embedded(ViewId(3)));
    assert!(!setup.core.host().is_detached(ViewId(3)));
    let answer = crate::pop_out::commands::run_with(
        &setup.core,
        crate::pop_out::commands::POP_IN,
        &json!({ "view": 3 }),
        None,
    )
    .expect("pops in");
    assert_eq!(
        answer,
        json!({ "view": 3, "detached": false, "changed": true })
    );
    assert_eq!(embeds(&setup).len(), 2);
    // With the embedding off, a pop-in of a released view changes nothing.
    engines.release(ViewId(3));
    let mut settings = setup.core.user_settings().engines.external;
    settings.embed = false;
    engines.apply_settings(&settings);
    assert!(!engines.embed_again(ViewId(3)));
    assert_eq!(embeds(&setup).len(), 2);
}

/// Firefox on Windows: the pid rBrowser holds is the launcher's, the browser child owns the
/// windows (review finding 2), so the family of the launcher is what the window is looked
/// for in.
#[test]
fn the_window_is_looked_for_in_the_launchers_family() {
    // (pid, parent): launcher 100 -> browser 200 -> content 300; 400 is someone else.
    let links = [(100, 1), (200, 100), (300, 200), (400, 1)];
    assert_eq!(process_family(100, &links), [100, 200, 300]);
    assert_eq!(process_family(200, &links), [200, 300]);
    assert_eq!(
        process_family(999, &links),
        [999],
        "an unlisted root is itself"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn arrange_shows_the_embedded_views_as_their_own_windows() {
    let (setup, engines, _firefox) = running(Some(4242)).await;
    until(|| embeds(&setup).len() == 1).await;
    engines.embedded_bound(ViewId(3));
    let answer = crate::engines::run_command(&setup.core, ARRANGE, &json!(null))
        .expect("an engines command")
        .expect("arranges");
    assert_eq!(answer["released"], json!([3]));
    assert_eq!(releases(&setup), [ViewId(3)]);
    assert!(!engines.is_embedded(ViewId(3)));
    // Again: nothing left to release.
    let answer = crate::engines::run_command(&setup.core, ARRANGE, &json!(null))
        .expect("an engines command")
        .expect("arranges");
    assert_eq!(answer["released"], json!([]));
}

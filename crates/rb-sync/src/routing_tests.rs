use std::time::Duration;

use tokio::runtime::Handle;

use super::*;
use crate::desync::DesyncReason;
use crate::driver::SyncHub;
use crate::event::{KeyData, MouseButton, MouseData, SyncEvent};
use crate::hub::Hub;
use crate::message::EventMessage;
use crate::replay::RecordingReplay;
use crate::switches::SyncSwitches;
use crate::switches::tests::target;

/// The grid: views 1 (the leader) and 2 on Chromium; external: 4 on Firefox, 5 on WebKit (no
/// replay yet), 6 a Firefox view whose window ended.
const LEADER: ViewId = ViewId(1);
const GRID: ViewId = ViewId(2);
const FIREFOX: ViewId = ViewId(4);
const WEBKIT: ViewId = ViewId(5);
const ENDED: ViewId = ViewId(6);

struct Setup {
    router: Arc<EngineRouter>,
    chromium: Arc<RecordingReplay>,
    firefox: Arc<RecordingReplay>,
}

fn setup() -> Setup {
    let chromium = Arc::new(RecordingReplay::default());
    let firefox = Arc::new(RecordingReplay::default());
    let router = Arc::new(EngineRouter::new(chromium.clone()));
    router.set_engine(EngineKind::Firefox, firefox.clone());
    router.set_routes([
        (FIREFOX, ViewRoute::Engine(EngineKind::Firefox)),
        (WEBKIT, ViewRoute::Engine(EngineKind::Webkit)),
        (ENDED, ViewRoute::Stopped),
    ]);
    Setup {
        router,
        chromium,
        firefox,
    }
}

fn click(seq: u64) -> EventMessage {
    EventMessage {
        v: 1,
        top: true,
        frame: None,
        seq,
        time: 0,
        path: Vec::new(),
        event: SyncEvent::Click(MouseData {
            control: None,
            button: MouseButton::Left,
            detail: 1,
            modifiers: 0,
            target: target("main > button"),
        }),
    }
}

fn request(view: ViewId) -> ReplayRequest {
    ReplayRequest {
        source: LEADER,
        target: view,
        top: true,
        frame: None,
        event: click(1).event.into(),
        block_submit: false,
    }
}

#[tokio::test]
async fn each_view_s_events_go_to_its_engine() {
    let setup = setup();
    setup.firefox.answer(FIREFOX, ReplayOutcome::NotFound);
    assert_eq!(
        setup.router.dispatch(request(GRID)).await,
        ReplayOutcome::Done
    );
    assert_eq!(
        setup.router.dispatch(request(FIREFOX)).await,
        ReplayOutcome::NotFound
    );
    assert_eq!(
        setup.router.dispatch(request(WEBKIT)).await,
        ReplayOutcome::Unsupported
    );
    assert_eq!(
        setup.router.dispatch(request(ENDED)).await,
        ReplayOutcome::Unsupported
    );
    assert_eq!(setup.chromium.calls_for(GRID).len(), 1);
    assert_eq!(setup.firefox.calls_for(FIREFOX).len(), 1);
    assert!(setup.chromium.calls_for(FIREFOX).is_empty());
    assert!(setup.firefox.calls_for(ENDED).is_empty());
}

#[test]
fn an_unnamed_view_is_chromium_s_and_routes_can_change() {
    let setup = setup();
    assert_eq!(
        setup.router.route(ViewId(42)),
        ViewRoute::Engine(EngineKind::Chromium)
    );
    setup
        .router
        .set_routes([(ENDED, ViewRoute::Engine(EngineKind::Firefox))]);
    assert_eq!(
        setup.router.route(ENDED),
        ViewRoute::Engine(EngineKind::Firefox)
    );
    assert_eq!(
        setup.router.route(FIREFOX),
        ViewRoute::Engine(EngineKind::Chromium)
    );
    assert_eq!(
        EngineRouter::default().route(GRID),
        ViewRoute::Engine(EngineKind::Chromium)
    );
}

/// The SyncHub over grid and external views at once: the leader's click reaches the Chromium
/// view and the Firefox window; the Firefox view that cannot find the element is desynced, the
/// ended and the not-yet-available views are skipped without a badge.
#[tokio::test(flavor = "multi_thread")]
async fn the_sync_hub_routes_to_grid_and_external_views_together() {
    let setup = setup();
    setup.firefox.answer(FIREFOX, ReplayOutcome::NotFound);
    let hub = SyncHub::new(
        Hub::new(SyncSwitches::default()),
        setup.router.clone(),
        Duration::from_millis(500),
        Handle::current(),
        Arc::new(|| {}),
    );
    hub.set_views(&[LEADER, GRID, FIREFOX, WEBKIT, ENDED]);
    hub.with(|hub| hub.set_active(Some(LEADER)));
    hub.on_event(LEADER, &click(1));
    // The polls give up after a generous deadline; no clock is compared.
    let patience = Duration::from_secs(10);
    tokio::time::timeout(patience, async {
        while setup.chromium.calls_for(GRID).is_empty()
            || setup.firefox.calls_for(FIREFOX).is_empty()
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("replayed");
    let desynced = tokio::time::timeout(patience, async {
        loop {
            let desynced = hub.with(|hub| hub.status()).desynced;
            if !desynced.is_empty() {
                break desynced;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("desynced");
    assert_eq!(desynced.len(), 1);
    assert_eq!(desynced[0].view, FIREFOX);
    assert_eq!(desynced[0].reason, DesyncReason::NotFound);
    // A key press: the desynced Firefox view is left out, the rest as before.
    let key = EventMessage {
        event: SyncEvent::Keydown(KeyData {
            key: "a".into(),
            code: "KeyA".into(),
            key_code: 65,
            location: 0,
            repeat: false,
            modifiers: 0,
            sensitive: false,
            target: None,
        }),
        ..click(2)
    };
    hub.on_event(LEADER, &key);
    tokio::time::timeout(patience, async {
        while setup.chromium.calls_for(GRID).len() < 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("replayed");
    assert_eq!(setup.firefox.calls_for(FIREFOX).len(), 1);
}

/// RB-090-32: each engine's replays are counted apart (`sync.replay.<engine>`), so K1 and K11
/// read from one snapshot.
#[test]
fn the_counter_of_a_view_names_its_engine() {
    use rb_util::perf::names;
    let setup = setup();
    assert_eq!(setup.router.counter(GRID), names::SYNC_REPLAY_CHROMIUM);
    assert_eq!(
        setup.router.counter(ViewId(99)),
        names::SYNC_REPLAY_CHROMIUM
    );
    assert_eq!(setup.router.counter(FIREFOX), names::SYNC_REPLAY_FIREFOX);
    assert_eq!(setup.router.counter(WEBKIT), names::SYNC_REPLAY_WEBKIT);
    assert_eq!(setup.router.counter(ENDED), names::SYNC_REPLAY);
    assert_eq!(
        engine_counter(EngineKind::Android),
        names::SYNC_REPLAY_ANDROID
    );
    assert_eq!(
        engine_counter(EngineKind::Safari),
        names::SYNC_REPLAY_SAFARI
    );
    assert_eq!(engine_counter(EngineKind::Ios), names::SYNC_REPLAY_IOS);
}

use super::*;

fn spec(id: u32, engine: EngineKind) -> ExternalSpec {
    ExternalSpec {
        view: ViewId(id),
        label: format!("View {id}"),
        breakpoint: format!("bp{id}"),
        engine,
        url: "http://localhost/".into(),
    }
}

fn registry(restart_on_crash: bool) -> Registry {
    let mut registry = Registry::new(Backoff::default(), restart_on_crash);
    registry.sync(&[spec(4, EngineKind::Firefox), spec(5, EngineKind::Webkit)]);
    registry
}

#[test]
fn the_backoff_doubles_up_to_the_last_attempt() {
    let backoff = Backoff::default();
    assert_eq!(backoff.delay(0), None);
    assert_eq!(backoff.delay(1), Some(Duration::from_secs(1)));
    assert_eq!(backoff.delay(2), Some(Duration::from_secs(2)));
    assert_eq!(backoff.delay(3), Some(Duration::from_secs(4)));
    assert_eq!(backoff.delay(4), None);
}

#[test]
fn sync_reports_the_new_and_the_gone_views_and_keeps_the_rest() {
    let mut registry = registry(true);
    registry.navigated(ViewId(4), "http://localhost/kept");
    let (added, gone) =
        registry.sync(&[spec(4, EngineKind::Firefox), spec(6, EngineKind::Firefox)]);
    assert_eq!(added, [ViewId(6)]);
    assert_eq!(gone, [ViewId(5)]);
    assert_eq!(
        registry.get(ViewId(4)).map(|view| view.url.as_str()),
        Some("http://localhost/kept")
    );
    assert_eq!(
        registry.of_engine(EngineKind::Firefox),
        [ViewId(4), ViewId(6)]
    );
    assert!(registry.of_engine(EngineKind::Webkit).is_empty());
}

/// A view whose breakpoint moves to another engine is a new view: the old window closes, the
/// new engine starts it on the page it showed (RB-040-04 review, finding 6).
#[test]
fn a_view_that_changes_its_engine_goes_and_comes_back() {
    let mut registry = registry(true);
    registry.started(ViewId(5), vec!["platform".into()], Instant::now());
    registry.navigated(ViewId(5), "http://localhost/kept");
    let (added, gone) =
        registry.sync(&[spec(4, EngineKind::Firefox), spec(5, EngineKind::Firefox)]);
    assert_eq!(added, [ViewId(5)]);
    assert_eq!(gone, [ViewId(5)]);
    let view = registry.get(ViewId(5)).expect("still there");
    assert_eq!(view.engine, EngineKind::Firefox);
    assert_eq!(view.status, ExternalStatus::Starting);
    assert_eq!(view.url, "http://localhost/kept");
    assert!(view.unsupported.is_empty());
    assert!(registry.of_engine(EngineKind::Webkit).is_empty());
}

#[test]
fn an_ended_view_restarts_three_times_then_gives_up() {
    let mut registry = registry(true);
    let now = Instant::now();
    registry.started(ViewId(4), Vec::new(), now);
    for wait in [1, 2, 4] {
        assert_eq!(
            registry.exited(ViewId(4), true, now),
            ExitAction::RestartAfter(Duration::from_secs(wait))
        );
        assert_eq!(
            registry.get(ViewId(4)).map(|view| view.status),
            Some(ExternalStatus::Restarting)
        );
        registry.started(ViewId(4), Vec::new(), now);
    }
    assert_eq!(registry.exited(ViewId(4), true, now), ExitAction::GiveUp);
    assert_eq!(
        registry.get(ViewId(4)).map(|view| view.status),
        Some(ExternalStatus::Exited)
    );
}

#[test]
fn a_manual_restart_and_a_stable_run_start_a_new_streak() {
    let mut registry = registry(true);
    let now = Instant::now();
    registry.started(ViewId(4), Vec::new(), now);
    for _ in 0..3 {
        registry.exited(ViewId(4), true, now);
    }
    assert_eq!(registry.exited(ViewId(4), true, now), ExitAction::GiveUp);
    registry.restart_requested(ViewId(4)).expect("known");
    registry.started(ViewId(4), Vec::new(), now);
    assert_eq!(
        registry.exited(ViewId(4), true, now),
        ExitAction::RestartAfter(Duration::from_secs(1))
    );
    registry.started(ViewId(4), Vec::new(), now);
    let later = now + STABLE_RUN;
    assert_eq!(
        registry.exited(ViewId(4), true, later),
        ExitAction::RestartAfter(Duration::from_secs(1))
    );
}

#[test]
fn without_restart_on_crash_a_crashed_view_stays_ended() {
    let mut registry = registry(false);
    registry.started(ViewId(4), Vec::new(), Instant::now());
    assert_eq!(
        registry.exited(ViewId(4), true, Instant::now()),
        ExitAction::Stay
    );
    assert_eq!(
        registry.get(ViewId(4)).map(|view| view.status),
        Some(ExternalStatus::Exited)
    );
    assert_eq!(
        registry.exited(ViewId(9), true, Instant::now()),
        ExitAction::Stay
    );
}

/// A window the person closed stays closed, restart or not: only a crash restarts
/// (RB-080-38). The restart command starts a new streak as before.
#[test]
fn a_window_closed_by_hand_stays_closed_until_asked_to_open_again() {
    let mut registry = registry(true);
    let now = Instant::now();
    registry.started(ViewId(4), Vec::new(), now);
    assert_eq!(registry.exited(ViewId(4), false, now), ExitAction::Stay);
    assert_eq!(
        registry.get(ViewId(4)).map(|view| view.status),
        Some(ExternalStatus::Closed)
    );
    // The next sync keeps it closed: not new, not gone.
    let (added, gone) = registry.sync(&[spec(4, EngineKind::Firefox), spec(5, EngineKind::Webkit)]);
    assert!(added.is_empty() && gone.is_empty(), "{added:?} {gone:?}");
    registry.restart_requested(ViewId(4)).expect("known");
    assert_eq!(
        registry.get(ViewId(4)).map(|view| view.status),
        Some(ExternalStatus::Restarting)
    );
}

#[test]
fn statuses_carry_their_reason_and_the_unsupported_fields() {
    let mut registry = registry(true);
    registry.unavailable(ViewId(5), "WebKit is not available yet".into());
    registry.failed(ViewId(4), "no Firefox found".into());
    let list = registry.list();
    assert_eq!(list[0].status, ExternalStatus::Failed);
    assert_eq!(list[0].reason.as_deref(), Some("no Firefox found"));
    assert_eq!(list[1].status, ExternalStatus::Unavailable);
    registry.started(ViewId(4), vec!["reducedMotion".into()], Instant::now());
    let view = registry.get(ViewId(4)).expect("known");
    assert_eq!(view.reason, None);
    assert_eq!(view.unsupported, ["reducedMotion"]);
    assert_eq!(
        registry.restart_requested(ViewId(9)),
        Err(RegistryError::UnknownView(ViewId(9)))
    );
}

#[test]
fn an_external_view_serializes_for_the_shell() {
    let mut registry = registry(true);
    registry.moved(
        ViewId(4),
        WindowRect {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
        },
    );
    let json = serde_json::to_value(registry.get(ViewId(4))).expect("serializes");
    assert_eq!(json["view"], 4);
    assert_eq!(json["engine"], "firefox");
    assert_eq!(json["status"], "starting");
    assert_eq!(json["window"]["width"], 3);
    assert!(json.get("attempts").is_none());
    assert!(json.get("unsupported").is_none());
}

#[test]
fn a_running_view_is_embedded_until_it_starts_again() {
    let mut registry = registry(true);
    // Not running yet: never embedded.
    assert!(!registry.set_embedded(ViewId(4), true));
    assert!(!registry.get(ViewId(4)).expect("view 4").embedded);
    registry.started(ViewId(4), Vec::new(), Instant::now());
    assert!(registry.set_embedded(ViewId(4), true));
    assert!(registry.get(ViewId(4)).expect("view 4").embedded);
    // Again is no change.
    assert!(!registry.set_embedded(ViewId(4), true));
    // A new window is not embedded until the app binds it again.
    registry.starting(ViewId(4));
    assert!(!registry.get(ViewId(4)).expect("view 4").embedded);
    registry.started(ViewId(4), Vec::new(), Instant::now());
    assert!(registry.set_embedded(ViewId(4), true));
    assert!(registry.set_embedded(ViewId(4), false));
    assert!(!registry.set_embedded(ViewId(9), true), "unknown view");
    let json = serde_json::to_value(registry.get(ViewId(4)).expect("view 4")).expect("json");
    assert_eq!(json["embedded"], false);
}

/// A window that ended is not in its cell any more, whatever `exited` or a status says.
#[test]
fn an_ended_or_closed_view_is_no_longer_embedded() {
    let mut registry = registry(true);
    for (crashed, status) in [
        (true, ExternalStatus::Restarting),
        (false, ExternalStatus::Closed),
    ] {
        registry.started(ViewId(4), Vec::new(), Instant::now());
        assert!(registry.set_embedded(ViewId(4), true));
        registry.exited(ViewId(4), crashed, Instant::now());
        let view = registry.get(ViewId(4)).expect("view 4");
        assert_eq!(view.status, status);
        assert!(!view.embedded, "{status:?}");
    }
    registry.started(ViewId(5), Vec::new(), Instant::now());
    assert!(registry.set_embedded(ViewId(5), true));
    registry.failed(ViewId(5), "gone".into());
    assert!(!registry.get(ViewId(5)).expect("view 5").embedded);
}

/// A WebKit view's context was built anew (a changed cell factor, RB-080-40): its window is a
/// new one, so it is not in its cell until the app binds it again, and the app hears it once.
#[test]
fn a_replaced_window_leaves_its_cell_and_is_reported_once() {
    let mut registry = registry(true);
    registry.started(ViewId(4), Vec::new(), Instant::now());
    assert!(registry.set_embedded(ViewId(4), true));
    registry.window_replaced(ViewId(4));
    assert!(!registry.get(ViewId(4)).expect("view 4").embedded);
    assert_eq!(registry.take_replaced(), vec![ViewId(4)]);
    assert_eq!(registry.take_replaced(), Vec::<ViewId>::new());
    registry.window_replaced(ViewId(9));
    assert_eq!(
        registry.take_replaced(),
        Vec::<ViewId>::new(),
        "unknown view"
    );
}

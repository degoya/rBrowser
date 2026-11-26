use std::time::{Duration, Instant};

use super::*;
use crate::event::{
    ChangeData, HoverData, KeyData, MouseButton, MouseData, PointerData, PointerPhase, SubmitData,
    TargetData,
};
use crate::leader::FOLLOW_WINDOW;
use crate::switches::tests::target;

pub(super) const A: ViewId = ViewId(1);
pub(super) const B: ViewId = ViewId(2);
pub(super) const C: ViewId = ViewId(3);

fn message(event: SyncEvent) -> EventMessage {
    EventMessage {
        v: 1,
        top: true,
        frame: None,
        seq: 1,
        time: 0,
        path: Vec::new(),
        event,
    }
}

pub(super) fn click() -> EventMessage {
    message(SyncEvent::Click(MouseData {
        control: None,
        button: MouseButton::Left,
        detail: 1,
        modifiers: 0,
        target: target("form > button"),
    }))
}

fn pointer(phase: PointerPhase) -> EventMessage {
    message(SyncEvent::Pointer(PointerData {
        phase,
        button: MouseButton::Left,
        buttons: 1,
        pointer_type: "mouse".into(),
        modifiers: 0,
        target: target("form > button"),
    }))
}

fn hover() -> EventMessage {
    message(SyncEvent::Hover(HoverData {
        target: target("nav"),
        menu: false,
    }))
}

pub(super) fn submit() -> EventMessage {
    message(SyncEvent::Submit(SubmitData {
        method: "post".into(),
        action: "http://localhost/form".into(),
        target: target("form"),
    }))
}

/// Three views, A active.
pub(super) fn hub() -> Hub {
    let mut hub = Hub::new(SyncSwitches::default());
    hub.set_views(&[A, B, C]);
    hub.set_active(Some(A));
    hub
}

pub(super) fn targets(route: &Route) -> Vec<ViewId> {
    match route {
        Route::Replay(requests) => requests.iter().map(|request| request.target).collect(),
        other => panic!("not a replay: {other:?}"),
    }
}

#[test]
fn an_event_of_a_reaches_b_and_c_but_not_a() {
    let mut hub = hub();
    let route = hub.route(A, &click(), Instant::now());
    assert_eq!(targets(&route), vec![B, C]);
    let Route::Replay(requests) = route else {
        unreachable!()
    };
    assert!(requests.iter().all(|request| request.source == A));
}

#[test]
fn an_excluded_view_gets_nothing_and_sends_nothing() {
    let mut hub = hub();
    assert_eq!(hub.exclude(B, true), Ok(true));
    assert_eq!(targets(&hub.route(A, &click(), Instant::now())), vec![C]);
    hub.set_active(Some(B));
    assert_eq!(
        hub.route(B, &click(), Instant::now()),
        Route::Skip(SkipReason::Excluded)
    );
    assert!(
        !hub.captures(B),
        "an excluded view's agent does not capture"
    );
    assert_eq!(hub.exclude(B, false), Ok(true));
    assert_eq!(targets(&hub.route(B, &click(), Instant::now())), vec![A, C]);
    assert_eq!(
        hub.exclude(ViewId(9), true),
        Err(HubError::UnknownView(ViewId(9)))
    );
}

#[test]
fn only_the_active_view_with_sync_on_and_the_kind_on_is_routed() {
    let mut hub = hub();
    let now = Instant::now();
    assert_eq!(
        hub.route(B, &click(), now),
        Route::Skip(SkipReason::NotActive)
    );
    hub.set_kind(SyncKind::Click, false);
    assert_eq!(
        hub.route(A, &click(), now),
        Route::Skip(SkipReason::KindOff(SyncKind::Click))
    );
    hub.set_kind(SyncKind::Click, true);
    hub.set_enabled(false);
    assert_eq!(
        hub.route(A, &click(), now),
        Route::Skip(SkipReason::Disabled)
    );
    assert!(!hub.captures(A));
    hub.set_enabled(true);
    assert!(hub.captures(A) && !hub.captures(B));
}

#[test]
fn a_new_active_view_is_routed_at_once() {
    let mut hub = hub();
    assert!(hub.set_active(Some(C)));
    // At once: the very next route, nothing in between.
    let route = hub.route(C, &click(), Instant::now());
    assert_eq!(targets(&route), vec![A, B]);
    assert!(!hub.set_active(Some(C)), "unchanged");
}

#[test]
fn skip_next_click_skips_the_whole_click_once() {
    let mut hub = hub();
    let now = Instant::now();
    hub.skip_next_click();
    assert!(hub.status().skip_next_click);
    assert_eq!(
        hub.route(A, &pointer(PointerPhase::Down), now),
        Route::Skip(SkipReason::SkippedClick)
    );
    assert_eq!(
        hub.route(A, &pointer(PointerPhase::Up), now),
        Route::Skip(SkipReason::SkippedClick)
    );
    assert!(hub.route(A, &hover(), now).is_replay());
    assert_eq!(
        hub.route(A, &click(), now),
        Route::Skip(SkipReason::SkippedClick)
    );
    assert!(!hub.status().skip_next_click);
    assert!(hub.route(A, &click(), now).is_replay());
}

fn key(key: &str) -> EventMessage {
    message(SyncEvent::Keydown(KeyData {
        key: key.into(),
        code: key.into(),
        key_code: 27,
        location: 0,
        repeat: false,
        modifiers: 0,
        sensitive: false,
        target: None,
    }))
}

#[test]
fn escape_disarms_skip_next_click_and_still_goes_out() {
    let mut hub = hub();
    let now = Instant::now();
    hub.skip_next_click();
    assert!(hub.route(A, &key("a"), now).is_replay());
    assert!(hub.skip_next_armed());
    assert!(hub.route(A, &key("Escape"), now).is_replay());
    assert!(!hub.skip_next_armed());
    assert!(hub.route(A, &click(), now).is_replay());
}

#[test]
fn escape_disarms_skip_next_click_with_keyboard_sync_off() {
    let mut hub = hub();
    hub.set_kind(SyncKind::Keyboard, false);
    hub.skip_next_click();
    assert_eq!(
        hub.route(A, &key("Escape"), Instant::now()),
        Route::Skip(SkipReason::KindOff(SyncKind::Keyboard))
    );
    assert!(!hub.skip_next_armed());
}

#[test]
fn escape_of_another_view_leaves_the_skip_armed() {
    let mut hub = hub();
    hub.skip_next_click();
    hub.route(B, &key("Escape"), Instant::now());
    assert!(hub.skip_next_armed());
}

#[test]
fn focus_goes_out_only_with_focus_sync_and_change_with_form_sync() {
    let mut hub = hub();
    let now = Instant::now();
    let focus = message(SyncEvent::Focus(TargetData {
        target: target("#q"),
    }));
    let change = message(SyncEvent::Change(ChangeData {
        value: Some("de".into()),
        checked: None,
        selected: Some(vec![1]),
        sensitive: false,
        target: target("#lang"),
    }));
    assert_eq!(
        hub.route(A, &focus, now),
        Route::Skip(SkipReason::KindOff(SyncKind::Focus))
    );
    assert!(hub.route(A, &change, now).is_replay());
    hub.set_kind(SyncKind::Focus, true);
    hub.set_kind(SyncKind::Forms, false);
    assert!(hub.route(A, &focus, now).is_replay());
    assert_eq!(
        hub.route(A, &change, now),
        Route::Skip(SkipReason::KindOff(SyncKind::Forms))
    );
}

#[test]
fn the_leader_submits_alone_and_the_others_follow_its_url() {
    let mut hub = hub();
    let now = Instant::now();
    let Route::Replay(requests) = hub.route(A, &click(), now) else {
        panic!("a replay")
    };
    assert!(requests.iter().all(|request| request.block_submit));
    assert_eq!(
        hub.route(A, &submit(), now),
        Route::Follow {
            leader: A,
            followers: vec![B, C]
        }
    );
    assert_eq!(hub.follow_leader(), Some(A), "the follow is open");
    let navigations = hub.navigated(A, "http://localhost/thanks", now);
    assert_eq!(hub.follow_leader(), None, "the leader's load took it");
    assert_eq!(
        navigations,
        vec![
            Navigation {
                view: B,
                url: "http://localhost/thanks".into()
            },
            Navigation {
                view: C,
                url: "http://localhost/thanks".into()
            },
        ]
    );
    // Until they commit, the followers get no events.
    assert_eq!(
        hub.route(A, &click(), now),
        Route::Skip(SkipReason::NoTargets)
    );
    assert!(hub.navigated(B, "http://localhost/thanks", now).is_empty());
    assert_eq!(targets(&hub.route(A, &click(), now)), vec![B]);
    // The follow is used up.
    assert!(hub.navigated(A, "http://localhost/next", now).is_empty());
}

#[test]
fn a_follow_expires_without_the_leader_s_navigation() {
    let mut hub = hub();
    let now = Instant::now();
    hub.route(A, &submit(), now);
    let late = now + FOLLOW_WINDOW + Duration::from_secs(1);
    assert!(hub.navigated(A, "http://localhost/late", late).is_empty());
}

#[test]
fn without_leader_only_submit_every_view_submits_by_its_own_input() {
    let mut hub = hub();
    hub.set_leader_only_submit(false);
    let now = Instant::now();
    let Route::Replay(requests) = hub.route(A, &click(), now) else {
        panic!("a replay")
    };
    assert!(requests.iter().all(|request| !request.block_submit));
    assert_eq!(
        hub.route(A, &submit(), now),
        Route::Skip(SkipReason::SubmitFollowsInput)
    );
    assert!(hub.navigated(A, "http://localhost/thanks", now).is_empty());
}

#[test]
fn without_navigation_sync_nobody_follows_a_submit() {
    let mut hub = hub();
    hub.set_kind(SyncKind::Navigation, false);
    assert_eq!(
        hub.route(A, &submit(), Instant::now()),
        Route::Skip(SkipReason::KindOff(SyncKind::Navigation))
    );
}

struct SessionLeaders;

impl SubmitPolicy for SessionLeaders {
    fn submitters(&self, _leader: ViewId, targets: &[ViewId]) -> Vec<ViewId> {
        targets.iter().copied().filter(|view| *view == C).collect()
    }
}

#[test]
fn the_session_hook_lets_other_session_leaders_submit_themselves() {
    let mut hub = hub();
    hub.set_submit_policy(std::sync::Arc::new(SessionLeaders));
    let now = Instant::now();
    let Route::Replay(requests) = hub.route(A, &click(), now) else {
        panic!("a replay")
    };
    let blocked: Vec<(ViewId, bool)> = requests
        .iter()
        .map(|request| (request.target, request.block_submit))
        .collect();
    assert_eq!(blocked, vec![(B, true), (C, false)]);
    assert_eq!(
        hub.route(A, &submit(), now),
        Route::Follow {
            leader: A,
            followers: vec![B]
        }
    );
}

#[test]
fn a_view_that_cannot_follow_is_desynced_and_left_out() {
    let mut hub = hub();
    let now = Instant::now();
    let event = click().event;
    assert!(hub.record(C, &event, &ReplayOutcome::NotFound));
    assert!(!hub.record(B, &event, &ReplayOutcome::Done));
    assert!(!hub.record(B, &event, &ReplayOutcome::Unsupported));
    let desynced = hub.status().desynced;
    assert_eq!(desynced.len(), 1);
    assert_eq!(desynced[0].view, C);
    assert_eq!(desynced[0].reason, DesyncReason::NotFound);
    assert_eq!(targets(&hub.route(A, &click(), now)), vec![B]);
    // Its next successful navigation brings it back.
    hub.navigated(C, "http://localhost/", now);
    assert!(hub.status().desynced.is_empty());
}

#[test]
fn every_failure_but_hover_and_scroll_desyncs() {
    let mut hub = hub();
    assert!(!hub.record(B, &hover().event, &ReplayOutcome::NotFound));
    assert!(hub.record(B, &click().event, &ReplayOutcome::NotVisible));
    assert!(hub.record(C, &click().event, &ReplayOutcome::Timeout));
    assert!(hub.record(A, &click().event, &ReplayOutcome::Failed("x".into())));
    let reasons: Vec<DesyncReason> = hub
        .status()
        .desynced
        .iter()
        .map(|desync| desync.reason)
        .collect();
    assert_eq!(
        reasons,
        vec![
            DesyncReason::Failed,
            DesyncReason::NotVisible,
            DesyncReason::Timeout
        ]
    );
}

#[test]
fn resync_clears_the_badge_and_names_the_leader_to_load() {
    let mut hub = hub();
    hub.record(C, &click().event, &ReplayOutcome::NotFound);
    let now = Instant::now();
    assert_eq!(hub.resync(C, now), Ok(Some(A)));
    assert!(hub.status().desynced.is_empty());
    // Loading: no events until it commits.
    assert_eq!(targets(&hub.route(A, &click(), now)), vec![B]);
    assert_eq!(hub.resync(A, now), Ok(None), "the leader follows nobody");
    assert_eq!(
        hub.resync(ViewId(9), now),
        Err(HubError::UnknownView(ViewId(9)))
    );
    assert_eq!(hub.desynced(), Vec::<ViewId>::new());
}

#[test]
fn views_that_are_gone_are_forgotten() {
    let mut hub = hub();
    hub.exclude(B, true).expect("known");
    hub.record(C, &click().event, &ReplayOutcome::NotFound);
    hub.set_views(&[B]);
    let status = hub.status();
    assert_eq!(status.excluded, vec![B]);
    assert!(status.desynced.is_empty());
    assert_eq!(hub.active(), None, "the active view is gone");
}

#[test]
fn a_view_navigation_sync_loads_gets_no_events_until_it_commits() {
    // RB-020-06: the fallback has B load the leader's URL.
    let mut hub = hub();
    let now = Instant::now();
    assert!(!hub.is_loading(B, now));
    hub.mark_loading(B, now);
    assert!(hub.is_loading(B, now));
    assert_eq!(targets(&hub.route(A, &click(), now)), vec![C]);
    hub.navigated(B, "http://localhost/b", now);
    assert!(!hub.is_loading(B, now));
    // An unknown view is not marked.
    hub.mark_loading(ViewId(9), now);
    assert!(!hub.is_loading(ViewId(9), now));
}

#[test]
fn a_view_with_its_navigation_decoupled_does_not_follow_a_submit() {
    // RB-020-06: `syncNavigation: false` keeps the view on its page.
    let mut hub = hub();
    hub.set_navigation_decoupled(&[C, ViewId(9)]);
    let now = Instant::now();
    assert_eq!(
        hub.route(A, &submit(), now),
        Route::Follow {
            leader: A,
            followers: vec![B]
        }
    );
    hub.set_navigation_decoupled(&[B, C]);
    assert_eq!(
        hub.route(A, &submit(), now),
        Route::Skip(SkipReason::NoTargets)
    );
}

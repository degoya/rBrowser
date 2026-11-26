//! The SyncHub's routing, leader and desync logic, without a runtime: [`Hub`] decides where a
//! captured event goes ([`Route`]), takes the replays' outcomes and the views' navigations, and
//! says what the shell and MCP show ([`HubStatus`]). The async side - queues per view, the
//! timeout, the engine - is [`crate::SyncHub`].
//!
//! The rules (`docs/sync-protocol.md`, SyncHub):
//!
//! - Only the active view's events are routed (the leader); with sync off, a kind off or the
//!   active view excluded, nothing.
//! - An event goes to every other view but the excluded, the desynced and the ones loading a
//!   URL the hub gave them (a follow or a resync): their old document is going away, so the
//!   event is dropped for them, never buffered.
//! - `Leader S N` skips the next click: the pointer events up to and including the next
//!   `click`, `dblclick` or `contextmenu`; Escape in the active view takes it back (RB-020-05).
//! - Leader-only submit: see [`crate::leader`].
//! - A view whose replay of a click, input, key or submit ends `not_found`, `not_visible`, in a
//!   timeout or an error is desynced (hover and scroll never desync); it is back in sync after
//!   its next committed navigation or a resync. An outcome of a replay routed before that
//!   ([`Hub::epoch`]) no longer counts.
//! - A load the hub gave ends when it commits, when the view's load finishes, or when it never
//!   starts ([`crate::loading`]).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

use rb_engine::ViewId;

use crate::desync::{DesyncReason, DesyncTable};
use crate::event::SyncEvent;
pub use crate::hub_types::{HubError, HubStatus, Navigation, Route, SkipReason};
use crate::leader::{Follow, LeaderOnly, SubmitPolicy};
use crate::loading::{LoadChange, LoadTable};
use crate::message::EventMessage;
use crate::replay::{ReplayOutcome, ReplayRequest};
use crate::switches::{SyncKind, SyncSwitches, marks_desync};

/// The routing state.
pub struct Hub {
    switches: SyncSwitches,
    views: Vec<ViewId>,
    active: Option<ViewId>,
    excluded: BTreeSet<ViewId>,
    /// Views loading a URL the hub gave them, until it commits or finishes.
    loading: LoadTable,
    /// Per view, how often it navigated or was resynced: a replay's outcome counts only in the
    /// document it was routed to.
    epochs: BTreeMap<ViewId, u64>,
    skip_click: bool,
    desync: DesyncTable,
    follow: Option<Follow>,
    policy: Arc<dyn SubmitPolicy>,
    /// Views whose navigation is decoupled (RB-020-06): they never follow a submit.
    navigation_decoupled: BTreeSet<ViewId>,
}

impl Hub {
    /// A hub without views, with `switches`.
    pub fn new(switches: SyncSwitches) -> Self {
        Self {
            switches,
            views: Vec::new(),
            active: None,
            excluded: BTreeSet::new(),
            loading: LoadTable::default(),
            epochs: BTreeMap::new(),
            skip_click: false,
            desync: DesyncTable::default(),
            follow: None,
            policy: Arc::new(LeaderOnly),
            navigation_decoupled: BTreeSet::new(),
        }
    }

    /// The switches in effect.
    pub fn switches(&self) -> SyncSwitches {
        self.switches
    }

    /// Replaces the switches (the project's `sync.*` changed); the global switch stays.
    pub fn set_switches(&mut self, switches: SyncSwitches) {
        self.switches = SyncSwitches {
            enabled: self.switches.enabled,
            ..switches
        };
    }

    /// Sync on or off.
    pub fn set_enabled(&mut self, on: bool) {
        self.switches.enabled = on;
    }

    /// One kind on or off.
    pub fn set_kind(&mut self, kind: SyncKind, on: bool) {
        self.switches.set_kind(kind, on);
    }

    /// Leader-only submit on or off.
    pub fn set_leader_only_submit(&mut self, on: bool) {
        self.switches.leader_only_submit = on;
    }

    /// Who besides the leader submits a form (the session hook of RB-020-07).
    pub fn set_submit_policy(&mut self, policy: Arc<dyn SubmitPolicy>) {
        self.policy = policy;
    }

    /// The views whose navigation is decoupled (RB-020-06's `syncNavigation: false`): they do
    /// not follow a leader's submit.
    pub fn set_navigation_decoupled(&mut self, views: &[ViewId]) {
        self.navigation_decoupled = views.iter().copied().collect();
    }

    /// The views, in grid order; the state of views that are gone is forgotten.
    pub fn set_views(&mut self, views: &[ViewId]) {
        self.views = views.to_vec();
        self.excluded.retain(|view| views.contains(view));
        self.loading.retain(views);
        self.epochs.retain(|view, _| views.contains(view));
        self.desync.retain(views);
        if self.active.is_some_and(|active| !views.contains(&active)) {
            self.active = None;
        }
        if let Some(follow) = &mut self.follow {
            follow.followers.retain(|view| views.contains(view));
        }
    }

    /// The leader of the open follow: a submit it routed waits for its load.
    pub fn follow_leader(&self) -> Option<ViewId> {
        self.follow.as_ref().map(|follow| follow.leader)
    }

    /// The active view.
    pub fn active(&self) -> Option<ViewId> {
        self.active
    }

    /// `view` became the active view (the leader); `true` when that changed.
    pub fn set_active(&mut self, view: Option<ViewId>) -> bool {
        let changed = self.active != view;
        self.active = view;
        changed
    }

    /// Excludes `view` from sync or takes it back; `Ok(true)` when that changed.
    pub fn exclude(&mut self, view: ViewId, excluded: bool) -> Result<bool, HubError> {
        self.known(view)?;
        Ok(if excluded {
            self.excluded.insert(view)
        } else {
            self.excluded.remove(&view)
        })
    }

    /// Whether `view` is excluded.
    pub fn is_excluded(&self, view: ViewId) -> bool {
        self.excluded.contains(&view)
    }

    /// `Leader S N`: the next click is not synced.
    pub fn skip_next_click(&mut self) {
        self.skip_click = true;
    }

    /// Whether `Leader S N` is armed.
    pub fn skip_next_armed(&self) -> bool {
        self.skip_click
    }

    /// Whether `view`'s agent captures: sync on, the active view, not excluded.
    pub fn captures(&self, view: ViewId) -> bool {
        self.switches.enabled && self.active == Some(view) && !self.excluded.contains(&view)
    }

    /// Where the event `message` of `source` goes. A leader-only submit starts a follow.
    pub fn route(&mut self, source: ViewId, message: &EventMessage, now: Instant) -> Route {
        let event = &message.event;
        let kind = SyncKind::of(event);
        if !self.switches.enabled {
            return Route::Skip(SkipReason::Disabled);
        }
        if self.active != Some(source) {
            return Route::Skip(SkipReason::NotActive);
        }
        if self.excluded.contains(&source) {
            return Route::Skip(SkipReason::Excluded);
        }
        if self.skip_click && is_escape(event) {
            // Escape takes `Leader S N` back; the key itself goes on as any other.
            self.skip_click = false;
        }
        if !self.switches.kind(kind) {
            return Route::Skip(SkipReason::KindOff(kind));
        }
        if self.skip_click && kind == SyncKind::Click {
            if !matches!(event, SyncEvent::Pointer(_)) {
                self.skip_click = false;
            }
            return Route::Skip(SkipReason::SkippedClick);
        }
        self.loading.expire(now);
        let targets: Vec<ViewId> = self
            .views
            .iter()
            .copied()
            .filter(|view| {
                *view != source
                    && !self.excluded.contains(view)
                    && !self.loading.is_loading(*view, now)
                    && !self.desync.is_desynced(*view)
            })
            .collect();
        if targets.is_empty() {
            return Route::Skip(SkipReason::NoTargets);
        }
        let leader_only = self.switches.leader_only_submit;
        let submitters = if leader_only {
            self.policy.submitters(source, &targets)
        } else {
            Vec::new()
        };
        if matches!(event, SyncEvent::Submit(_)) {
            return self.submit(source, &targets, &submitters, now);
        }
        // One copy of the payload for every target (RB-080-27).
        let event = Arc::new(event.clone());
        let frame: Option<Arc<[String]>> = message.frame.as_deref().map(Arc::from);
        Route::Replay(
            targets
                .into_iter()
                .map(|target| ReplayRequest {
                    source,
                    target,
                    top: message.top,
                    frame: frame.clone(),
                    event: event.clone(),
                    block_submit: leader_only && !submitters.contains(&target),
                })
                .collect(),
        )
    }

    fn submit(
        &mut self,
        leader: ViewId,
        targets: &[ViewId],
        submitters: &[ViewId],
        now: Instant,
    ) -> Route {
        if !self.switches.leader_only_submit {
            return Route::Skip(SkipReason::SubmitFollowsInput);
        }
        if !self.switches.navigation {
            return Route::Skip(SkipReason::KindOff(SyncKind::Navigation));
        }
        let followers: Vec<ViewId> = targets
            .iter()
            .copied()
            .filter(|view| !submitters.contains(view) && !self.navigation_decoupled.contains(view))
            .collect();
        if followers.is_empty() {
            return Route::Skip(SkipReason::NoTargets);
        }
        self.follow = Some(Follow::new(leader, followers.clone(), now));
        Route::Follow { leader, followers }
    }

    /// The outcome of a replay into `view` routed in its document `epoch` ([`Hub::epoch`]);
    /// `true` when the view became desynced. The outcome of a document the view has left (it
    /// navigated or was resynced since) is dropped.
    pub fn record_at(
        &mut self,
        view: ViewId,
        epoch: u64,
        event: &SyncEvent,
        outcome: &ReplayOutcome,
    ) -> bool {
        self.epoch(view) == epoch && self.record(view, event, outcome)
    }

    /// `view`'s document: it counts up with every committed navigation and resync.
    pub fn epoch(&self, view: ViewId) -> u64 {
        self.epochs.get(&view).copied().unwrap_or_default()
    }

    fn next_epoch(&mut self, view: ViewId) {
        *self.epochs.entry(view).or_default() += 1;
    }

    /// The outcome of a replay into `view`; `true` when the view became desynced.
    pub fn record(&mut self, view: ViewId, event: &SyncEvent, outcome: &ReplayOutcome) -> bool {
        if !marks_desync(event) || !self.views.contains(&view) {
            return false;
        }
        let reason = match outcome {
            ReplayOutcome::Done | ReplayOutcome::Unsupported => return false,
            ReplayOutcome::NotFound => DesyncReason::NotFound,
            ReplayOutcome::NotVisible => DesyncReason::NotVisible,
            ReplayOutcome::Timeout => DesyncReason::Timeout,
            ReplayOutcome::Failed(_) => DesyncReason::Failed,
        };
        self.desync.mark(view, reason, event)
    }

    /// `view` committed a navigation to `url`: it is in sync again, and when it is the leader
    /// of an open follow, the followers load `url`.
    /// The time from the load the hub gave to this commit is the counter `sync.navigation`
    /// (RB-090-32).
    pub fn navigated(&mut self, view: ViewId, url: &str, now: Instant) -> Vec<Navigation> {
        if let Some(given) = self.loading.given_at(view, now) {
            rb_util::perf::record(
                rb_util::perf::names::SYNC_NAVIGATION,
                now.saturating_duration_since(given),
            );
        }
        self.loading.clear(view);
        self.desync.clear(view);
        self.next_epoch(view);
        self.follow_up(view, url, now)
    }

    /// The host says `view` is `loading` (or not), has seen `ended` loads end and shows `url`.
    /// A load the hub gave is over once a load ended after it; and the end of a leader's load
    /// after a leader-only submit that committed no new URL (a form posted to its own page) has
    /// the followers load `url` - the navigation they wait for never comes.
    pub fn load_state(
        &mut self,
        view: ViewId,
        (loading, ended): (bool, u64),
        url: &str,
        now: Instant,
    ) -> Vec<Navigation> {
        match self.loading.host_state(view, loading, ended) {
            Some(LoadChange::Finished) if !url.is_empty() => self.follow_up(view, url, now),
            _ => Vec::new(),
        }
    }

    /// When `view` leads an open follow, its followers load `url` (and get no events until
    /// they do).
    fn follow_up(&mut self, view: ViewId, url: &str, now: Instant) -> Vec<Navigation> {
        if self
            .follow
            .as_ref()
            .is_some_and(|follow| follow.expired(now))
        {
            self.follow = None;
        }
        let Some(follow) = self.follow.take_if(|follow| follow.is_due(view, now)) else {
            return Vec::new();
        };
        follow
            .followers
            .into_iter()
            .filter(|follower| self.views.contains(follower) && !self.excluded.contains(follower))
            .map(|follower| {
                self.loading.mark(follower, now);
                Navigation {
                    view: follower,
                    url: url.to_owned(),
                }
            })
            .collect()
    }

    /// `view` is about to load a URL navigation sync gave it (RB-020-06): no events until it
    /// commits or its load ends. Mark before the navigation starts, [`Hub::load_failed`] when it
    /// does not.
    pub fn mark_loading(&mut self, view: ViewId, now: Instant) {
        if self.views.contains(&view) {
            self.loading.mark(view, now);
        }
    }

    /// The navigation the hub or navigation sync gave `view` did not start: it takes events
    /// again.
    pub fn load_failed(&mut self, view: ViewId) {
        self.loading.clear(view);
    }

    /// Whether `view` is loading a URL the hub or navigation sync gave it at `now`.
    pub fn is_loading(&self, view: ViewId, now: Instant) -> bool {
        self.loading.is_loading(view, now)
    }

    /// Resync `view` at `now`: its badge goes, and it loads the leader's URL -
    /// `Ok(Some(leader))`, marked as loading until [`Hub::load_failed`] or its load ends;
    /// `Ok(None)` when it is the leader or there is none.
    pub fn resync(&mut self, view: ViewId, now: Instant) -> Result<Option<ViewId>, HubError> {
        self.known(view)?;
        self.desync.clear(view);
        self.next_epoch(view);
        match self.active {
            Some(leader) if leader != view => {
                self.loading.mark(view, now);
                Ok(Some(leader))
            }
            _ => Ok(None),
        }
    }

    /// The desynced views, by id.
    pub fn desynced(&self) -> Vec<ViewId> {
        self.desync
            .list()
            .iter()
            .map(|desync| desync.view)
            .collect()
    }

    /// What the shell and MCP show.
    pub fn status(&self) -> HubStatus {
        HubStatus {
            switches: self.switches,
            excluded: self.excluded.iter().copied().collect(),
            desynced: self.desync.list(),
            skip_next_click: self.skip_click,
        }
    }

    fn known(&self, view: ViewId) -> Result<(), HubError> {
        if self.views.contains(&view) {
            Ok(())
        } else {
            Err(HubError::UnknownView(view))
        }
    }
}

/// Whether `event` is Escape going down.
fn is_escape(event: &SyncEvent) -> bool {
    matches!(event, SyncEvent::Keydown(key) if key.key == "Escape")
}

#[cfg(test)]
#[path = "hub_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "hub_loading_tests.rs"]
mod loading_tests;

#[cfg(test)]
#[path = "hub_sharing_tests.rs"]
mod sharing_tests;

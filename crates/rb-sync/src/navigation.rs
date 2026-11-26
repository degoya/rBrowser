//! Navigation sync as a fallback (RB-020-06), without a runtime: [`NavigationSync`] decides
//! when the views are checked against the leader's page and how each one that is elsewhere
//! follows. The app runs the timer and the navigations (`crates/rbrowser/src/nav_sync/`).
//!
//! The rules (`docs/sync-protocol.md`, Navigation sync):
//!
//! - **Fallback.** Input sync usually takes the followers along: the replayed click on a link
//!   navigates them too. A committed navigation of the leader (the active view, synced, not
//!   excluded, its navigation coupled) arms a check after `sync.navigationSettleMs`; every
//!   coupled follower that then shows another page ([`crate::nav_url::same_page`], the hash
//!   per `sync.navigationIncludeHash`) loads the leader's URL - in its own environment when the
//!   two are in different ones ([`crate::nav_url::follow_url`]).
//! - **Debounce.** A further navigation of the leader before the check (a redirect, a SPA
//!   route that replaces itself) re-arms it and keeps the page the leader came from.
//! - **Loading views wait.** A follower that is still loading gets up to [`SETTLE_ROUNDS`]
//!   checks before it is loaded; a follower is loaded at most once per check.
//! - **SPA routes.** When the leader's page changed within its document (`pushState`,
//!   `replaceState`, the hash; CDP `Page.navigatedWithinDocument`) and a follower still shows
//!   the page the leader came from, the follower moves through the history API in its own
//!   document ([`FollowMode::History`]): no reload, the page's state stays.
//! - **No loops.** Only the leader's own navigations arm a check; what a follower commits
//!   (because it followed) never does. Only http(s) and file pages are followed.
//! - **Decoupled views** (`breakpoints[].syncNavigation: false`, `Leader S U`, `sync_set`)
//!   neither lead nor follow; a runtime change holds until the project config changes.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Duration;

use rb_engine::ViewId;

use crate::nav_url::{follow_url, followable, same_origin, same_page};

/// The default of `sync.navigationSettleMs`.
pub const DEFAULT_SETTLE: Duration = Duration::from_millis(1000);
/// How many checks a loading follower gets before it is loaded anyway.
pub const SETTLE_ROUNDS: u32 = 3;

/// What navigation sync follows (`sync.navigation*` and the project's environments).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavOptions {
    /// `sync.navigationIncludeHash`: a different fragment is a different page.
    pub include_hash: bool,
    /// `sync.navigationSettleMs`: how long input sync has to take the followers along.
    pub settle: Duration,
    /// The base URLs of the project's environments.
    pub environments: Vec<String>,
}

impl Default for NavOptions {
    fn default() -> Self {
        Self {
            include_hash: false,
            settle: DEFAULT_SETTLE,
            environments: Vec::new(),
        }
    }
}

/// How a follower moves to the leader's page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowMode {
    /// It loads the URL.
    Load,
    /// It changes its route through the history API in its own document (the agent's
    /// `navigate` command): no reload.
    History,
}

/// One follower to move.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavFollow {
    /// The view.
    pub view: ViewId,
    /// Where to.
    pub url: String,
    /// How.
    pub mode: FollowMode,
}

/// A check to run after `delay`; a check whose generation is no longer the current one is
/// stale and does nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settle {
    /// The check.
    pub generation: u64,
    /// When, from now.
    pub delay: Duration,
}

/// A view the check looks at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Follower {
    /// The view.
    pub view: ViewId,
    /// Its URL now.
    pub url: String,
    /// Whether it is still loading (a navigation of its own, or one the hub gave it).
    pub loading: bool,
}

/// What a check found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SettleOutcome {
    /// The followers to move now.
    pub follows: Vec<NavFollow>,
    /// Another round for the followers still loading.
    pub retry: Option<Settle>,
}

/// An armed check.
#[derive(Clone, Debug)]
struct Pending {
    leader: ViewId,
    /// The leader's page before the navigation (or the first of a burst).
    from: Option<String>,
    generation: u64,
    round: u32,
    /// The followers this check moved already.
    handled: BTreeSet<ViewId>,
}

/// The navigation sync state of one window.
#[derive(Debug, Default)]
pub struct NavigationSync {
    options: NavOptions,
    /// Decoupled by the project (`syncNavigation: false`).
    configured: BTreeSet<ViewId>,
    /// Runtime changes, `true` for coupled; until the project config changes.
    overrides: BTreeMap<ViewId, bool>,
    /// Each view's last committed URL.
    urls: HashMap<ViewId, String>,
    /// Each view's last URL changed within its document (CDP).
    within: HashMap<ViewId, String>,
    pending: Option<Pending>,
    generation: u64,
}

impl NavigationSync {
    /// No views yet, with `options`.
    pub fn new(options: NavOptions) -> Self {
        Self {
            options,
            ..Self::default()
        }
    }

    /// The options in effect.
    pub fn options(&self) -> &NavOptions {
        &self.options
    }

    /// New options (the project's `sync.*` changed).
    pub fn set_options(&mut self, options: NavOptions) {
        self.options = options;
    }

    /// The views the project decouples (`syncNavigation: false`).
    pub fn set_configured(&mut self, decoupled: impl IntoIterator<Item = ViewId>) {
        self.configured = decoupled.into_iter().collect();
    }

    /// Drops the runtime changes (the project config changed).
    pub fn clear_overrides(&mut self) {
        self.overrides.clear();
    }

    /// Couples `view`'s navigation or decouples it; `true` when that changed.
    pub fn couple(&mut self, view: ViewId, coupled: bool) -> bool {
        let before = self.is_coupled(view);
        if coupled == !self.configured.contains(&view) {
            self.overrides.remove(&view);
        } else {
            self.overrides.insert(view, coupled);
        }
        before != coupled
    }

    /// Whether `view` leads and follows navigations.
    pub fn is_coupled(&self, view: ViewId) -> bool {
        self.overrides
            .get(&view)
            .copied()
            .unwrap_or_else(|| !self.configured.contains(&view))
    }

    /// The decoupled views of `views`, in their order.
    pub fn decoupled(&self, views: &[ViewId]) -> Vec<ViewId> {
        views
            .iter()
            .copied()
            .filter(|view| !self.is_coupled(*view))
            .collect()
    }

    /// The views now; what belongs to views that are gone is forgotten.
    pub fn retain(&mut self, views: &[ViewId]) {
        self.configured.retain(|view| views.contains(view));
        self.overrides.retain(|view, _| views.contains(view));
        self.urls.retain(|view, _| views.contains(view));
        self.within.retain(|view, _| views.contains(view));
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| !views.contains(&pending.leader))
        {
            self.pending = None;
        }
    }

    /// `view` committed `url`. `leads`: it is the active view, sync and its navigation kind
    /// are on and it is not excluded - then, when it is coupled and the page is followed, its
    /// navigation arms (or re-arms) the check.
    pub fn committed(&mut self, view: ViewId, url: &str, leads: bool) -> Option<Settle> {
        let previous = self.urls.insert(view, url.to_owned());
        if !leads || !self.is_coupled(view) {
            return None;
        }
        if !followable(url) {
            if self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.leader == view)
            {
                self.pending = None;
            }
            return None;
        }
        self.generation += 1;
        let from = match self.pending.take() {
            Some(pending) if pending.leader == view => pending.from,
            _ => previous,
        };
        self.pending = Some(Pending {
            leader: view,
            from,
            generation: self.generation,
            round: 0,
            handled: BTreeSet::new(),
        });
        Some(Settle {
            generation: self.generation,
            delay: self.options.settle,
        })
    }

    /// `view`'s top frame changed its URL within the document (CDP
    /// `Page.navigatedWithinDocument`).
    pub fn within_document(&mut self, view: ViewId, url: &str) {
        self.within.insert(view, url.to_owned());
    }

    /// `view`'s top frame loaded a new document (CDP `Page.frameNavigated`).
    pub fn cross_document(&mut self, view: ViewId) {
        self.within.remove(&view);
    }

    /// The leader of the armed check.
    pub fn pending_leader(&self) -> Option<ViewId> {
        self.pending.as_ref().map(|pending| pending.leader)
    }

    /// The last URL `view` committed.
    pub fn url(&self, view: ViewId) -> Option<&str> {
        self.urls.get(&view).map(String::as_str)
    }

    /// How many checks were armed so far: the generation of the last one.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Drops the armed check (sync off, the active view changed).
    pub fn cancel(&mut self) {
        self.pending = None;
    }

    /// The check `generation` is due: `leader` (the active view now) shows `leader_url`, the
    /// other views are `followers` (excluded views left out by the caller).
    pub fn settle(
        &mut self,
        generation: u64,
        leader: ViewId,
        leader_url: &str,
        followers: &[Follower],
    ) -> SettleOutcome {
        let Some(mut pending) = self
            .pending
            .take_if(|pending| pending.generation == generation)
        else {
            return SettleOutcome::default();
        };
        if pending.leader != leader || !followable(leader_url) || !self.is_coupled(leader) {
            return SettleOutcome::default();
        }
        let same_document = self
            .within
            .get(&leader)
            .is_some_and(|url| same_page(url, leader_url, true));
        let last_round = pending.round + 1 >= SETTLE_ROUNDS;
        let (mut follows, mut waiting) = (Vec::new(), false);
        for follower in followers {
            let view = follower.view;
            if view == leader || !self.is_coupled(view) || pending.handled.contains(&view) {
                continue;
            }
            let environments = &self.options.environments;
            let target = follow_url(leader_url, &follower.url, environments);
            if same_page(&follower.url, &target, self.options.include_hash) {
                continue;
            }
            if follower.loading && !last_round {
                waiting = true;
                continue;
            }
            let on_previous_page = pending.from.as_deref().is_some_and(|from| {
                same_page(
                    &follower.url,
                    &follow_url(from, &follower.url, environments),
                    false,
                )
            });
            let mode = if same_document && on_previous_page && same_origin(&follower.url, &target) {
                FollowMode::History
            } else {
                FollowMode::Load
            };
            pending.handled.insert(view);
            follows.push(NavFollow {
                view,
                url: target,
                mode,
            });
        }
        let retry = waiting.then(|| {
            pending.round += 1;
            let settle = Settle {
                generation,
                delay: self.options.settle,
            };
            self.pending = Some(pending);
            settle
        });
        SettleOutcome { follows, retry }
    }
}

#[cfg(test)]
#[path = "navigation_tests.rs"]
mod tests;

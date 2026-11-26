//! The page clock and the animation rates in every view (RB-094-07): date-dependent pages
//! (calendars, countdowns, "3 days ago") and animations alike in every breakpoint, reproducible
//! for screenshots and visual diffs.
//!
//! - **The clock** is one for every view, so the sync stays alike: a fixed time or one that runs
//!   from a start at real or faster speed. It is a `Date` and `Intl.DateTimeFormat` override in
//!   each page's main world before the page's scripts (`clock.js`), not CDP's virtual time,
//!   which would stop or race the page's timers: the timers, `performance.now()` and
//!   `requestAnimationFrame` stay real. A start without a zone is each page's own local time,
//!   its emulated time zone (RB-080-10) included.
//! - **The animations** run at a rate per view - stopped, 10 %, 25 %, 100 % - on the document
//!   timeline (`Animation.setPlaybackRate`: CSS animations and transitions, Web Animations);
//!   stopped ones move on a step at a time (`step.js`). A capture freezes and puts the rate
//!   back (RB-080-12, `crate::capture::clean`): a stopped view stays stopped after it.
//! - Firefox and WebKit views keep their time and animations; the commands name them as
//!   `unsupported`.
//!
//! | Part | Module |
//! | --- | --- |
//! | The clock, the requests, the rates, the activity | [`model`] |
//! | The page scripts and CDP parameters | [`script`] |
//! | The clock and rates per view over CDP | [`hooks`] |
//! | The commands `emulation.clock` and `overlays.animations` | [`command`] |
//! | Their definitions | [`defs`] |
//!
//! The host's attach hands each new page session over ([`attach`]); the activity bar shows what
//! is changed (`timeControl` in the snapshot, [`publish`]); the page's context menu offers both
//! under Emulation (`crate::context_menu::time_control`).

pub mod command;
pub mod defs;
pub mod hooks;
pub mod model;
pub mod script;

use std::sync::{Arc, LazyLock};

use rb_engine::ViewHost as _;
use rb_engine::{ViewEvent, ViewId};
use tokio::sync::broadcast::error::RecvError;

pub use defs::{ANIMATIONS, ANIMATIONS_TOOL, CLOCK, CLOCK_TOOL, definitions, register};
pub use hooks::TimeHooks;

use crate::cdp::CdpRouter;
use crate::control::Core;
use crate::host::Host;

/// The clock and the rates of the window's views. One window per process.
static HOOKS: LazyLock<TimeHooks> = LazyLock::new(TimeHooks::default);

/// The window's clock and rates.
pub fn hooks() -> &'static TimeHooks {
    &HOOKS
}

/// `view` has a new page `session` (the host's attach): its clock and rate go in before its
/// first document.
pub async fn attach(cdp: &CdpRouter, view: ViewId, session: &str) {
    HOOKS.attach(cdp, view, session).await;
}

/// The animation rate held for the view of page `session`, unless it is normal: what a
/// capture's undo puts back where the page answers 0.
pub fn held_rate(session: &str) -> Option<f64> {
    HOOKS.held_rate(session)
}

/// Hands the shell what is changed: the activity bar's entry.
pub fn publish(host: &Host) {
    let activity = model::activity(HOOKS.clock().as_ref(), &HOOKS.rates());
    host.update_shell_state(|shell| shell.time_control = activity);
}

/// Follows the window's views: a closed one is forgotten.
pub fn on_start(core: &Core) {
    let host = core.host();
    let mut events = host.subscribe();
    let weak = Arc::downgrade(host);
    host.runtime().spawn(async move {
        loop {
            let event = events.recv().await;
            let Some(host) = weak.upgrade() else {
                return;
            };
            match event {
                Ok(ViewEvent::Closed(view)) => {
                    HOOKS.forget(view);
                    publish(&host);
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => {
                    if reconcile(&host, &HOOKS) {
                        publish(&host);
                    }
                }
                Err(RecvError::Closed) => return,
            }
        }
    });
}

/// The follower fell behind the host's events and may have missed a `Closed`: the views the
/// host has now decide which `hooks` keeps (RB-099-19); `true` when one was forgotten.
fn reconcile(host: &Host, hooks: &TimeHooks) -> bool {
    // Asked per view under the hooks' lock, not from a list taken before it (review w39).
    hooks.forget_gone(|view| host.view(view).is_some())
}

#[cfg(test)]
#[path = "follow_tests.rs"]
mod tests;

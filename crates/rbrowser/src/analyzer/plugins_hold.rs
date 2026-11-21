//! The first choice holds the plugins' load (RB-091-08, variant A - the owner's decision): at the
//! very first start, while the first-run wizard's plugin step has not answered the choice of the
//! bundled plugins (installed, none, skipped, or the wizard closed), the one load of the process
//! waits. Until then nothing loads and nothing keeps an empty load for the process
//! ([`super::bundled`] gives no plugin without keeping it); the window's plugin runtime waits for
//! the answer ([`super::bundled_when_answered`]) and then registers commands, panels, page
//! scripts and MCP tools as at a normal start. `crate::plugins::bundle::hold_for_wizard` holds
//! it; an answer of the plugin step or the wizard's window closing releases it. Without the
//! wizard nothing ever holds and the start is as before.

use std::sync::{Condvar, Mutex, PoisonError};

/// What the gate knows, under one lock - so a load and a hold never both win.
#[derive(Debug, Default)]
struct State {
    /// The load waits for the first choice.
    held: bool,
    /// The load began (or ran): too late to hold it.
    started: bool,
    /// The wizard's window never opened or the app quits: nothing loads any more.
    abandoned: bool,
}

/// A gate that is open unless held.
#[derive(Debug, Default)]
pub(super) struct Hold {
    state: Mutex<State>,
    opened: Condvar,
}

impl Hold {
    /// Open.
    pub(super) const fn new() -> Self {
        Self {
            state: Mutex::new(State {
                held: false,
                started: false,
                abandoned: false,
            }),
            opened: Condvar::new(),
        }
    }

    /// Closes the gate; refused (`false`) once a load began or the gate was abandoned - a load
    /// under way already read the choice, so the wizard's answer cannot be live (review of
    /// RB-091-08).
    pub(super) fn try_hold(&self) -> bool {
        let mut state = rb_util::lock(&self.state);
        if state.started || state.abandoned {
            return false;
        }
        state.held = true;
        true
    }

    /// A load may begin now (`true`, and it is marked begun): not while held, not abandoned.
    pub(super) fn begin_load(&self) -> bool {
        let mut state = rb_util::lock(&self.state);
        if state.held || state.abandoned {
            return false;
        }
        state.started = true;
        true
    }

    /// Opens the gate and wakes every waiter; whether it was held - then the load comes now.
    pub(super) fn release(&self) -> bool {
        let was = std::mem::replace(&mut rb_util::lock(&self.state).held, false);
        self.opened.notify_all();
        was
    }

    /// Wakes every waiter for good, and no load begins any more.
    pub(super) fn abandon(&self) {
        {
            let mut state = rb_util::lock(&self.state);
            state.held = false;
            state.abandoned = true;
        }
        self.opened.notify_all();
    }

    /// Whether the gate is held.
    pub(super) fn is_held(&self) -> bool {
        rb_util::lock(&self.state).held
    }

    /// Waits until the gate is open; at once when it is. A blocking thread only.
    pub(super) fn wait(&self) {
        let mut state = rb_util::lock(&self.state);
        while state.held {
            state = self
                .opened
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

/// The process's gate.
static HOLD: Hold = Hold::new();

/// Holds the plugins' load until [`release`]; `false` when a load began already (then nothing
/// holds and the wizard's answer loads at the next start).
pub fn hold() -> bool {
    let held = HOLD.try_hold();
    if held {
        tracing::info!("plugins: the load waits for the first-run wizard's plugin choice");
    } else {
        tracing::info!(
            "plugins: the load began already; the wizard's choice loads at the next start"
        );
    }
    held
}

/// Lets the plugins load; whether the load was held - then it starts now, live.
pub fn release() -> bool {
    let was = HOLD.release();
    if was {
        tracing::info!("plugins: the plugin choice is answered; the plugins load now");
    }
    was
}

/// The wizard's window could not open, or the app quits: no thread waits for an answer any more,
/// and no load starts (at a quit) - a held choice stays pending, so the next start applies the
/// recommended set.
pub fn abandon() {
    HOLD.abandon();
}

/// Whether the load waits for the first choice.
pub fn held() -> bool {
    HOLD.is_held()
}

/// Marks the load begun when it may begin (see [`Hold::begin_load`]).
pub(super) fn begin_load() -> bool {
    HOLD.begin_load()
}

/// Waits until the load may run (a blocking thread only).
pub(super) fn wait_released() {
    HOLD.wait();
}

#[cfg(test)]
#[path = "plugins_hold_tests.rs"]
mod tests;

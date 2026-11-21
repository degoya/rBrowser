//! "Force state" (RB-090-53): pseudo-class states - `:hover`, `:active`, `:focus`,
//! `:focus-visible`, `:focus-within`, `:visited`, `:target` - forced on a page's element with
//! CDP's `CSS.forcePseudoState`, so a hover menu stays open in every view while the mouse is
//! elsewhere (a ruler dragged into it, a picker, a screenshot), until "Reset states" or a
//! navigation. `:hover`, `:active` and `:focus-within` reach every ancestor up to `<html>`, as a
//! real hover does - otherwise `li:hover > ul` would not open.
//!
//! Who forces: the page menu's "Force state" (this view, all views; `context_menu`), the
//! commands `view.force_state.set` and `view.force_state.reset` (`Leader D H`, `Leader D Q`) and
//! MCP `views_force_state`. While a view has a forced state, the activity bar shows the states
//! and the views with Reset (`forcedStates` in the snapshot, [`publish`], RB-094-16). A menu
//! that opens only through JavaScript (`mouseenter`) does not react to a forced `:hover`; the
//! job file says why rBrowser holds no hover of its own.
//!
//! | Part | Module |
//! | --- | --- |
//! | The states, each view's forced elements and the nodes they reach | [`model`] |
//! | The CDP calls, on a session of rBrowser's own per view | [`cdp`] |
//! | Forcing and resetting over the views, free of the window | [`apply`] |
//! | The commands' arguments, the window's views, the activity bar and the status bar | [`command`] |
//! | The command definitions | [`defs`] |
//! | Navigation and closed views end what was forced | [`watch`] |

pub mod apply;
pub mod cdp;
pub mod command;
pub mod defs;
pub mod model;
#[cfg(test)]
mod testing;
pub mod watch;

use std::sync::{LazyLock, Mutex};

use rb_engine::ViewId;
use rb_ipc::ViewForcedStates;

pub use defs::{RESET, SET, TOOL, definitions, register};
pub use model::{ForcedMarks, PseudoClass, Registry};

use crate::cdp::lock;
use crate::host::Host;

/// The forced elements of the window's views. One window per process.
static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(|| Mutex::new(Registry::default()));

/// One request at a time: a pick waits for a click, and the registry's sessions are opened once.
static OPERATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn registry() -> &'static Mutex<Registry> {
    &REGISTRY
}

/// The page menu's marks for `view` among `views` (every view of the window).
#[must_use]
pub fn marks(view: ViewId, views: &[ViewId]) -> ForcedMarks {
    lock(registry()).marks(view, views)
}

/// Hands the shell what is forced now: the activity bar's entry (RB-094-16). Whether an entry
/// was shown before, and whether one is now.
pub fn publish(host: &Host) -> (bool, bool) {
    let shown = lock(registry()).activity();
    let now = !shown.is_empty();
    (show(host, shown), now)
}

/// Puts `shown` into the shell's state; whether it held forced states before.
fn show(host: &Host, shown: Vec<ViewForcedStates>) -> bool {
    let mut had = false;
    host.update_shell_state(|shell| {
        had = !shell.forced_states.is_empty();
        shell.forced_states = shown;
    });
    had
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

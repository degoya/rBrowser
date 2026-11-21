//! The consent probe (RB-094-09): for a plugin granted `consent-probe` (the consent check,
//! RB-094-05) the core loads a page in a fresh, empty session of its own, captures what the page
//! does before any consent and after "Reject all", discards the session and hands the plugin the
//! capture as the argument `$consent` - as it hands `$console` to a plugin granted
//! `console-logs`. A plugin cannot do that itself: it can neither open a session nor navigate nor
//! wait, and reads only its view's own cookies.
//!
//! | Part | Where |
//! | --- | --- |
//! | Public addresses or the project's own sites, else the person's setting | [`address`] |
//! | Caller's `$consent` dropped; the offer, the request, the second call | [`args`] |
//! | The capture's shape, values stripped, lists bounded | [`shape`] |
//! | One probe: open, load, capture, reject, capture, discard | [`run`] |
//! | The call: the plugin asks, the host probes, the plugin is called again | [`call`] |
//! | The windowless browser in a fresh request context, over CDP | [`cdp`] |
//! | The window's prober: one probe at a time, fixed failure sentences | [`prober`] |
//!
//! The views' sessions are never touched: the probe's browser lives in a request context made
//! for it alone (in memory, never cached, `sessions::cef::fresh_request_context`) and is
//! attached only when CDP shows its browser context shared with no other target. Only `http`
//! and `https` addresses are loaded. Cookie and storage values are never read into the capture;
//! logs name a failure by a fixed sentence, never the address or what the page set.

pub mod address;
pub mod args;
pub mod call;
pub mod cdp;
pub mod prober;
pub mod run;
pub mod shape;

use std::sync::{Arc, Mutex};

use crate::cdp::lock;
use crate::control::Core;

pub use args::{ASK_KEY, PROBE_KEY, ProbeRequest, offered};
pub use call::{Call, Prober};

/// The render view session of the probe's browser: a request context of its own, in memory
/// (`cef_app/render_views.rs`). No config session can be named so
/// (`rb_config::is_valid_session_id`).
pub const PROBE_SESSION: &str = "$consent-probe";

static PROBER: Mutex<Option<Arc<dyn Prober>>> = Mutex::new(None);

/// Starts the probes of `core`'s window (once a plugin granted `consent-probe` is loaded).
pub fn start(core: &Core) {
    let Some(core) = core.arc() else {
        return;
    };
    let prober: Arc<dyn Prober> = Arc::new(prober::HostProber::new(&core));
    *lock(&PROBER) = Some(prober);
}

/// Whether the prober starts now: a plugin `wanted` it (granted `consent-probe`) and none runs
/// yet - at the window's start or a live change (RB-092-06).
#[must_use]
pub fn should_start(wanted: bool, running: bool) -> bool {
    wanted && !running
}

/// Starts the prober when one of `plugins` is granted `consent-probe` and none runs yet.
pub fn follow(core: &Core, plugins: &[super::runtime::Plugin]) {
    let wanted = plugins
        .iter()
        .any(|plugin| plugin.component.consent_probe());
    if should_start(wanted, prober().is_some()) {
        start(core);
    }
}

/// The running window's prober; none before [`start`].
#[must_use]
pub fn prober() -> Option<Arc<dyn Prober>> {
    lock(&PROBER).clone()
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

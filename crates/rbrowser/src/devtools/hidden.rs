//! The targets no outer CDP client may see (ADR 0001, Decision 4), the app's one
//! [`rb_cdp::HiddenTargets`]: every page of the app's own scheme (`rbrowser://ui/`: the shell
//! UI and the settings window - a grid view may never navigate there), every DevTools page
//! (`devtools://`, the docked frontend and the native DevTools window), every page and worker
//! of a Chrome extension (`chrome-extension://`, RB-080-06), and the targets created
//! with a marker URL - the DevTools dock ([`DOCK_MARKER_URL`]), the analyser's hidden page
//! ([`ANALYZER_MARKER_URL`]) and the sitemap's render views (every URL that starts with
//! [`crate::sitemap::backend::MARKER_PREFIX`]) - which are registered here by their id as soon
//! as it is known.
//! The external CDP proxy (RB-030-06) filters `/json/list`, `Target.getTargets`, the `Target.*`
//! events and every `targetId` with [`filter`]; the internal DevTools endpoint filters the
//! events it relays the same way.

use rb_cdp::{HiddenTargets, TargetFilter as _};
use serde_json::Value;

/// The URL the dock starts with until its target is captured and prepared.
pub const DOCK_MARKER_URL: &str = "about:blank#rb-devtools-dock";
/// The URL the analyser's hidden target starts with until its id is registered.
pub const ANALYZER_MARKER_URL: &str = "about:blank#rb-analyzer";

static HIDDEN: HiddenTargets = HiddenTargets::with_prefixes(
    &["devtools:", "rbrowser:", "chrome-extension:"],
    &[DOCK_MARKER_URL, ANALYZER_MARKER_URL],
    &[crate::sitemap::backend::MARKER_PREFIX],
);

/// The app's registry of hidden targets.
pub fn filter() -> &'static HiddenTargets {
    &HIDDEN
}

/// Hides the target `target_id` from every outer client.
pub fn register(target_id: &str) {
    HIDDEN.register(target_id);
}

/// The target `target_id` is closed; its id is not kept.
pub fn forget(target_id: &str) {
    HIDDEN.forget(target_id);
}

/// Whether the target `target_id` showing `url` is hidden: a registered id, a page of the
/// app's own scheme or of DevTools, or a marker URL.
pub fn is_hidden(target_id: &str, url: &str) -> bool {
    HIDDEN.is_hidden(target_id, url)
}

/// [`is_hidden`] for a CDP `TargetInfo` (`{ targetId, url, ... }`).
pub fn is_hidden_target(info: &Value) -> bool {
    rb_cdp::target_filter::hides_info(&HIDDEN, info)
}

#[cfg(test)]
#[path = "hidden_tests.rs"]
mod tests;

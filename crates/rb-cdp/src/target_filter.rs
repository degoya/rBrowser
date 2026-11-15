//! The targets no outer CDP client sees or reaches (ADR 0001 Decision 4, K10): the shell UI and
//! the settings window (both on the app's own scheme `rbrowser://`), every DevTools page
//! (`devtools://`), the DevTools dock and the analyser's hidden target (by the marker URL they
//! are created with, then by their id, registered as soon as it is known), and the numbered
//! targets such as the sitemap's render views (by their marker prefix, `about:blank#rb-render-`). A target is hidden
//! when any rule says so: an unknown or odd target fails closed only through its scheme, a
//! known internal one also when it shows something else.
//!
//! The proxy filters `/json/list`, `Target.getTargets`, every `Target.*` event and every
//! command that names a `targetId` with [`TargetFilter`]; the docked DevTools' own relay hides
//! the same targets from the frontend.

use std::collections::BTreeSet;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde_json::Value;

/// Which targets are hidden.
pub trait TargetFilter: Send + Sync {
    /// Whether the target `target_id` showing `url` is hidden. An empty `url` asks by id only.
    fn is_hidden(&self, target_id: &str, url: &str) -> bool;

    /// Whether the target of the CDP `TargetInfo` `info` is hidden: by default
    /// [`Self::is_hidden`] on its `targetId` and `url`. A filter that also judges by its `type`
    /// (a page, an iframe, a worker) overrides it; a target hidden this way is remembered by the
    /// session, so commands naming it are refused too.
    fn hides_info(&self, info: &Value) -> bool {
        let field = |name: &str| info.get(name).and_then(Value::as_str).unwrap_or_default();
        self.is_hidden(field("targetId"), field("url"))
    }
}

/// The app's registry of hidden targets: schemes and marker URLs fixed at build time, target
/// ids registered at run time.
#[derive(Debug)]
pub struct HiddenTargets {
    ids: Mutex<BTreeSet<String>>,
    schemes: &'static [&'static str],
    urls: &'static [&'static str],
    prefixes: &'static [&'static str],
}

impl HiddenTargets {
    /// A registry hiding every URL of `schemes` (with their colon, `rbrowser:`) and the exact
    /// `urls`, and no id yet.
    #[must_use]
    pub const fn new(schemes: &'static [&'static str], urls: &'static [&'static str]) -> Self {
        Self::with_prefixes(schemes, urls, &[])
    }

    /// [`HiddenTargets::new`] that also hides every URL starting with one of `prefixes`: the
    /// marker of a target that carries a number (`about:blank#rb-render-7`).
    #[must_use]
    pub const fn with_prefixes(
        schemes: &'static [&'static str],
        urls: &'static [&'static str],
        prefixes: &'static [&'static str],
    ) -> Self {
        Self {
            ids: Mutex::new(BTreeSet::new()),
            schemes,
            urls,
            prefixes,
        }
    }

    /// Hides the target `target_id` from now on, whatever it shows.
    pub fn register(&self, target_id: &str) {
        self.ids().insert(target_id.to_owned());
    }

    /// The target `target_id` is gone; its id is not kept.
    pub fn forget(&self, target_id: &str) {
        self.ids().remove(target_id);
    }

    fn ids(&self) -> MutexGuard<'_, BTreeSet<String>> {
        // A set of strings stays consistent whatever panicked while holding it.
        self.ids.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl TargetFilter for HiddenTargets {
    fn is_hidden(&self, target_id: &str, url: &str) -> bool {
        let scheme = self.schemes.iter().any(|scheme| {
            url.get(..scheme.len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(scheme))
        });
        scheme
            || self.urls.contains(&url)
            || self.prefixes.iter().any(|prefix| url.starts_with(prefix))
            || self.ids().contains(target_id)
    }
}

/// [`TargetFilter::hides_info`] for a CDP `TargetInfo` (`{ targetId, type, url, ... }`).
#[must_use]
pub fn hides_info(filter: &dyn TargetFilter, info: &Value) -> bool {
    filter.hides_info(info)
}

/// The `TargetInfo`s of `infos` no rule hides, in their order.
#[must_use]
pub fn visible_infos(filter: &dyn TargetFilter, infos: &[Value]) -> Vec<Value> {
    infos
        .iter()
        .filter(|info| !hides_info(filter, info))
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "target_filter_tests.rs"]
mod tests;

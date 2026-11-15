//! One CDP endpoint per view: a flow's `page` is the page of exactly its view, and Playwright
//! cannot tell targets apart, so each view of a run gets an internal CDP proxy of its own whose
//! filter hides every other page - the other views', a popup, a view added during the run - while
//! the frames and workers of its own page stay visible. The proxy takes a `&'static` filter: the
//! filters are a fixed set of slots, claimed for a run and freed once the view's proxy and its
//! last connection are gone (`super::endpoints`). A free slot hides everything.

use std::collections::BTreeSet;
use std::sync::Mutex;

use rb_cdp::TargetFilter;
use serde_json::Value;

use crate::cdp::lock;

/// The most views that run flows at once (all runs together).
pub const SLOTS: usize = 32;

/// What a claimed slot shows and hides.
#[derive(Debug)]
struct Shown {
    /// The view's page.
    page: String,
    /// Every other view's page when the run started.
    hidden: BTreeSet<String>,
}

/// What one slot lets through: one view's page with its frames and workers, never another page
/// - another view's, a popup or a view added while the run goes on.
#[derive(Debug)]
pub struct ViewScope {
    shown: Mutex<Option<Shown>>,
}

impl ViewScope {
    const fn free() -> Self {
        Self {
            shown: Mutex::new(None),
        }
    }
}

impl TargetFilter for ViewScope {
    fn is_hidden(&self, target_id: &str, url: &str) -> bool {
        crate::devtools::hidden::is_hidden(target_id, url)
            || lock(&self.shown)
                .as_ref()
                .is_none_or(|shown| shown.hidden.contains(target_id))
    }

    /// By id and URL, and every page but the view's own: Playwright takes the first page it
    /// sees as the flow's `page`.
    fn hides_info(&self, info: &Value) -> bool {
        let field = |name: &str| info.get(name).and_then(Value::as_str).unwrap_or_default();
        let (id, url) = (field("targetId"), field("url"));
        self.is_hidden(id, url)
            || (field("type") == "page"
                && lock(&self.shown)
                    .as_ref()
                    .is_none_or(|shown| shown.page != id))
    }
}

static SCOPES: [ViewScope; SLOTS] = [const { ViewScope::free() }; SLOTS];

/// A claimed slot; freed when dropped.
#[derive(Debug)]
pub struct Claimed {
    scope: &'static ViewScope,
}

impl Claimed {
    /// The filter of the view's proxy.
    pub fn filter(&self) -> &'static dyn TargetFilter {
        self.scope
    }
}

impl Drop for Claimed {
    fn drop(&mut self) {
        *lock(&self.scope.shown) = None;
    }
}

/// Claims a slot that shows `shown` and hides every target of `views` but it; `None` when all
/// slots are taken.
pub fn claim(shown: &str, views: &[String]) -> Option<Claimed> {
    let hidden: BTreeSet<String> = views
        .iter()
        .filter(|target| target.as_str() != shown)
        .cloned()
        .collect();
    SCOPES.iter().find_map(|scope| {
        let mut slot = lock(&scope.shown);
        if slot.is_some() {
            return None;
        }
        *slot = Some(Shown {
            page: shown.to_owned(),
            hidden: hidden.clone(),
        });
        Some(Claimed { scope })
    })
}

#[cfg(test)]
#[path = "scope_tests.rs"]
mod tests;

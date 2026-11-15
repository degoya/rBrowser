//! The footer's own analysis (`ui.viewFooter.analyze`, RB-080-47): once a view's page loads
//! are quiet for [`SETTLE`], the media queries of the document it shows are read from that
//! very document (`read.rs`: a script in an isolated world of the view) - nothing is requested
//! a second time. The analyser plugin turns them into boundaries, kept per document in the
//! host (`results.rs`, `crate::host::footer`), beside the result of Analyze Breakpoints: that
//! one is neither touched nor kept from running.
//!
//! Each view waits for itself. A committed address and every load that ends start a read, so a
//! reload (a dev server that came up, changed CSS) reaches the footer; a read whose content is
//! the one kept already ends there. Quiet: a failure is logged, never a notice. The setting is
//! read when the wait ends; switching it off drops the footer's results, so the footers name
//! the ends of their ranges again - and a read still under way then is not kept. A result is
//! kept under the address the host knows for the view; a page that names another one (an error
//! page, an address the host missed) is not kept. A changed config (the analyser's settings,
//! the breakpoints) makes the results stale: they go and the views are read again.
//!
//! The waits, the switch and the reads under way are the host's ([`AutoState`] in its footer
//! state), not the process's.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Weak};
use std::time::Duration;

use rb_engine::{ViewEvent, ViewHost, ViewId};
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

use super::document_of;
use super::read::{PageRead, READ_WORLD, expression};
use super::results::PageQueries;
use crate::control::Core;

/// How long a view's page loads must be quiet before its document is read: a login redirects
/// several times, and a load's end follows its commit.
pub const SETTLE: Duration = Duration::from_millis(400);
/// How long the script may take in a view.
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(5);

/// The waits of the views.
#[derive(Clone, Debug, Default)]
pub struct Due {
    /// The number of the last wait.
    ticket: u64,
    views: BTreeMap<ViewId, ViewDue>,
}

#[derive(Clone, Debug, Default)]
struct ViewDue {
    /// The wait that counts; `None` once it was used.
    ticket: Option<u64>,
    /// How many loads of the view had ended at its last wait.
    loads: u64,
}

impl Due {
    fn wait(&mut self, view: ViewId) -> u64 {
        self.ticket += 1;
        self.views.entry(view).or_default().ticket = Some(self.ticket);
        self.ticket
    }

    /// `view` committed an address: the ticket of its new wait.
    pub fn navigated(&mut self, view: ViewId) -> u64 {
        self.wait(view)
    }

    /// `view` changed and `loads` of its loads have ended: the ticket of its new wait when one
    /// ended since the last.
    pub fn updated(&mut self, view: ViewId, loads: u64) -> Option<u64> {
        let known = self.views.entry(view).or_default();
        if loads <= known.loads {
            return None;
        }
        known.loads = loads;
        Some(self.wait(view))
    }

    /// The wait `ticket` of `view` ended: whether the view is read now - not when a newer wait
    /// runs, the view closed or everything was reset.
    pub fn take(&mut self, view: ViewId, ticket: u64) -> bool {
        match self.views.get_mut(&view) {
            Some(known) if known.ticket == Some(ticket) => {
                known.ticket = None;
                true
            }
            _ => false,
        }
    }

    /// `loads` of the loads of `view` have ended and are read with the wait that starts now:
    /// only a load that ends after them starts another.
    pub fn seen(&mut self, view: ViewId, loads: u64) {
        let known = self.views.entry(view).or_default();
        known.loads = known.loads.max(loads);
    }

    /// `view` closed: its wait ends. Its count of loads stays - a late change of the closed
    /// view starts nothing.
    pub fn forget(&mut self, view: ViewId) {
        if let Some(known) = self.views.get_mut(&view) {
            known.ticket = None;
        }
    }

    /// Ends every wait. The counts of loads stay: a title or a size that changes afterwards
    /// is no ended load.
    pub fn reset(&mut self) {
        for known in self.views.values_mut() {
            known.ticket = None;
        }
    }
}

/// What a change of `ui.viewFooter.analyze` asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Switched on: the pages the views show are read.
    On,
    /// Switched off: the footer's results go.
    Off,
}

/// `ui.viewFooter.analyze` as it was last: only its own change does something - not the
/// footer shown or hidden, not another field list.
#[derive(Clone, Debug, Default)]
pub struct Switch(Option<bool>);

impl Switch {
    /// The setting is `on` now.
    pub fn set(&mut self, on: bool) -> Option<Change> {
        let before = self.0.replace(on);
        match (before, on) {
            (Some(false), true) => Some(Change::On),
            (Some(true), false) => Some(Change::Off),
            // The start, or no change.
            _ => None,
        }
    }
}

/// The documents whose read is with the analyser plugin right now.
#[derive(Clone, Debug, Default)]
pub struct Reading(BTreeSet<(String, u64)>);

impl Reading {
    /// Whether `document` with the content `signature` is to be analysed: not while another
    /// view's same read is.
    pub fn start(&mut self, document: &str, signature: u64) -> bool {
        self.0.insert((document.to_owned(), signature))
    }

    /// The analysis of `document` with `signature` ended.
    pub fn end(&mut self, document: &str, signature: u64) {
        self.0.remove(&(document.to_owned(), signature));
    }
}

/// What the host keeps for the footer's reads: the views' waits, the switch as it was last
/// and the reads that are with the analyser.
#[derive(Clone, Debug, Default)]
pub struct AutoState {
    /// The waits of the views.
    pub due: Due,
    /// `ui.viewFooter.analyze` as it was last.
    pub switch: Switch,
    /// The reads under way.
    pub reading: Reading,
}

/// Follows the page loads of `core`'s views.
pub fn on_start(core: &Core) {
    let Some(core) = core.arc() else {
        return;
    };
    let on = core.user_settings().ui.view_footer.analyze;
    core.host().footer_auto(|auto| auto.switch.set(on));
    let mut events = core.host().subscribe();
    let weak = Arc::downgrade(&core);
    core.runtime().spawn(async move {
        loop {
            let event = events.recv().await;
            let Some(core) = weak.upgrade() else {
                return;
            };
            let host = core.host();
            match event {
                Ok(ViewEvent::Navigated { view, .. }) => {
                    let ticket = host.footer_auto(|auto| auto.due.navigated(view));
                    wait(&core, view, ticket);
                }
                Ok(ViewEvent::Updated(view)) => {
                    let ticket = host
                        .loads_ended(view)
                        .and_then(|loads| host.footer_auto(|auto| auto.due.updated(view, loads)));
                    if let Some(ticket) = ticket {
                        wait(&core, view, ticket);
                    }
                }
                Ok(ViewEvent::Closed(view)) => host.footer_auto(|auto| auto.due.forget(view)),
                // A config that changed what an analysis says: the results went, read again.
                Ok(ViewEvent::LayoutChanged | ViewEvent::StateChanged) => {
                    if host.footer_take_stale() {
                        read_all(&core);
                    }
                }
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => return,
            }
        }
    });
}

/// The user settings changed: `ui.viewFooter.analyze` switched on reads the pages the views
/// show; switched off, the footer's results go and the footers name the ends of their ranges.
/// Any other setting does nothing here.
pub fn on_user_settings_changed(core: &Core) {
    let on = core.user_settings().ui.view_footer.analyze;
    let host = core.host();
    match host.footer_auto(|auto| auto.switch.set(on)) {
        Some(Change::On) => read_all(core),
        Some(Change::Off) => {
            host.footer_auto(|auto| auto.due.reset());
            host.footer_results_clear();
        }
        None => {}
    }
}

/// Starts a wait for every view: the pages they show are read. The loads that have ended are
/// read with it.
fn read_all(core: &Core) {
    let host = core.host();
    for view in host.views() {
        let loads = host.loads_ended(view.id).unwrap_or(0);
        let ticket = host.footer_auto(|auto| {
            auto.due.seen(view.id, loads);
            auto.due.navigated(view.id)
        });
        wait(core, view.id, ticket);
    }
}

/// Starts the wait `ticket` of `view`; at its end the view's document is read.
fn wait(core: &Core, view: ViewId, ticket: u64) {
    if !core.user_settings().ui.view_footer.analyze {
        return;
    }
    let Some(core) = core.arc() else {
        return;
    };
    let weak: Weak<Core> = Arc::downgrade(&core);
    core.runtime().spawn(async move {
        tokio::time::sleep(SETTLE).await;
        let Some(core) = weak.upgrade() else {
            return;
        };
        if core.user_settings().ui.view_footer.analyze
            && core.host().footer_auto(|auto| auto.due.take(view, ticket))
        {
            read(&core, view).await;
        }
    });
}

/// Reads the document `view` shows and keeps what its media queries say for the footers.
async fn read(core: &Arc<Core>, view: ViewId) {
    let host = core.host();
    // The script runs over CDP: Chromium views. The other engines' views show the same site;
    // they take the result of their document.
    let Some(start) = host.footer_read_start(view) else {
        return;
    };
    let Some(page) = evaluate(host.as_ref(), view).await else {
        return;
    };
    // Kept under the address the host knows for the view - where the footers look it up. A
    // page that names another one is not this document: an error page, a missed address.
    if document_of(&page.url) != start.document {
        tracing::debug!(%view, "the page read for the footer is not the view's document");
        return;
    }
    let document = start.document;
    let signature = page.signature(start.config_mark);
    if host.footer_result_signature(&document) == Some(signature) {
        return;
    }
    if !host.footer_auto(|auto| auto.reading.start(&document, signature)) {
        return;
    }
    if page.skipped > 0 || page.capped {
        tracing::debug!(
            %view,
            skipped = page.skipped,
            capped = page.capped,
            sheets = page.sheets,
            "the footer's read of the page is incomplete"
        );
    }
    let detected = match page.collected() {
        // A page without a media query: nothing to analyse, and that is its result.
        None => Ok(Vec::new()),
        Some(collected) => {
            let config = host.config();
            let analysed = tokio::task::spawn_blocking(move || {
                let plugins = crate::analyzer::plugins::bundled();
                crate::analyzer::run::detect(plugins, &config, &collected, &[], None)
            })
            .await;
            match analysed {
                Ok(Ok(detection)) => Ok(detection.detected),
                Ok(Err(error)) => Err(error.to_string()),
                Err(error) => Err(error.to_string()),
            }
        }
    };
    host.footer_auto(|auto| auto.reading.end(&document, signature));
    match detected {
        Ok(detected) => {
            let found = detected.len();
            let queries = PageQueries {
                detected,
                skipped: page.skipped,
                capped: page.capped,
                signature,
            };
            // Not kept when the setting was switched off or the config changed meanwhile.
            let kept = host.footer_page_read(&document, queries, start.generation);
            tracing::debug!(%view, found, kept, "the footer read the page's queries");
        }
        // Not kept: the next load of the view reads again.
        Err(error) => tracing::debug!(%view, %error, "the footer's queries were not analysed"),
    }
}

/// The value of the read script in [`READ_WORLD`] of `view`'s main frame; `None` when the view
/// has no session yet, the script failed or took too long. Never the page's own world.
async fn evaluate(host: &dyn ViewHost, view: ViewId) -> Option<PageRead> {
    let script = async {
        let tree = host.cdp_send(view, "Page.getFrameTree", json!({})).await?;
        let frame = tree
            .pointer("/frameTree/frame/id")
            .and_then(Value::as_str)
            .ok_or_else(|| rb_engine::EngineError::Invalid("no main frame".into()))?
            .to_owned();
        let world = host
            .cdp_send(
                view,
                "Page.createIsolatedWorld",
                json!({ "frameId": frame, "worldName": READ_WORLD }),
            )
            .await?;
        let context = world
            .get("executionContextId")
            .and_then(Value::as_i64)
            .ok_or_else(|| rb_engine::EngineError::Invalid("no isolated world".into()))?;
        host.cdp_send(
            view,
            "Runtime.evaluate",
            json!({
                "expression": expression(),
                "contextId": context,
                "returnByValue": true,
                "awaitPromise": false,
                "silent": true,
            }),
        )
        .await
    };
    let result = match tokio::time::timeout(SCRIPT_TIMEOUT, script).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => {
            tracing::debug!(%view, %error, "the footer's read of the page failed");
            return None;
        }
        Err(_) => {
            tracing::debug!(%view, "the footer's read of the page took too long");
            return None;
        }
    };
    if result.get("exceptionDetails").is_some() {
        tracing::debug!(%view, "the footer's read script threw");
        return None;
    }
    let value = result.get("result")?.get("value")?.clone();
    serde_json::from_value(value).ok()
}

#[cfg(test)]
#[path = "auto_tests.rs"]
mod tests;

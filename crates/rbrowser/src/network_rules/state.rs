//! [`Rules`]: the documents the plugins answered, what of them applies (project rules only in a
//! trusted workspace), the hits per rule, and the `$network` state each call of such a plugin
//! gets. Plain data behind a mutex; the CDP side is [`super::executor`]. The debugger's parts -
//! breakpoints, held requests, asks and their answers, the composed requests - are
//! [`super::debug`] on the same data.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use rb_engine::{EngineKind, ViewId};
use serde_json::{Value, json};
use tokio::sync::watch;

use super::action::Action;
use super::compose::Runner;
use super::document::{Breakpoint, DebugSettings};
use super::fetch::{self, Outcome, Paused};
use super::held::Held;
use super::rule::{self, Document, Origin, Rejected, Rule};
use crate::cdp::lock;

/// The rules of every plugin, in plugin order, as they apply now.
pub type Active = Arc<Vec<(String, Vec<Rule>)>>;

pub(super) struct Applied {
    /// The answer as it came (its asks taken out), to tell an unchanged one.
    raw: Value,
    pub(super) document: Document,
    rejected: Vec<Rejected>,
    /// Why the answer did not read, if it did not.
    unreadable: Option<String>,
}

#[derive(Default)]
pub(super) struct Inner {
    pub(super) documents: BTreeMap<String, Applied>,
    pub(super) trusted: bool,
    pub(super) hits: BTreeMap<String, BTreeMap<String, u64>>,
    /// Bodies a rule left alone because they were over the limit, per plugin and rule.
    pub(super) skipped: BTreeMap<String, BTreeMap<String, u64>>,
    pub(super) intercepting: BTreeSet<ViewId>,
    clear_pending: bool,
    pub(super) active: Active,
    /// The breakpoints that apply now (of the documents whose switch is on).
    pub(super) breakpoints: Arc<Vec<Breakpoint>>,
    pub(super) breakpoints_enabled: bool,
    pub(super) settings: DebugSettings,
    /// The plugins whose documents are project content: no debugger parts from them.
    pub(super) project_plugins: BTreeSet<String>,
    /// The asks seen and their answers, by plugin and ask id, kept while the ask is in that
    /// plugin's document.
    pub(super) answers: BTreeMap<String, BTreeMap<String, Value>>,
    /// The newest composed requests' results.
    pub(super) composed: VecDeque<Value>,
    pub(super) project_root: Option<PathBuf>,
}

/// The views of the window, for the state: id and engine.
pub type ViewsFn = Box<dyn Fn() -> Vec<(ViewId, EngineKind)> + Send + Sync>;

/// The host's interception rules of one window.
pub struct Rules {
    pub(super) inner: Mutex<Inner>,
    changed: watch::Sender<u64>,
    views: ViewsFn,
    session: String,
    pub(super) held: Arc<Held>,
    pub(super) runner: OnceLock<Arc<dyn Runner>>,
    /// This `Rules` as the runner's finished tasks reach it.
    pub(super) me: OnceLock<Weak<Rules>>,
}

impl Rules {
    /// Rules over the views `views` lists; `session` names this run of the app.
    #[must_use]
    pub fn new(views: ViewsFn, session: String) -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            changed: watch::channel(0).0,
            views,
            session,
            held: Arc::new(Held::default()),
            runner: OnceLock::new(),
            me: OnceLock::new(),
        }
    }

    /// Ticks whenever what applies changed.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }

    pub(super) fn notify(&self) {
        self.changed.send_modify(|generation| *generation += 1);
    }

    /// The project the rules run in: what map-local folders are resolved against.
    pub fn set_project_root(&self, root: Option<PathBuf>) {
        let mut inner = lock(&self.inner);
        if inner.project_root == root {
            return;
        }
        inner.project_root = root;
        // The map-local folders resolve against the project: the next answer is read anew.
        for applied in inner.documents.values_mut() {
            applied.raw = Value::Null;
        }
    }

    /// Plugin `plugin`'s answer to `@rules`: validated, it replaces the plugin's rules. A raise of
    /// its `clearSiteData` counter (not its first value) asks for one clearing. An answer equal
    /// to the last changes nothing and ticks no subscriber (the panel polls every few seconds);
    /// its asks are run either way.
    ///
    /// # Errors
    ///
    /// The document does not read; the plugin's rules are dropped then.
    pub fn apply(&self, plugin: &str, document: &Value) -> Result<Vec<Rejected>, String> {
        self.apply_as(plugin, document, false)
    }

    /// As [`Rules::apply`] for a project plugin: every rule of its answer is a project rule
    /// whatever origin it declares, so none applies without workspace trust (review finding M2
    /// of Charge 7), and its debugger parts (breakpoints, settings, asks) are ignored.
    ///
    /// # Errors
    ///
    /// As [`Rules::apply`].
    pub fn apply_from_project(
        &self,
        plugin: &str,
        document: &Value,
    ) -> Result<Vec<Rejected>, String> {
        self.apply_as(plugin, document, true)
    }

    /// Drops plugin `plugin`'s rules, hits and answer (it may not run any more); ticks the
    /// subscribers only when it had an answer.
    pub fn remove(&self, plugin: &str) {
        {
            let mut inner = lock(&self.inner);
            inner.hits.remove(plugin);
            inner.skipped.remove(plugin);
            inner.project_plugins.remove(plugin);
            if inner.documents.remove(plugin).is_none() {
                return;
            }
            rebuild(&mut inner);
        }
        self.notify();
    }

    fn apply_as(
        &self,
        plugin: &str,
        document: &Value,
        project: bool,
    ) -> Result<Vec<Rejected>, String> {
        let mut raw = document.clone();
        if let Value::Object(map) = &mut raw {
            map.remove("asks");
        }
        let unchanged = lock(&self.inner)
            .documents
            .get(plugin)
            .filter(|applied| applied.raw == raw)
            .map(|applied| {
                applied
                    .unreadable
                    .clone()
                    .map_or_else(|| Ok(applied.rejected.clone()), Err)
            });
        if let Some(outcome) = unchanged {
            if !project {
                // Only the asks are new: the rules are not parsed again.
                let (asks, _) = super::document::parse_asks(&document["asks"]);
                self.run_asks(plugin, asks);
            }
            return outcome;
        }
        let parsed = rule::parse(document).map(|(mut document, rejected)| {
            if project {
                for rule in &mut document.rules {
                    rule.origin = Origin::Project;
                }
                document.breakpoints.clear();
                document.settings = None;
                document.asks.clear();
            }
            (document, rejected)
        });
        let (asks, rejected) = {
            let mut inner = lock(&self.inner);
            if project {
                inner.project_plugins.insert(plugin.to_owned());
            }
            let previous = inner
                .documents
                .get(plugin)
                .map(|applied| applied.document.clear_site_data);
            let (mut document, mut rejected) = match &parsed {
                Ok((document, rejected)) => (document.clone(), rejected.clone()),
                Err(reason) => (
                    Document {
                        clear_site_data: previous.unwrap_or(0),
                        ..Document::default()
                    },
                    vec![Rejected {
                        id: "*".to_owned(),
                        reason: reason.clone(),
                    }],
                ),
            };
            if previous.is_some_and(|seen| document.clear_site_data > seen) {
                inner.clear_pending = true;
            }
            if let Some(settings) = &document.settings {
                inner.settings = settings.clone();
            }
            let roots = inner.settings.map_local_roots.clone();
            resolve_local(
                &mut document,
                &mut rejected,
                inner.project_root.as_deref(),
                &roots,
            );
            let asks = std::mem::take(&mut document.asks);
            let applied = Applied {
                raw,
                document,
                rejected: rejected.clone(),
                unreadable: parsed.as_ref().err().cloned(),
            };
            inner.documents.insert(plugin.to_owned(), applied);
            rebuild(&mut inner);
            (asks, rejected)
        };
        self.notify();
        self.run_asks(plugin, asks);
        parsed.map(|_| rejected)
    }

    /// Workspace trust changed: project rules apply only while it holds.
    pub fn set_trusted(&self, trusted: bool) {
        let mut inner = lock(&self.inner);
        if inner.trusted == trusted {
            return;
        }
        inner.trusted = trusted;
        rebuild(&mut inner);
        drop(inner);
        self.notify();
    }

    /// The rules that apply now.
    #[must_use]
    pub fn active(&self) -> Active {
        Arc::clone(&lock(&self.inner).active)
    }

    /// Whether a plugin asked for the cache to be off.
    #[must_use]
    pub fn cache_disabled(&self) -> bool {
        lock(&self.inner)
            .documents
            .values()
            .any(|applied| applied.document.cache_disabled)
    }

    /// Whether a clearing of the site data is asked for; asking resets it.
    pub fn take_clear(&self) -> bool {
        std::mem::take(&mut lock(&self.inner).clear_pending)
    }

    /// The views whose requests are intercepted now.
    pub fn set_intercepting(&self, views: BTreeSet<ViewId>) {
        lock(&self.inner).intercepting = views;
    }

    /// The outcome the active rules give `request`; a hit is counted for the plugin.
    #[must_use]
    pub fn decide(&self, request: &Paused) -> Outcome {
        let active = self.active();
        for (plugin, rules) in active.iter() {
            let outcome = fetch::decide(rules, request);
            if let Some(rule) = outcome.rule() {
                *lock(&self.inner)
                    .hits
                    .entry(plugin.clone())
                    .or_default()
                    .entry(rule.to_owned())
                    .or_default() += 1;
                return outcome;
            }
        }
        fetch::decide(&[], request)
    }

    /// The `$network` state of plugin `plugin`: this run's session, trust, the plugin's hits and
    /// rejected rules, the cache switch, per view whether its engine is intercepted, and the
    /// debugger's parts ([`super::debug`]).
    #[must_use]
    pub fn state(&self, plugin: &str) -> Value {
        let views = (self.views)();
        let inner = lock(&self.inner);
        let rejected: Vec<Value> = inner
            .documents
            .get(plugin)
            .map(|applied| {
                applied
                    .rejected
                    .iter()
                    .map(|r| json!({ "id": r.id, "reason": r.reason }))
                    .collect()
            })
            .unwrap_or_default();
        let applied = inner
            .active
            .iter()
            .find(|(owner, _)| owner == plugin)
            .map_or(0, |(_, rules)| rules.len());
        let views: Vec<Value> = views
            .iter()
            .map(|(view, engine)| {
                json!({
                    "view": view.0,
                    "supported": *engine == EngineKind::Chromium,
                    "intercepting": inner.intercepting.contains(view),
                })
            })
            .collect();
        let mut state = json!({
            "session": self.session,
            "trusted": inner.trusted,
            "hits": inner.hits.get(plugin).cloned().unwrap_or_default(),
            "skipped": inner.skipped.get(plugin).cloned().unwrap_or_default(),
            "rejected": rejected,
            "applied": applied,
            "cacheDisabled": inner
                .documents
                .values()
                .any(|applied| applied.document.cache_disabled),
            "views": views,
        });
        self.debug_state(&inner, plugin, &mut state);
        state
    }
}

/// Resolves every map-local rule of `document` against the project and `roots`; one outside
/// them moves to `rejected`.
fn resolve_local(
    document: &mut Document,
    rejected: &mut Vec<Rejected>,
    project: Option<&std::path::Path>,
    roots: &[String],
) {
    let mut kept = Vec::with_capacity(document.rules.len());
    for mut rule in std::mem::take(&mut document.rules) {
        if let Action::MapLocal(local) = &mut rule.action {
            let given = local
                .folder
                .as_deref()
                .or(local.file.as_deref())
                .unwrap_or_default();
            match super::local::resolve(given, project, roots) {
                Ok(path) => local.resolved = Some(path),
                Err(reason) => {
                    rejected.push(Rejected {
                        id: rule.id.clone(),
                        reason: reason.to_owned(),
                    });
                    continue;
                }
            }
        }
        kept.push(rule);
    }
    document.rules = kept;
}

/// What applies: every plugin's rules, its project rules only under trust; the breakpoints of
/// the documents whose switch is on.
pub(super) fn rebuild(inner: &mut Inner) {
    let trusted = inner.trusted;
    let active = inner
        .documents
        .iter()
        .map(|(plugin, applied)| {
            let rules = applied
                .document
                .rules
                .iter()
                .filter(|rule| trusted || rule.origin == Origin::Settings)
                .cloned()
                .collect();
            (plugin.clone(), rules)
        })
        .collect();
    inner.active = Arc::new(active);
    let breakpoints: Vec<Breakpoint> = inner
        .documents
        .values()
        .filter(|applied| applied.document.breakpoints_enabled)
        .flat_map(|applied| applied.document.breakpoints.iter().cloned())
        .collect();
    inner.breakpoints_enabled = inner.documents.values().any(|applied| {
        !applied.document.breakpoints.is_empty() && applied.document.breakpoints_enabled
    });
    inner.breakpoints = Arc::new(breakpoints);
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;

//! The debugger's half of [`Rules`] (RB-080-37): the breakpoints' verdict before the rules', the
//! held requests, the asks a document brings (continue, abort, compose, repeat) with the answers
//! `$network.answers` keeps while the ask stays in the document, and the composed results
//! `$network.composed` lists. Everything that leaves the host - the panel's state and the MCP
//! answers alike - is masked ([`super::mask`]): a masked value sent back in an edit means "as
//! it was" ([`super::held::unmask_edits`]).

use std::sync::{Arc, Weak};

use rb_engine::ViewId;
use serde_json::{Value, json};

use super::compose::{MAX_COMPOSED, Runner};
use super::document::{Ask, Breakpoint, DebugSettings};
use super::fetch::{self, Outcome, Paused};
use super::held::{Held, HeldError, Resolution};
use super::mask;
use super::state::{Inner, Rules};
use crate::cdp::lock;

/// The most bytes of a held body `$network.held` carries.
pub const STATE_BODY_BYTES: usize = 256 * 1024;

/// What a paused request gets: held by a breakpoint, or the rules' outcome.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// Held by the breakpoint named.
    Hold(String),
    Outcome(Outcome),
}

impl Rules {
    /// The verdict for `request` of `view`: a breakpoint of its stage covering the view holds
    /// it; else the rules decide ([`Rules::decide`]).
    #[must_use]
    pub fn verdict(&self, view: ViewId, request: &Paused) -> Verdict {
        let breakpoints = {
            let inner = lock(&self.inner);
            if !inner.breakpoints_enabled {
                Arc::new(Vec::new())
            } else {
                Arc::clone(&inner.breakpoints)
            }
        };
        let held = breakpoints.iter().find(|breakpoint| {
            breakpoint.enabled
                && breakpoint.stage == request.stage
                && breakpoint
                    .views
                    .as_ref()
                    .is_none_or(|views| views.contains(&view))
                && fetch::matcher_matches(&breakpoint.matcher, request)
        });
        match held {
            Some(breakpoint) => Verdict::Hold(breakpoint.id.clone()),
            None => Verdict::Outcome(self.decide(request)),
        }
    }

    /// The held requests.
    #[must_use]
    pub fn held(&self) -> &Arc<Held> {
        &self.held
    }

    /// The debugger settings in force.
    #[must_use]
    pub fn settings(&self) -> DebugSettings {
        lock(&self.inner).settings.clone()
    }

    /// The breakpoints that apply now, and whether the switch is on.
    #[must_use]
    pub fn breakpoints(&self) -> (bool, Arc<Vec<Breakpoint>>) {
        let inner = lock(&self.inner);
        (inner.breakpoints_enabled, Arc::clone(&inner.breakpoints))
    }

    /// The `Fetch.enable` patterns of every rule that applies and every breakpoint that is on,
    /// each once.
    #[must_use]
    pub fn patterns(&self) -> Vec<Value> {
        let (enabled, breakpoints) = self.breakpoints();
        let mut all: Vec<Value> = Vec::new();
        let rules = self.active();
        for pattern in rules
            .iter()
            .flat_map(|(_, rules)| fetch::patterns(rules))
            .chain(if enabled {
                fetch::breakpoint_patterns(&breakpoints)
            } else {
                Vec::new()
            })
        {
            if !all.contains(&pattern) {
                all.push(pattern);
            }
        }
        all
    }

    /// The host's compose runner; `me` is this `Rules` as the runner's tasks reach it.
    pub fn set_runner(&self, me: Weak<Rules>, runner: Arc<dyn Runner>) {
        // Set once per window; a second call keeps the first.
        let _ = self.runner.set(runner);
        let _ = self.me.set(me);
    }

    /// The compose runner, once the host set one.
    #[must_use]
    pub fn runner(&self) -> Option<Arc<dyn Runner>> {
        self.runner.get().cloned()
    }

    /// Runs the asks of `plugin`'s document that were not run yet, and forgets the answers of
    /// the asks the document no longer carries.
    pub(super) fn run_asks(&self, plugin: &str, asks: Vec<Ask>) {
        if lock(&self.inner).project_plugins.contains(plugin) {
            return;
        }
        let fresh: Vec<Ask> = {
            let mut inner = lock(&self.inner);
            let answers = inner.answers.entry(plugin.to_owned()).or_default();
            answers.retain(|id, _| asks.iter().any(|ask| ask.id() == id));
            let fresh: Vec<Ask> = asks
                .into_iter()
                .filter(|ask| !answers.contains_key(ask.id()))
                .collect();
            for ask in &fresh {
                answers.insert(ask.id().to_owned(), json!({ "running": true }));
            }
            fresh
        };
        for ask in fresh {
            match ask {
                Ask::Continue {
                    id,
                    held,
                    all_views,
                    edits,
                } => {
                    let answer = self.resolve_answer(self.held.resolve(
                        &held,
                        all_views,
                        Resolution::Continue(edits),
                    ));
                    self.answer_ask(plugin, &id, answer);
                }
                Ask::Abort {
                    id,
                    held,
                    all_views,
                } => {
                    let answer =
                        self.resolve_answer(self.held.resolve(&held, all_views, Resolution::Abort));
                    self.answer_ask(plugin, &id, answer);
                }
                Ask::Compose {
                    id,
                    view,
                    request,
                    times,
                } => {
                    let Some((runner, me)) = self.runner_and_me() else {
                        self.answer_ask(plugin, &id, json!({ "error": "no_runner" }));
                        continue;
                    };
                    let results = runner.compose(view, request, times);
                    let plugin = plugin.to_owned();
                    runner.spawn(Box::pin(async move {
                        let results = results.await;
                        if let Some(rules) = me.upgrade() {
                            rules.finish_compose(&plugin, &id, Ok(results));
                        }
                    }));
                }
                Ask::Repeat {
                    id,
                    view,
                    request_id,
                    edits,
                    times,
                } => {
                    let Some((runner, me)) = self.runner_and_me() else {
                        self.answer_ask(plugin, &id, json!({ "error": "no_runner" }));
                        continue;
                    };
                    let results = runner.repeat(view, request_id, edits, times);
                    let plugin = plugin.to_owned();
                    runner.spawn(Box::pin(async move {
                        let results = results.await;
                        if let Some(rules) = me.upgrade() {
                            rules.finish_compose(&plugin, &id, results);
                        }
                    }));
                }
            }
        }
    }

    fn runner_and_me(&self) -> Option<(Arc<dyn Runner>, Weak<Rules>)> {
        Some((self.runner.get()?.clone(), self.me.get()?.clone()))
    }

    fn resolve_answer(&self, resolved: Result<usize, HeldError>) -> Value {
        match resolved {
            Ok(count) => json!({ "done": true, "resolved": count }),
            Err(HeldError::Unknown) => json!({ "error": "unknown_held" }),
        }
    }

    /// Records `answer` for ask `id` of `plugin` (kept while the ask stays in its document).
    pub fn answer_ask(&self, plugin: &str, id: &str, answer: Value) {
        let mut inner = lock(&self.inner);
        if let Some(answers) = inner.answers.get_mut(plugin)
            && answers.contains_key(id)
        {
            answers.insert(id.to_owned(), answer);
        }
    }

    /// A compose or repeat finished: its results go into `composed` (newest last) and its ask
    /// is answered.
    pub fn finish_compose(
        &self,
        plugin: &str,
        id: &str,
        results: Result<Vec<super::compose::ComposeResult>, String>,
    ) {
        match results {
            Ok(results) => {
                let count = results.len();
                for result in &results {
                    let mut entry = result.to_json(true);
                    entry["ask"] = json!(id);
                    self.push_composed(entry);
                }
                self.answer_ask(plugin, id, json!({ "done": true, "results": count }));
            }
            Err(error) => self.answer_ask(plugin, id, json!({ "error": error })),
        }
    }

    /// Keeps `result` among the newest [`MAX_COMPOSED`] composed results.
    pub fn push_composed(&self, result: Value) {
        let mut inner = lock(&self.inner);
        if inner.composed.len() >= MAX_COMPOSED {
            inner.composed.pop_front();
        }
        inner.composed.push_back(result);
    }

    /// The composed results, oldest first, masked.
    #[must_use]
    pub fn composed(&self) -> Vec<Value> {
        lock(&self.inner).composed.iter().cloned().collect()
    }

    /// The held requests with cookies and tokens masked, the bodies cut to `max_body`.
    #[must_use]
    pub fn held_masked(&self, max_body: usize) -> Vec<Value> {
        self.held
            .list()
            .iter()
            .map(|request| {
                let mut entry = request.to_json(max_body);
                mask::mask_entry(&mut entry);
                entry
            })
            .collect()
    }

    /// The breakpoints as MCP lists them.
    #[must_use]
    pub fn breakpoints_json(&self) -> Value {
        let (enabled, breakpoints) = self.breakpoints();
        json!({
            "enabled": enabled,
            "breakpoints": breakpoints.iter().map(breakpoint_json).collect::<Vec<_>>(),
        })
    }

    /// A response body a rule left alone because it was over the limit.
    pub fn note_skipped(&self, plugin: &str, rule: &str) {
        *lock(&self.inner)
            .skipped
            .entry(plugin.to_owned())
            .or_default()
            .entry(rule.to_owned())
            .or_default() += 1;
    }

    /// The plugin that owns rule `rule` among the active rules.
    #[must_use]
    pub fn owner_of(&self, rule: &str) -> Option<String> {
        self.active()
            .iter()
            .find(|(_, rules)| rules.iter().any(|known| known.id == rule))
            .map(|(plugin, _)| plugin.clone())
    }

    /// The debugger's parts of `$network`: the breakpoints' state, the held requests (not for
    /// a project plugin), the answers, the composed results and the settings.
    pub(super) fn debug_state(&self, inner: &Inner, plugin: &str, state: &mut Value) {
        state["breakpoints"] = json!({
            "enabled": inner.breakpoints_enabled,
            "active": inner.breakpoints.len(),
        });
        state["answers"] = inner
            .answers
            .get(plugin)
            .map_or_else(|| json!({}), |answers| json!(answers));
        state["composed"] = Value::Array(inner.composed.iter().cloned().collect());
        state["settings"] = json!({
            "timeoutMs": inner.settings.timeout_ms,
            "maxBodyBytes": inner.settings.max_body_bytes,
            "allowComposeFromMcp": inner.settings.allow_compose_from_mcp,
        });
        if !inner.project_plugins.contains(plugin) {
            state["held"] = Value::Array(self.held_masked(STATE_BODY_BYTES));
        }
    }
}

/// A breakpoint as the panel and MCP see it.
#[must_use]
pub fn breakpoint_json(breakpoint: &Breakpoint) -> Value {
    use super::rule::UrlPattern;
    let (pattern, regex) = match &breakpoint.matcher.url {
        UrlPattern::Glob(glob) => (glob.clone(), false),
        UrlPattern::Regex(regex) => (regex.as_str().to_owned(), true),
    };
    json!({
        "id": breakpoint.id,
        "urlPattern": pattern,
        "regex": regex,
        "method": breakpoint.matcher.method,
        "resourceType": breakpoint.matcher.resource_type,
        "stage": breakpoint.stage.name(),
        "views": breakpoint.views.as_ref().map(|views| views.iter().map(|view| view.0).collect::<Vec<_>>()),
        "enabled": breakpoint.enabled,
    })
}

#[cfg(test)]
#[path = "debug_tests.rs"]
mod tests;

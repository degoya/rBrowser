//! The report window (RB-091-02): a plugin's HTML report - Lighthouse's per run and breakpoint -
//! shown inside rBrowser, isolated from everything else.
//!
//! **How it opens.** The plugin's answer (its panel, its command, its MCP tool) carries
//! `$report` ([`ask`]); [`take`] removes it from the answer, admits only a bundled plugin of
//! [`ask::PLUGINS`] in an open, trusted project, checks the path ([`guard`]: below the plugin's
//! own `.rbrowser/out/<id>/`, no `..`, no link) and hands the window an address of its own
//! ([`tokens`]). The same answer may name run folders to delete (the history, the retention):
//! each is checked the same way and removed with what is in it.
//!
//! **What the report can reach.** It is HTML with script, so it runs in a top-level window of
//! its own (`cef_app/report_window.rs`), never in the shell, a panel or a grid view: an origin of
//! its own (`rbrowser://report/`) without `cefQuery` and without the plugins' page bridge, hidden
//! from outer CDP clients like every `rbrowser:` page, under [`tokens::CSP`] (no network, no
//! frames, no forms). It may only show report addresses; a link the person clicks opens through
//! `app.open_link` (http and https only), every other navigation and every popup is refused.

pub mod ask;
pub mod guard;
pub mod tokens;
pub mod windows;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Value, json};

use crate::control::Core;
use crate::plugins::runtime::PluginRuntime;
use ask::{Ask, Placement};
use guard::Refusal;
use tokens::{Entry, Tokens};

/// The window's title; `{name}` is the report's path below the plugin's folder.
pub const TITLE_KEY: &str = "reports.window_title";
/// The notice after run folders were deleted; `{count}`.
pub const REMOVED_KEY: &str = "reports.notice.removed";

static TOKENS: Mutex<Tokens> = Mutex::new(Tokens::new());

/// What the window is asked to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportOpen {
    /// The report's address (`rbrowser://report/<token>/<file>`).
    pub url: String,
    /// The window title.
    pub title: String,
    pub placement: Placement,
}

/// The report `url` names, checked again on every load: its file when the token is live, the
/// open project `root` is still the token's and trusted (security review L1: a revoked trust or
/// another project ends every address) and the file still passes [`guard::report_file`].
///
/// # Errors
///
/// [`Refusal::NoProject`] or [`Refusal::Untrusted`] from [`guard::admit`],
/// [`Refusal::NotFound`] for an unknown token or another project, else the guard's.
pub fn resolve(url: &str, root: Option<PathBuf>, trusted: bool) -> Result<PathBuf, Refusal> {
    let entry = rb_util::lock(&TOKENS)
        .resolve(url)
        .cloned()
        .ok_or(Refusal::NotFound)?;
    if guard::admit(root, trusted)? != entry.root {
        return Err(Refusal::NotFound);
    }
    guard::report_file(&entry.root, &entry.plugin, &entry.rel)
}

/// `url` is no longer shown: its address stops working.
pub fn revoke(url: &str) {
    rb_util::lock(&TOKENS).revoke(url);
}

/// What one ask did, for the answer.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The report to show, when it passed.
    pub open: Option<ReportOpen>,
    /// Why the report did not open.
    pub open_refused: Option<Refusal>,
    pub removed: Vec<String>,
    pub not_removed: Vec<(String, Refusal)>,
}

impl Outcome {
    /// The outcome as fields of the plugin's answer: `window` and `removed`.
    fn merge_into(&self, ask: &Ask, answer: &mut Value) {
        let Some(fields) = answer.as_object_mut() else {
            return;
        };
        if ask.open.is_some() {
            let window = match self.open_refused {
                Some(refusal) => json!({ "opened": false, "reason": refusal.code() }),
                None => json!({ "opened": true }),
            };
            fields.insert("window".to_owned(), window);
        }
        if !ask.remove.is_empty() {
            fields.insert("removed".to_owned(), json!(self.removed));
            let refused: Vec<Value> = self
                .not_removed
                .iter()
                .map(|(run, refusal)| json!({ "run": run, "reason": refusal.code() }))
                .collect();
            fields.insert("notRemoved".to_owned(), Value::Array(refused));
        }
    }
}

/// Carries out `ask` of plugin `plugin` in the project at `root` (trusted when `trusted`); the
/// removals first, so a report of a removed run never opens.
#[must_use]
pub fn apply(root: Option<PathBuf>, trusted: bool, plugin: &str, ask: &Ask) -> Outcome {
    let mut outcome = Outcome::default();
    let root = match guard::admit(root, trusted) {
        Ok(root) => root,
        Err(refusal) => {
            outcome.open_refused = ask.open.as_ref().map(|_| refusal);
            outcome.not_removed = ask
                .remove
                .iter()
                .map(|run| (run.clone(), refusal))
                .collect();
            return outcome;
        }
    };
    for run in &ask.remove {
        let removed = guard::run_folder(&root, plugin, run).and_then(|folder| {
            // `remove_dir_all` follows no link inside the folder.
            std::fs::remove_dir_all(folder).map_err(|_| Refusal::Io)
        });
        match removed {
            Ok(()) => outcome.removed.push(run.clone()),
            Err(refusal) => outcome.not_removed.push((run.clone(), refusal)),
        }
    }
    if let Some(rel) = &ask.open {
        match guard::report_file(&root, plugin, rel) {
            Ok(_) => {
                let entry = Entry {
                    root,
                    plugin: plugin.to_owned(),
                    rel: rel.clone(),
                };
                let title = rb_i18n::t_with(TITLE_KEY, &[("name", entry.name())]);
                let url = rb_util::lock(&TOKENS).issue(entry);
                outcome.open = Some(ReportOpen {
                    url,
                    title,
                    placement: ask.window,
                });
            }
            Err(refusal) => outcome.open_refused = Some(refusal),
        }
    }
    outcome
}

/// Plugin `plugin`'s `answer` without `$report`, after what it asked for was done: the report
/// window opened, the run folders deleted, the outcome in the answer's `window` and `removed`.
/// A plugin that may not ask gets its answer back without the key and nothing done.
pub fn take(core: &Core, runtime: &PluginRuntime, plugin: &str, answer: Value) -> Value {
    take_noted(core, runtime, plugin, answer).0
}

/// [`take`], and whether something was done that the person sees (a report window, a notice):
/// a command needs no "done" then.
pub fn take_noted(
    core: &Core,
    runtime: &PluginRuntime,
    plugin: &str,
    answer: Value,
) -> (Value, bool) {
    let (mut answer, ask) = ask::split(answer);
    let Some(ask) = ask else {
        return (answer, false);
    };
    let source = runtime
        .plugins()
        .iter()
        .find(|served| served.id == plugin)
        .map(|served| served.source);
    if !source.is_some_and(|source| ask::may_ask(plugin, source)) {
        tracing::debug!(
            plugin_id = plugin,
            "a plugin asked for its reports and may not; ignored"
        );
        return (answer, false);
    }
    let host = core.host();
    let outcome = apply(
        host.project_root(),
        host.shell_state().trusted,
        plugin,
        &ask,
    );
    for (_, refusal) in &outcome.not_removed {
        // The code only: a run's name is the plugin's, its path never logged.
        tracing::info!(
            plugin_id = plugin,
            code = refusal.code(),
            "a run folder was not removed"
        );
    }
    if !outcome.removed.is_empty() {
        notify(
            core,
            NotificationLevel::Info,
            REMOVED_KEY,
            &outcome.removed.len().to_string(),
        );
    }
    if let Some(refusal) = outcome.open_refused {
        tracing::info!(
            plugin_id = plugin,
            code = refusal.code(),
            "a report was not opened"
        );
        notify(core, NotificationLevel::Warning, refusal.key(), "");
    }
    outcome.merge_into(&ask, &mut answer);
    let seen =
        outcome.open.is_some() || outcome.open_refused.is_some() || !outcome.removed.is_empty();
    if let Some(open) = outcome.open {
        core.ui().open_report(open);
    }
    (answer, seen)
}

fn notify(core: &Core, level: NotificationLevel, key: &str, count: &str) {
    let args = if count.is_empty() {
        BTreeMap::new()
    } else {
        BTreeMap::from([("count".to_owned(), count.to_owned())])
    };
    core.notify(CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args,
        settings_key: None,
    }));
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

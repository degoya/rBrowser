//! Findings a plugin reports itself (`findings.report`, Charge 6): one run of that plugin over
//! the window's views, held to the person's rule overrides and ignore patterns
//! (`rb_plugin_host::reported_run`), written under the project like every run and pushed to the
//! findings panel. Over the views of the last run it is merged into it - that plugin's part
//! replaced, the other plugins' kept - and the progress details follow ([`merge`], RB-094-14);
//! the file written is the plugin's run alone. Called from a plugin's host call: it answers at
//! once, the writing runs on the core's runtime.

use std::collections::BTreeMap;
use std::time::SystemTime;

use rb_engine::ViewHost as _;
use rb_ipc::CoreToShell;
use rb_plugin_host::{ChecksRunArgs, reported_run, write_run};
use rb_plugin_types::{Finding, FindingsFile};

use super::{LAST, merge, progress, run};
use crate::cdp::lock;
use crate::control::Core;

/// The run of `findings` reported by plugin `plugin`, over every view of `core`'s window.
pub fn run_of(core: &Core, plugin: &str, findings: Vec<Finding>) -> FindingsFile {
    let host = core.host();
    let views = host.views().iter().map(run::run_view).collect();
    let (project, user) = (host.config().checks.findings, core.user_settings().checks);
    let request = run::request(ChecksRunArgs::default(), views, SystemTime::now());
    reported_run(&run::settings(&project, &user), plugin, findings, request)
}

/// What the panel shows after `plugin`'s run `next`: merged into `last` (the run it shows, with
/// the texts of its keys) when both checked the same views, `next` alone otherwise; and whether
/// it merged.
pub fn shown_run(
    last: Option<(FindingsFile, BTreeMap<String, String>)>,
    next: &FindingsFile,
    plugin: &str,
    texts: impl Fn(&FindingsFile) -> BTreeMap<String, String>,
) -> (FindingsFile, BTreeMap<String, String>, bool) {
    if let Some((previous, mut messages)) = last
        && let Some(merged) = merge::plugin_run(&previous, next, plugin)
    {
        messages.extend(texts(next));
        return (merged, messages, true);
    }
    (next.clone(), texts(next), false)
}

/// Shows the run of `findings` in the findings panel and writes it under the project.
pub fn publish(core: &Core, plugin: &str, findings: Vec<Finding>) {
    let file = run_of(core, plugin, findings);
    let host = core.host();
    let (project, user) = (host.config().checks.findings, core.user_settings().checks);
    let merged = {
        // Read, merged and replaced in one step: two plugins reporting at once lose neither.
        let mut last = lock(&LAST);
        let previous = match last.as_ref() {
            Some(CoreToShell::Findings {
                run: shown,
                messages,
                ..
            }) => serde_json::from_value::<FindingsFile>(shown.clone())
                .ok()
                .map(|previous| (previous, messages.clone())),
            _ => None,
        };
        let (shown, messages, merged) = shown_run(previous, &file, plugin, |of| {
            run::messages(of, rb_i18n::plugin_texts)
        });
        match serde_json::to_value(&shown) {
            Ok(value) => {
                let message = CoreToShell::Findings {
                    run: value,
                    messages,
                    project_off: run::project_off(&project, &user),
                    partial: false,
                };
                core.send(&message);
                *last = Some(message);
            }
            Err(error) => tracing::warn!(%error, "a plugin's findings cannot be shown"),
        }
        merged
    };
    progress::follow_reported(core, &file, plugin, merged);
    if let Some(root) = host.project_root() {
        let keep = run::settings(&project, &user).keep_runs();
        core.runtime().spawn(async move {
            if let Err(failure) = write_run(&root, &file, keep).await {
                tracing::warn!(%failure, "a plugin's findings were not written");
            }
        });
    }
}

#[cfg(test)]
#[path = "reported_tests.rs"]
mod tests;

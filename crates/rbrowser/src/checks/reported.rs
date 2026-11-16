//! Findings a plugin reports itself (`findings.report`, Charge 6): one run of that plugin over
//! the window's views, held to the person's rule overrides and ignore patterns
//! (`rb_plugin_host::reported_run`), written under the project like every run and pushed to the
//! findings panel. Called from a plugin's host call: it answers at once, the writing runs on the
//! core's runtime.

use std::time::SystemTime;

use rb_engine::ViewHost as _;
use rb_ipc::CoreToShell;
use rb_plugin_host::{ChecksRunArgs, reported_run, write_run};
use rb_plugin_types::{Finding, FindingsFile};

use super::{LAST, run};
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

/// Shows the run of `findings` in the findings panel and writes it under the project.
pub fn publish(core: &Core, plugin: &str, findings: Vec<Finding>) {
    let file = run_of(core, plugin, findings);
    let host = core.host();
    let (project, user) = (host.config().checks.findings, core.user_settings().checks);
    match serde_json::to_value(&file) {
        Ok(value) => {
            let message = CoreToShell::Findings {
                run: value,
                messages: run::messages(&file, rb_i18n::plugin_texts),
                project_off: run::project_off(&project, &user),
                partial: false,
            };
            core.send(&message);
            *lock(&LAST) = Some(message);
        }
        Err(error) => tracing::warn!(%error, "a plugin's findings cannot be shown"),
    }
    if let Some(root) = host.project_root() {
        let keep = run::settings(&project, &user).keep_runs();
        core.runtime().spawn(async move {
            if let Err(failure) = write_run(&root, &file, keep).await {
                tracing::warn!(%failure, "a plugin's findings were not written");
            }
        });
    }
}

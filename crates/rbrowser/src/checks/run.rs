//! One check run without the window: the settings of Checks > Findings, the views of the target
//! as the runner sees them, the check plugins among the loaded ones, the run written under the
//! project, and the texts of the message keys its findings name.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use rb_config::{FindingsSettings, RuleSetting};
use rb_engine::{Target, ViewId, ViewInfo};
use rb_plugin_host::{
    CheckRunner, CheckSource, ChecksRunArgs, ChecksSettings, LoadedEntry, LoadedPlugin, RunControl,
    RunOutcome, RunRequest, RunView, run_id, write_run,
};
use rb_plugin_types::{
    Capability, CheckContext, CheckTarget, Failure, Finding, FindingsFile, PluginKind, RuleInfo,
};

/// The plugins' arguments: `checks_run` passes none.
const NO_ARGS: &str = "{}";

/// The settings of one run: the project's `checks` keys over the user's, as `rb-plugin-host`
/// reads them.
pub fn settings(project: &FindingsSettings, user: &FindingsSettings) -> ChecksSettings {
    match serde_json::to_value(project.over(user)) {
        Ok(section) => ChecksSettings::from_value(Some(&section)),
        Err(error) => {
            tracing::warn!(%error, "the checks settings do not serialize; defaults apply");
            ChecksSettings::default()
        }
    }
}

/// The levels the settings in effect set on plugin `plugin`'s own rules, as JSON text - what
/// `settings.get` answers for [`rb_plugin_types::RULE_LEVELS_SETTING`] (RB-091-06).
pub fn rule_levels(project: &FindingsSettings, user: &FindingsSettings, plugin: &str) -> String {
    serde_json::to_string(&settings(project, user).rule_levels(plugin))
        .unwrap_or_else(|_| "{}".to_owned())
}

/// What `project` (the project's `checks` keys) turned off over `user`: the rules it switched
/// off that the user's settings run, and its ignore entries (which replace the user's).
pub fn project_off(project: &FindingsSettings, user: &FindingsSettings) -> rb_ipc::ProjectOff {
    let rules = project
        .rules
        .iter()
        .filter(|(id, level)| {
            **level == RuleSetting::Off && user.rules.get(*id) != Some(&RuleSetting::Off)
        })
        .map(|(id, _)| id.clone())
        .collect();
    rb_ipc::ProjectOff {
        rules,
        ignore: project.ignore.clone(),
    }
}

/// The views of a check target.
pub fn engine_target(target: CheckTarget) -> Target {
    match target {
        CheckTarget::All => Target::All,
        CheckTarget::View(id) => Target::View(ViewId(id)),
    }
}

/// What the runner knows of a view: its id, its breakpoint and its page - an `http(s)` one
/// only, so a blank or internal page is no URL to check.
pub fn run_view(info: &ViewInfo) -> RunView {
    let web = info.url.starts_with("https://") || info.url.starts_with("http://");
    RunView {
        id: info.id.0,
        breakpoint: info.breakpoint.clone(),
        url: web.then(|| info.url.clone()),
    }
}

/// The request for `args` over `views`, started at `now`.
pub fn request(args: ChecksRunArgs, views: Vec<RunView>, now: SystemTime) -> RunRequest {
    let since = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    RunRequest {
        run_id: run_id(since.as_millis()),
        started_at: since.as_secs(),
        target: args.target,
        views,
        url: args.url,
        plugins: args.plugins,
        args: NO_ARGS.to_owned(),
    }
}

/// The check plugins among `loaded` - a project plugin's only while the workspace is trusted
/// (`project_trusted`; review M2 of Charge 6); one granted `console-logs` gets the views'
/// buffers in its context's `args` ([`crate::plugins::console`]).
pub fn check_sources(loaded: &[LoadedEntry], project_trusted: bool) -> Vec<Arc<dyn CheckSource>> {
    crate::plugins::runtime::runnable(loaded, |entry| entry.source, project_trusted)
        .filter(|entry| entry.plugin.manifest().is(PluginKind::Check))
        .map(|entry| {
            let plugin = entry.plugin.clone();
            if plugin.manifest().grants.has(Capability::ConsoleLogs) {
                Arc::new(Fed(plugin)) as Arc<dyn CheckSource>
            } else {
                Arc::new(plugin) as Arc<dyn CheckSource>
            }
        })
        .collect()
}

/// A check plugin whose context carries the console feed.
struct Fed(LoadedPlugin);

#[async_trait]
impl CheckSource for Fed {
    fn plugin_id(&self) -> &str {
        &self.0.manifest().id
    }

    async fn describe(&self) -> Result<Vec<RuleInfo>, Failure> {
        self.0.describe().await
    }

    async fn run(&self, mut ctx: CheckContext) -> Result<Vec<Finding>, Failure> {
        ctx.args = crate::plugins::console::with_current_feed(&ctx.args);
        self.0.run_check(&ctx).await
    }
}

/// Runs `sources` as `request` says - telling `control`'s sink as it goes, stopped by its
/// cancel - and writes the run under the project at `root` (`None` writes nothing): the run with
/// how each job ended, and why it was not written.
pub async fn execute(
    sources: &[Arc<dyn CheckSource>],
    settings: &ChecksSettings,
    request: RunRequest,
    root: Option<&Path>,
    control: &RunControl,
) -> (RunOutcome, Option<Failure>) {
    let outcome = CheckRunner::new(settings)
        .run_with(sources, request, control)
        .await;
    let not_written = match root {
        Some(root) => write_run(root, &outcome.file, settings.keep_runs())
            .await
            .err(),
        None => None,
    };
    (outcome, not_written)
}

/// The texts of the message keys `file`'s findings name, unformatted, each from the locales of a
/// plugin that reported the finding (`texts` gives one plugin's texts by key), and of the failure
/// codes a plugin gave from its own locales (the core's `plugins.error.*` are the shell's); a
/// key no reporting plugin has is left out, the panel then shows the key.
pub fn messages(
    file: &FindingsFile,
    texts: impl Fn(&str) -> BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut of_plugin: BTreeMap<&str, BTreeMap<String, String>> = BTreeMap::new();
    let mut messages = BTreeMap::new();
    for finding in &file.findings {
        let key = &finding.message.key;
        if messages.contains_key(key) {
            continue;
        }
        for plugin in &finding.plugins {
            let known = of_plugin
                .entry(plugin.as_str())
                .or_insert_with(|| texts(plugin));
            if let Some(text) = known.get(key) {
                messages.insert(key.clone(), text.clone());
                break;
            }
        }
    }
    for failure in &file.failures {
        let Some(code) = failure.code.as_ref() else {
            continue;
        };
        if code.starts_with("plugins.") || messages.contains_key(code) {
            continue;
        }
        let known = of_plugin
            .entry(failure.plugin.as_str())
            .or_insert_with(|| texts(&failure.plugin));
        if let Some(text) = known.get(code) {
            messages.insert(code.clone(), text.clone());
        }
    }
    messages
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;

//! The plugins' commands in the one registry (RB-060-12): every `[[commands]]` entry of a served
//! feature plugin as `plugin.<id>.<command>` - in the palette, under the leader group its
//! manifest names (default `X`, a free `X <key>` when it brings no chord), behind its shortcut
//! when the reservation list and the other commands leave it free - and run through the
//! plugin's `feature.run-command`. `plugins.panels` opens the panels window; a command's answer
//! may ask for one of its plugin's panels ([`super::open`]). A command that opened no panel has
//! no effect the host can see, so its end is named in the status bar, and a command of the
//! Checks group also opens the findings, where its findings went (RB-080-18) - in the panels
//! page (RB-080-69), a dock folded to its rail opened, its end naming how many it found
//! (RB-099-25).

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, Weak};

use rb_commands::{CommandError, CommandRegistry, Group, Handler};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_plugin_host::{LoadedEntry, plugin_command_defs, valid_plugin_id};
use rb_plugin_types::{Failure, PluginKind, codes};
use serde_json::{Value, json};

use super::open::{select_message, split_open};
use super::runtime::PluginRuntime;
use crate::analyzer::plugins::block_on;
use crate::control::Core;
use crate::shell::page_url;

/// The route of the panels window.
pub const ROUTE_PANELS: &str = "panels";
/// The status message after a plugin command that opened no panel (`{command}`: its title).
pub const COMMAND_DONE_KEY: &str = "plugins.notice.command_done";
/// The status message after a plugin's check that reported its findings (`{command}`: its
/// title, `{count}`: the findings of this run), RB-099-25.
pub const CHECK_DONE_KEY: &str = "plugins.notice.check_done";
/// The status message when a plugin's check starts (`{command}`: its title), RB-090-85.
pub const COMMAND_STARTED_KEY: &str = "plugins.notice.command_started";

/// Registers the commands of every feature plugin among `loaded` with `core`'s registry.
pub fn register(core: &Arc<Core>, runtime: &Arc<PluginRuntime>, loaded: &[LoadedEntry]) {
    let me = Arc::downgrade(core);
    core.register_late(|registry| register_into(registry, &me, runtime, loaded.iter()));
}

/// A live change (RB-092-06), as one change of the registry the shell hears of once: the
/// commands of the plugins `gone` (removed or replaced) leave it with their shortcuts, then those
/// of the feature plugins among `loaded` named in `came` (added or replaced) come in. A plugin
/// that stayed keeps its commands and the chords the host gave them.
pub fn follow(
    core: &Arc<Core>,
    runtime: &Arc<PluginRuntime>,
    loaded: &[LoadedEntry],
    gone: &BTreeSet<String>,
    came: &BTreeSet<String>,
) {
    let me = Arc::downgrade(core);
    core.register_late(|registry| {
        for plugin in gone {
            let removed = registry.unregister_plugin(plugin);
            tracing::debug!(
                plugin_id = plugin.as_str(),
                removed = removed.len(),
                "a plugin's commands left the registry"
            );
        }
        let entries = loaded
            .iter()
            .filter(|entry| came.contains(&entry.plugin.manifest().id));
        register_into(registry, &me, runtime, entries)
    });
}

/// Adds the commands of the feature plugins among `entries` to `registry`; what it refused.
fn register_into<'a>(
    registry: &mut CommandRegistry,
    me: &Weak<Core>,
    runtime: &Arc<PluginRuntime>,
    entries: impl Iterator<Item = &'a LoadedEntry>,
) -> Vec<CommandError> {
    let mut refused = Vec::new();
    for entry in entries {
        let manifest = entry.plugin.manifest();
        if !manifest.is(PluginKind::Feature) {
            continue;
        }
        let (defs, notices) = plugin_command_defs(manifest);
        for notice in &notices {
            tracing::info!(
                plugin_id = manifest.id.as_str(),
                code = notice.code().unwrap_or_default(),
                "a plugin shortcut stays unbound"
            );
        }
        for def in defs {
            let command = def.id.as_str().rsplit('.').next().unwrap_or_default();
            // RB-099-10: a command of the views the grid shows, `$scope` when its call has none.
            let scope = manifest
                .commands
                .iter()
                .find(|entry| entry.id == command)
                .and_then(|entry| entry.scope.clone());
            let handler = handler(
                me.clone(),
                Arc::downgrade(runtime),
                &manifest.id,
                command,
                &def.title_key,
                def.group,
                scope,
            );
            if let Err(error) = registry.register_plugin(def, handler) {
                // Never silent: a command left out on a live replace keeps an old handler.
                tracing::warn!(
                    plugin_id = manifest.id.as_str(),
                    %error,
                    "a plugin command was not registered"
                );
                refused.push(error);
            }
        }
    }
    refused
}

fn handler(
    core: Weak<Core>,
    runtime: Weak<PluginRuntime>,
    plugin: &str,
    command: &str,
    title_key: &str,
    group: Group,
    scope: Option<rb_config::ViewScope>,
) -> Handler {
    let (plugin, command, title_key) =
        (plugin.to_owned(), command.to_owned(), title_key.to_owned());
    Arc::new(move |args: Value| {
        let closed = || CommandError::Failed("the window is closed".to_owned());
        let core = core.upgrade().ok_or_else(closed)?;
        let runtime = runtime.upgrade().ok_or_else(closed)?;
        let args = if args.is_null() { json!({}) } else { args };
        let args = crate::run_scope::with_default_scope(&args, scope.as_ref());
        let args = crate::run_scope::plugin_args(&core.host().config(), &args)
            .map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
        report_started(&core, &plugin, &title_key, group);
        let reports = crate::checks::reported::mark(&plugin);
        let (answer, noted) = block_on(runtime.run_command_noted(&plugin, &command, &args))
            .map_err(|error| CommandError::Failed(error.to_string()))?
            .map_err(|failure| command_error(&plugin, failure))?;
        // RB-091-02: a report it asks to open, run folders it asks to delete.
        let (answer, shown) = crate::report_view::take_noted(&core, &runtime, &plugin, answer);
        let (answer, panel_opened) = opened(&core, &runtime, &plugin, answer, group);
        // A command that said in the status bar what it did needs no "done" (RB-080-64: the
        // colour picker's "click a pixel, Esc to stop" was replaced by "Pick a colour: done"),
        // nor one that showed a report (RB-091-02).
        if !panel_opened && !noted && !shown {
            // A check names the findings of its own run (RB-099-25).
            let found = (group == Group::Checks)
                .then(|| crate::checks::reported::reported_since(&plugin, reports))
                .flatten();
            report_done(&core, &plugin, &title_key, group, found);
        }
        Ok(answer)
    })
}

/// `answer` without its `$open`, and whether the panels window was opened at the panel it asks
/// for (only when plugin `plugin` offers that panel) - as [`open_asked`] opens it for `group`.
fn opened(
    core: &Core,
    runtime: &PluginRuntime,
    plugin: &str,
    answer: Value,
    group: Group,
) -> (Value, bool) {
    let offered = super::ipc::panels_offered(core, runtime);
    let (answer, panel) = split_open(plugin, answer, &offered);
    let Some(panel) = panel else {
        return (answer, false);
    };
    open_asked(core, &json!({ "plugin": plugin, "panel": panel }), group);
    (answer, true)
}

/// A plugin command's `$open` for a command of `group`. A command of the Panels group shows its
/// panel (a colour picker's "Show the colour picker panel"): a person's ask from the Panels
/// menus, the palette or its shortcut - no MCP tool runs a plugin command - so a dock folded to
/// its rail opens, as the core panels' commands open it (RB-090-71, RB-090-82). Any other
/// command's panel is a side effect and keeps the fold ([`open_panels`]).
pub fn open_asked(core: &Core, args: &Value, group: Group) -> Value {
    if group == Group::Panels {
        open_panels_unfolded(core, args)
    } else {
        open_panels(core, args)
    }
}

/// Names the end of plugin `plugin`'s command titled `title_key` in the status bar - with the
/// `found` findings of this run when it reported them ([`CHECK_DONE_KEY`], 0 reads "nothing
/// found"). A command of the Checks group first opens the findings, where its findings went - in
/// the panels page (RB-080-69): a person ran it (no MCP tool runs a plugin command; a tool's
/// check answers with its findings), so a dock folded to its rail opens at them (RB-099-25). A
/// side effect - another group's `$open`, a plugin's own report - keeps the fold.
fn report_done(core: &Core, plugin: &str, title_key: &str, group: Group, found: Option<usize>) {
    if group == Group::Checks {
        crate::panel_dock::core_panels::open_unfolded(
            core,
            crate::panel_dock::core_panels::FINDINGS,
        );
    }
    match found {
        Some(count) => notify_command_with(
            core,
            CHECK_DONE_KEY,
            plugin,
            title_key,
            [("count".to_owned(), count.to_string())],
        ),
        None => notify_command(core, COMMAND_DONE_KEY, plugin, title_key),
    }
}

/// Names the start of plugin `plugin`'s check titled `title_key` in the status bar: a check
/// (Lighthouse, accessibility, SEO) runs for a while before its end is named (RB-090-85).
fn report_started(core: &Core, plugin: &str, title_key: &str, group: Group) {
    if group == Group::Checks {
        notify_command(core, COMMAND_STARTED_KEY, plugin, title_key);
    }
}

fn notify_command(core: &Core, message_key: &str, plugin: &str, title_key: &str) {
    notify_command_with(core, message_key, plugin, title_key, []);
}

/// [`notify_command`] with further arguments of the message beside `{command}`.
fn notify_command_with<const N: usize>(
    core: &Core,
    message_key: &str,
    plugin: &str,
    title_key: &str,
    more: [(String, String); N],
) {
    let title = rb_i18n::plugin_texts(plugin)
        .remove(title_key)
        .unwrap_or_else(|| title_key.to_owned());
    let mut args = std::collections::BTreeMap::from([("command".to_owned(), title)]);
    args.extend(more);
    core.notify(CoreToShell::Notification(Notification {
        level: NotificationLevel::Info,
        message_key: message_key.to_owned(),
        args,
        settings_key: None,
    }));
}

/// A plugin's failure as text: its code, then its parameters (`code key=value ...`), no English.
#[must_use]
pub fn failure_text(failure: &Failure) -> String {
    let mut text = failure.code().unwrap_or(codes::HOST_FAILED).to_owned();
    for (key, value) in &failure.params {
        let _ = write!(text, " {key}={value}");
    }
    text
}

/// The failure of plugin `plugin` as a sentence for the status bar, the log and an MCP answer
/// (RB-080-58): a `plugin.<plugin>.` code from that plugin's own catalogue, a host code
/// ([`rb_plugin_types::codes::ALL`]) from the core's, its parameters filled in. Any other code -
/// another plugin's, a core text that is no failure - is shown as [`failure_text`], so a plugin
/// never speaks with another's voice; a code without a text is logged once.
#[must_use]
pub fn failure_message(plugin: &str, failure: &Failure) -> String {
    let code = failure.code().unwrap_or(codes::HOST_FAILED);
    let own = code
        .strip_prefix("plugin.")
        .and_then(|rest| rest.strip_prefix(plugin))
        .is_some_and(|rest| rest.starts_with('.'));
    if own && let Some(text) = rb_i18n::plugin_text(plugin, code) {
        return rb_i18n::format_message(&text, &params(failure));
    }
    if !own {
        return host_failure_message(failure);
    }
    no_text(failure)
}

/// A failure of the host itself (a plugin's load, a host service) as a sentence: only the host
/// codes ([`rb_plugin_types::codes::ALL`]) read from the core catalogue.
#[must_use]
pub fn host_failure_message(failure: &Failure) -> String {
    let code = failure.code().unwrap_or(codes::HOST_FAILED);
    if codes::ALL.contains(&code) {
        let text = rb_i18n::t_with(code, &params(failure));
        if text != code {
            return text;
        }
    }
    no_text(failure)
}

fn params(failure: &Failure) -> Vec<(&str, &str)> {
    failure
        .params
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect()
}

/// [`failure_text`], the code logged the first time it has no text.
fn no_text(failure: &Failure) -> String {
    let code = failure.code().unwrap_or(codes::HOST_FAILED);
    if first_missing(code) {
        tracing::warn!(%code, "a plugin failure has no text");
    }
    failure_text(failure)
}

/// Whether `code` is reported missing for the first time in this process.
fn first_missing(code: &str) -> bool {
    static SEEN: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
    rb_util::lock(&SEEN).insert(code.to_owned())
}

/// A plugin's failure as a command error: [`failure_message`].
pub fn command_error(plugin: &str, failure: Failure) -> CommandError {
    CommandError::Failed(failure_message(plugin, &failure))
}

/// The plugin and panel `args` names, when both are well formed.
fn panel_of(args: &Value) -> Option<(&str, &str)> {
    let plugin = args["plugin"].as_str().filter(|id| valid_plugin_id(id))?;
    let panel = args["panel"].as_str().filter(|id| valid_panel_id(id))?;
    Some((plugin, panel))
}

/// The panels window's URL: at panel `panel` of plugin `plugin` when both are well formed.
#[must_use]
pub fn panels_url(args: &Value) -> String {
    match panel_of(args) {
        Some((plugin, panel)) => page_url(&format!("{ROUTE_PANELS}/{plugin}.{panel}")),
        None => page_url(ROUTE_PANELS),
    }
}

/// `[a-z][a-z0-9-]*`, at most 40 characters: a panel id as a manifest checks it.
fn valid_panel_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// A plugin command's `$open`: opens the panels window - or the docked panels, as `panels.dock`
/// says (RB-080-57), a folded dock kept folded - at a panel when `args` names one: a new page
/// from its route, one already open from the `select` event.
pub fn open_panels(core: &Core, args: &Value) -> Value {
    crate::panel_dock::show(core, panels_url(args));
    select_named(core, args)
}

/// The command `plugins.panels`, a person's (the Panels menus, the palette, its shortcut; no MCP
/// tool runs it): as [`open_panels`], but a dock folded to its rail opens (RB-090-71). A plugin
/// command's `$open` stays [`open_panels`] - a side effect keeps the fold.
pub fn open_panels_unfolded(core: &Core, args: &Value) -> Value {
    crate::panel_dock::show_unfolded(core, panels_url(args));
    select_named(core, args)
}

/// Selects the panel `args` names in an open panels page.
fn select_named(core: &Core, args: &Value) -> Value {
    if let Some((plugin, panel)) = panel_of(args) {
        core.send(&CoreToShell::PluginPanel {
            message: select_message(plugin, panel),
        });
    }
    Value::Null
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "commands_texts_tests.rs"]
mod texts_tests;

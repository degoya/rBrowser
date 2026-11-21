//! A plugin's progress in the activity bar (RB-093-01): `host.report-progress` of a running call
//! becomes the snapshot's `pluginProgress` - the plugin's name, `done` of `total`, and the
//! command that cancels it when its manifest has a `cancel` command ([`describe`]) - which the
//! shell shows as the activity bar's plugins entry.
//!
//! - **Throttled.** A call may report every few milliseconds (the width sweep reports each
//!   width); the shell hears of a step at most every [`THROTTLE`] per plugin. The first report
//!   of a call and its last step (`done == total`) are told at once; a step held back is kept,
//!   so the next told one or any other state push carries it.
//! - **Ended with the calls.** The entry goes when the plugin's last running call ends
//!   (`HostServices::call_ended`, the same span that ends a width lease): answered, failed,
//!   trapped, timed out or dropped. A cancel call beside a sweep ends first and leaves the
//!   sweep's entry.
//!
//! The bookkeeping ([`ProgressBoard`]) is apart from the shell state so it is tested alone; the
//! clock is the caller's.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use rb_ipc::PluginProgress;
use rb_plugin_host::PluginManifest;
use rb_plugin_types::PluginKind;

use crate::control::Core;

/// The least time between two steps of one plugin the shell hears of.
pub const THROTTLE: Duration = Duration::from_millis(250);

/// The command id a plugin's cancel command has in its manifest.
pub const CANCEL_COMMAND: &str = "cancel";

/// What the bar names of a plugin.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Described {
    /// Its display name.
    pub name: String,
    /// The registry command that cancels its call.
    pub cancel: Option<String>,
}

/// What a plugin's manifest gives the bar: its name (the translation `plugin.<id>.name` of its
/// locales when it has one) and its `cancel` command when it is a feature plugin, whose commands
/// are in the registry.
#[must_use]
pub fn describe(manifest: &PluginManifest) -> Described {
    let id = &manifest.id;
    let name = rb_i18n::plugin_text(id, &format!("plugin.{id}.name"))
        .unwrap_or_else(|| manifest.name.clone());
    let commands: Vec<&str> = manifest
        .commands
        .iter()
        .map(|command| command.id.as_str())
        .collect();
    describe_parts(id, &name, manifest.is(PluginKind::Feature), &commands)
}

/// [`describe`] of its parts: plugin `id` named `name`, a feature plugin or not, with the
/// commands `commands` (the last segment of each id).
#[must_use]
pub fn describe_parts(id: &str, name: &str, feature: bool, commands: &[&str]) -> Described {
    let cancel = (feature && commands.contains(&CANCEL_COMMAND)).then(|| {
        format!(
            "{}{CANCEL_COMMAND}",
            rb_commands::registry::plugin_command_prefix(id)
        )
    });
    Described {
        name: name.to_owned(),
        cancel,
    }
}

struct Entry {
    done: u32,
    total: u32,
    /// When the shell last heard of a step.
    told: Instant,
}

#[derive(Default)]
struct Inner {
    entries: BTreeMap<String, Entry>,
    /// The calls running per plugin.
    running: BTreeMap<String, u32>,
}

/// The progress of the plugins' running calls, by plugin id.
#[derive(Default)]
pub struct ProgressBoard {
    inner: Mutex<Inner>,
}

impl ProgressBoard {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A call of `plugin` started.
    pub fn started(&self, plugin: &str) {
        *self.lock().running.entry(plugin.to_owned()).or_default() += 1;
    }

    /// A call of `plugin` reports `done` of `total` at `now`: kept, and whether the shell is to
    /// hear of it now ([`THROTTLE`]). A report outside a call is ignored.
    pub fn report(&self, plugin: &str, done: u32, total: u32, now: Instant) -> bool {
        let mut inner = self.lock();
        if !inner.running.contains_key(plugin) {
            return false;
        }
        let done = done.min(total);
        let last = total > 0 && done == total;
        match inner.entries.get_mut(plugin) {
            None => {
                let entry = Entry {
                    done,
                    total,
                    told: now,
                };
                inner.entries.insert(plugin.to_owned(), entry);
                true
            }
            Some(entry) => {
                entry.done = done;
                entry.total = total;
                let due = last || now.saturating_duration_since(entry.told) >= THROTTLE;
                if due {
                    entry.told = now;
                }
                due
            }
        }
    }

    /// A call of `plugin` ended; whether its entry went with it (its last running call ended
    /// after a report), so the shell is to hear of it.
    pub fn ended(&self, plugin: &str) -> bool {
        let mut inner = self.lock();
        match inner.running.get_mut(plugin) {
            Some(count) if *count > 1 => {
                *count -= 1;
                false
            }
            Some(_) => {
                inner.running.remove(plugin);
                inner.entries.remove(plugin).is_some()
            }
            None => false,
        }
    }

    /// The entries as they stand, by plugin id, each named by `describe`.
    pub fn list(&self, describe: impl Fn(&str) -> Described) -> Vec<PluginProgress> {
        let steps: Vec<(String, u32, u32)> = self
            .lock()
            .entries
            .iter()
            .map(|(plugin, entry)| (plugin.clone(), entry.done, entry.total))
            .collect();
        // `describe` reads the locales and the plugins outside the lock.
        steps
            .into_iter()
            .map(|(plugin, done, total)| {
                let Described { name, cancel } = describe(&plugin);
                PluginProgress {
                    plugin,
                    name,
                    done,
                    total,
                    cancel,
                }
            })
            .collect()
    }
}

/// [`describe`] of loaded plugin `plugin`; its id alone while it is not loaded (any more).
#[must_use]
pub fn describe_loaded(plugin: &str) -> Described {
    crate::analyzer::plugins::loaded()
        .and_then(|loaded| {
            loaded
                .report
                .loaded
                .iter()
                .map(|entry| entry.plugin.manifest())
                .find(|manifest| manifest.id == plugin)
                .map(describe)
        })
        .unwrap_or_else(|| Described {
            name: plugin.to_owned(),
            cancel: None,
        })
}

/// The shell hears of `board` as it stands: the snapshot's `pluginProgress`.
pub fn publish(core: &Core, board: &ProgressBoard) {
    let list = board.list(describe_loaded);
    core.host()
        .update_shell_state(|shell| shell.plugin_progress = list);
}

#[cfg(test)]
#[path = "progress_tests.rs"]
mod tests;

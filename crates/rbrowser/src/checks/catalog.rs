//! The rules of the check plugins by name (RB-080-72), for the settings window's rule overrides
//! (`checks.rules`, `x-options: "check-rules"`): each rule as its qualified id
//! (`<plugin>.<rule>`, the key `checks.rules` takes) and its title in the plugin's locales,
//! after the plugin's name. A plugin's rules come from its `check.describe`, which runs in the
//! plugin, so they are enumerated once per [`CatalogKey`] - the project, the workspace trust and
//! the check plugins that may run - off the UI thread, and kept. Only the settings window's
//! `getSettingSources` asks ([`data`]); a key that changed (another project, trust granted or
//! revoked, other plugins) enumerates again, and until a list for the current key is there the
//! window gets `loading`. When it is, the windows hear `settingSourcesChanged`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use rb_ipc::CoreToShell;
use rb_plugin_types::{PluginKind, RuleInfo};
use serde_json::{Value, json};

use crate::cdp::lock;
use crate::control::Core;

/// One rule as the window offers it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleOption {
    /// The qualified id: what `checks.rules` takes.
    pub value: String,
    /// `<plugin name> - <rule title>`.
    pub label: String,
}

/// What a list of rules holds for: another key, another list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CatalogKey {
    pub project: Option<PathBuf>,
    pub trusted: bool,
    /// The ids of the check plugins that may run, sorted.
    pub plugins: Vec<String>,
}

/// The enumerated list and the one running.
#[derive(Debug, Default)]
pub struct State {
    pub done: Option<(CatalogKey, Vec<RuleOption>)>,
    pub running: Option<CatalogKey>,
}

/// What a request for `key` does.
#[derive(Debug, PartialEq, Eq)]
pub enum Next {
    /// The list for `key` is there.
    Ready(Vec<RuleOption>),
    /// It is being enumerated.
    Wait,
    /// Enumerate it now.
    Start,
}

/// What `state` gives for `key`: a list of another key is stale.
pub fn next(state: &State, key: &CatalogKey) -> Next {
    match &state.done {
        Some((done, options)) if done == key => Next::Ready(options.clone()),
        _ if state.running.as_ref() == Some(key) => Next::Wait,
        _ => Next::Start,
    }
}

static STATE: Mutex<State> = Mutex::new(State {
    done: None,
    running: None,
});

/// Clears `running` of [`STATE`] when the enumeration of its key ends - finished, cancelled or
/// panicked - so a later request can start again.
pub struct Running {
    key: CatalogKey,
}

impl Running {
    /// Marks `key` as running in `state`.
    pub fn start(state: &mut State, key: CatalogKey) -> Running {
        state.running = Some(key.clone());
        Running { key }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let mut state = lock(&STATE);
        if state.running.as_ref() == Some(&self.key) {
            state.running = None;
        }
    }
}

/// The options of `plugin`'s `rules`: each title from `texts` (the plugin's locales), the id
/// where the title has no text.
pub fn options_of(
    plugin: &str,
    plugin_name: &str,
    rules: &[RuleInfo],
    texts: &BTreeMap<String, String>,
) -> Vec<RuleOption> {
    rules
        .iter()
        .map(|rule| {
            let title = texts
                .get(&rule.title)
                .map_or(rule.id.as_str(), String::as_str);
            RuleOption {
                value: format!("{plugin}.{}", rule.id),
                label: format!("{plugin_name} - {title}"),
            }
        })
        .collect()
}

/// The window's data of `options`, open to a typed id; `loading` while they are enumerated.
pub fn data_of(options: &[RuleOption], loading: bool) -> Value {
    let options: Vec<Value> = options
        .iter()
        .map(|option| json!({ "value": option.value, "label": option.label }))
        .collect();
    if loading {
        json!({ "options": options, "open": true, "loading": true })
    } else {
        json!({ "options": options, "open": true })
    }
}

/// The rules for the settings window now: the list of the current key, else `loading` and the
/// enumeration started (once). `None` while the plugins still load.
pub fn data(core: &Core) -> Option<Value> {
    let loaded = crate::analyzer::plugins::loaded()?;
    let trusted = crate::plugins::list::trusted();
    let sources = super::run::check_sources(&loaded.report.loaded, trusted);
    let mut plugins: Vec<String> = sources
        .iter()
        .map(|source| source.plugin_id().to_owned())
        .collect();
    plugins.sort();
    let key = CatalogKey {
        project: crate::analyzer::plugins::project_root(),
        trusted,
        plugins,
    };
    let running = {
        let mut state = lock(&STATE);
        match next(&state, &key) {
            Next::Ready(options) => return Some(data_of(&options, false)),
            Next::Wait => return Some(data_of(&[], true)),
            Next::Start => Running::start(&mut state, key.clone()),
        }
    };
    let names: BTreeMap<String, String> = loaded
        .report
        .loaded
        .iter()
        .map(|entry| entry.plugin.manifest())
        .filter(|manifest| manifest.is(PluginKind::Check))
        .map(|manifest| (manifest.id.clone(), manifest.name.clone()))
        .collect();
    let me = core.arc();
    drop(core.runtime().spawn(async move {
        let _running = running;
        let mut options = Vec::new();
        for source in sources {
            let plugin = source.plugin_id().to_owned();
            match source.describe().await {
                Ok(rules) => {
                    let name = names.get(&plugin).map_or(plugin.as_str(), String::as_str);
                    options.extend(options_of(
                        &plugin,
                        name,
                        &rules,
                        &rb_i18n::plugin_texts(&plugin),
                    ));
                }
                Err(failure) => tracing::debug!(
                    plugin,
                    code = failure.code().unwrap_or_default(),
                    "a check plugin did not describe its rules"
                ),
            }
        }
        options.sort_by(|a, b| a.value.cmp(&b.value));
        lock(&STATE).done = Some((key, options));
        if let Some(core) = me {
            core.send(&CoreToShell::SettingSourcesChanged);
        }
    }));
    Some(data_of(&[], true))
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;

//! What the core adds to a setting's definition at runtime (RB-080-32): a field whose schema
//! says `"x-options": "dynamic"` gets its choices from the running core, not from the schema
//! file - today `engines.firefox.path`, the installed Firefox editions. The data goes into the
//! def's `dynamic` of the schema payload (`{ options: [{ value, label }], missing? }`), beside
//! `lockedKey`; the window merges the options like static `x-options`.
//!
//! A list or a map whose `x-options` names a source ([`SOURCES`], RB-080-72) gets the values of
//! that source by name - not with the schema, which the palette asks for too, but through
//! `getSettingSources` ([`sources`]), which the settings window asks again whenever the settings
//! change: the installed plugins (`plugins`), those with panels
//! (`plugin-panels`) or page scripts (`plugin-page-scripts`) - from the manifests of the last
//! load, verified packages only - the rules of the check plugins (`check-rules`,
//! `crate::checks::catalog`, `open`: a rule of a plugin not loaded may be typed), and the open
//! project's environments and breakpoints (`environments`, `breakpoints`). A source not known
//! yet (the plugins still loading) adds nothing, and the window takes typed values.

use serde_json::{Value, json};

use crate::control::Core;
use crate::engines::firefox::{PATH_KEY, installed};

/// Every dynamic field's data for `core`, by key.
pub fn fields(core: &Core) -> Vec<(&'static str, Value)> {
    let fields = vec![(PATH_KEY, installed::field_for(core))];
    // The list the answer carries is the last enumeration; the next one starts now, off this
    // (the UI) thread.
    installed::refresh_in_background(core);
    fields
}

/// The sources of known values a schema may name in `x-options` (the settings lint,
/// `scripts/lib/settings_schema.py`, holds the same list).
pub const SOURCES: [&str; 7] = [
    "plugins",
    "plugin-panels",
    "plugin-page-scripts",
    "check-rules",
    "environments",
    "breakpoints",
    "layouts",
];

/// What a source needs to know of a verified plugin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginFacts {
    pub id: String,
    pub name: String,
    pub panels: bool,
    pub page_scripts: bool,
}

/// The data of every source known now, by source, for `getSettingSources` - the settings window
/// asks with its schema and again on `settingsChanged` and `settingSourcesChanged`, so a pick
/// list follows the project while the window is open. The check rules are enumerated in the
/// plugins once per project, trust and plugin set (`crate::checks::catalog`); until then
/// `loading`.
pub fn sources(core: &Core) -> serde_json::Map<String, Value> {
    let mut sources = serde_json::Map::new();
    if let Some(loaded) = crate::analyzer::plugins::loaded() {
        let facts = plugin_facts(loaded);
        for source in SOURCES {
            if let Some(data) = plugin_options(&facts, source) {
                sources.insert(source.to_owned(), data);
            }
        }
    }
    let config = core.host().config();
    for source in SOURCES {
        if let Some(data) = config_options(&config, source) {
            sources.insert(source.to_owned(), data);
        }
    }
    if let Some(rules) = crate::checks::catalog::data(core) {
        sources.insert("check-rules".to_owned(), rules);
    }
    sources
}

/// The verified plugins of the last load, running or switched off.
fn plugin_facts(loaded: &crate::analyzer::plugins::LoadedPlugins) -> Vec<PluginFacts> {
    let of = |manifest: &rb_plugin_host::PluginManifest| PluginFacts {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        panels: !manifest.panels.is_empty(),
        page_scripts: !manifest.page_scripts.is_empty(),
    };
    let report = &loaded.report;
    report
        .loaded
        .iter()
        .map(|entry| of(entry.plugin.manifest()))
        .chain(report.disabled.iter().map(|entry| of(&entry.manifest)))
        .collect()
}

/// The options of plugin source `source` among `facts`, by id; `None` for another source.
pub fn plugin_options(facts: &[PluginFacts], source: &str) -> Option<Value> {
    let wanted: fn(&PluginFacts) -> bool = match source {
        "plugins" => |_| true,
        "plugin-panels" => |facts| facts.panels,
        "plugin-page-scripts" => |facts| facts.page_scripts,
        _ => return None,
    };
    let mut chosen: Vec<&PluginFacts> = facts.iter().filter(|facts| wanted(facts)).collect();
    chosen.sort_by(|a, b| a.id.cmp(&b.id));
    chosen.dedup_by(|a, b| a.id == b.id);
    let options: Vec<Value> = chosen
        .into_iter()
        .map(|facts| json!({ "value": facts.id, "label": facts.name }))
        .collect();
    Some(json!({ "options": options }))
}

/// The options of project source `source` (`environments`: id and URL, `breakpoints`: label
/// and id, `layouts`: the layout modes and the presets) of `config`; `None` for another source.
pub fn config_options(config: &rb_config::ProjectConfig, source: &str) -> Option<Value> {
    let options: Vec<Value> = match source {
        "environments" => config
            .environments
            .iter()
            .map(|environment| {
                json!({ "value": environment.id, "label": format!("{} - {}", environment.id, environment.url) })
            })
            .collect(),
        "breakpoints" => config
            .breakpoints
            .iter()
            .map(|breakpoint| {
                json!({ "value": breakpoint.id, "label": breakpoint.label.as_deref().unwrap_or(&breakpoint.id) })
            })
            .collect(),
        // The layout a flow run shows (`automation.flows.runLayout`, RB-090-84): the modes over
        // the preset shown, then the project's presets by id.
        "layouts" => rb_config::RUN_LAYOUTS
            .iter()
            .map(|mode| json!({ "value": mode, "label": mode }))
            .chain(
                config
                    .layouts
                    .presets
                    .keys()
                    .filter(|id| !rb_config::RUN_LAYOUTS.contains(&id.as_str()))
                    .map(|id| json!({ "value": id, "label": id })),
            )
            .collect(),
        _ => return None,
    };
    Some(json!({ "options": options }))
}

/// Puts `data` into the def of `key` in the schema `payload` (`defs[].dynamic`); a key the
/// payload does not list changes nothing.
pub fn add(payload: &mut Value, key: &str, data: Value) {
    let Some(defs) = payload.get_mut("defs").and_then(Value::as_array_mut) else {
        return;
    };
    if let Some(def) = defs
        .iter_mut()
        .find(|def| def.get("key").and_then(Value::as_str) == Some(key))
        .and_then(Value::as_object_mut)
    {
        def.insert("dynamic".to_owned(), data);
    }
}

#[cfg(test)]
#[path = "dynamic_tests.rs"]
mod tests;

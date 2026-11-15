//! Which plugins the runtime holds and which of them may run (split from `runtime.rs`).

use super::*;

/// The feature plugins among `loaded`, each with its page scripts' bridge.
pub fn plugins_of(loaded: &[LoadedEntry]) -> Vec<Plugin> {
    loaded
        .iter()
        .filter(|entry| entry.plugin.manifest().is(PluginKind::Feature))
        .map(|entry| {
            let manifest = entry.plugin.manifest();
            let files = Arc::clone(&entry.ui_files);
            Plugin {
                id: manifest.id.clone(),
                source: entry.source,
                component: Arc::new(entry.plugin.clone()),
                bridge: PageBridge::new(manifest, |path| files.get(path).cloned()).map(Arc::new),
                console: manifest
                    .grants
                    .has(rb_plugin_types::Capability::ConsoleLogs),
                network: manifest
                    .grants
                    .has(rb_plugin_types::Capability::NetworkRules),
                status: manifest.grants.has(rb_plugin_types::Capability::Status),
            }
        })
        .collect()
}

/// Whether a plugin from `source` may run now: a project plugin only while the open workspace
/// is trusted. Revoked or stale trust stops a project plugin entirely - its page scripts,
/// commands, MCP tools, panels, page requests and checks, and so the host services only a call
/// reaches (review finding M2 of Charge 6).
#[must_use]
pub fn may_run(source: PluginSource, project_trusted: bool) -> bool {
    source != PluginSource::Project || project_trusted
}

/// The `items` that [`may_run`] now, each item's source given by `source`.
pub fn runnable<'a, T>(
    items: &'a [T],
    source: impl Fn(&T) -> PluginSource + 'a,
    project_trusted: bool,
) -> impl Iterator<Item = &'a T> + 'a {
    items
        .iter()
        .filter(move |item| may_run(source(item), project_trusted))
}

/// The page scripts that run: every plugin's but those switched off (`off`, the setting
/// `plugins.pageScriptsDisabled`) and - without workspace trust - a project plugin's.
pub fn enabled_bridges(
    plugins: &[Plugin],
    off: &BTreeSet<String>,
    project_trusted: bool,
) -> Vec<Arc<PageBridge>> {
    runnable(plugins, |plugin| plugin.source, project_trusted)
        .filter(|plugin| !off.contains(&plugin.id))
        .filter_map(|plugin| plugin.bridge.clone())
        .collect()
}

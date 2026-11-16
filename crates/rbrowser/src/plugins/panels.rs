//! The plugins' panels (RB-060-12): which panels the shell offers, their files under
//! `rbrowser://plugin/<id>/<path>`, and the requests of the panels window (`pluginPanel`).
//!
//! A panel runs in an `<iframe sandbox="allow-scripts">` of the panels window: an opaque origin,
//! no `cefQuery` (only a shell page's main frame has it), no top navigation, popups or forms. Its
//! files carry a CSP without network ([`PANEL_CSP`]) - as a header with `sandbox allow-scripts`,
//! and as the document's first element - and the shell's own CSP lets the frame navigate to
//! nothing but `rbrowser://plugin`. It talks to its plugin only through the shell's bridge
//! (`ui/src/panels/bridge.ts`), which this module answers.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use rb_plugin_host::{LoadedEntry, PanelPlacement, PluginManifest};
use rb_plugin_types::PluginKind;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The host of the panels' files under the app's scheme.
pub const PANEL_HOST: &str = "plugin";
/// Every panel file's URL starts with this.
pub const PANEL_ORIGIN: &str = "rbrowser://plugin/";
/// What a panel document may load: its plugin's files and inline code, no network at all.
pub const PANEL_CSP: &str = "default-src 'none'; script-src 'self' 'unsafe-inline'; \
     style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; \
     media-src 'self' data:; connect-src 'none'; frame-src 'none'; worker-src 'none'; \
     form-action 'none'; base-uri 'none'";
/// The header form of [`PANEL_CSP`]: the document is sandboxed even when loaded on its own.
pub const PANEL_CSP_HEADER: &str = "default-src 'none'; script-src 'self' 'unsafe-inline'; \
     style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; \
     media-src 'self' data:; connect-src 'none'; frame-src 'none'; worker-src 'none'; \
     form-action 'none'; base-uri 'none'; sandbox allow-scripts";

/// One panel the shell offers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelInfo {
    pub plugin: String,
    pub panel: String,
    /// The title in the plugin's locales (the key when it has none).
    pub title: String,
    pub placement: PanelPlacement,
    /// The manifest's Lucide icon name for the side bar (RB-080-61), checked by the plugin
    /// host; absent when the manifest names none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// `rbrowser://plugin/<id>/<entry>`.
    pub url: String,
}

/// The URL of plugin `plugin`'s package member `path`.
#[must_use]
pub fn file_url(plugin: &str, path: &str) -> String {
    format!("{PANEL_ORIGIN}{plugin}/{path}")
}

/// The panels of the feature plugins of `manifests` but those `hidden`
/// (`plugins.panelsHidden`), with their titles from `texts`.
pub fn panels_of<'a>(
    manifests: impl IntoIterator<Item = &'a PluginManifest>,
    hidden: &BTreeSet<String>,
    texts: impl Fn(&str) -> String,
) -> Vec<PanelInfo> {
    manifests
        .into_iter()
        .filter(|manifest| manifest.is(PluginKind::Feature) && !hidden.contains(&manifest.id))
        .flat_map(|manifest| {
            manifest.panels.iter().map(|panel| PanelInfo {
                plugin: manifest.id.clone(),
                panel: panel.id.clone(),
                title: texts(&panel.title),
                placement: panel.placement,
                icon: panel.icon.clone(),
                url: file_url(&manifest.id, &panel.entry),
            })
        })
        .collect()
}

/// `html` with the CSP meta element in front of everything, so the parser puts it first in the
/// head and it holds before any of the plugin's elements.
#[must_use]
pub fn with_csp(html: &[u8]) -> Vec<u8> {
    let mut document =
        format!("<meta http-equiv=\"Content-Security-Policy\" content=\"{PANEL_CSP}\">\n")
            .into_bytes();
    document.extend_from_slice(html);
    document
}

/// One file a panel loads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelFile {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}

/// Every panel file of the feature plugins among `plugins` (each manifest with its `page/` and
/// `panel/` members), by URL; an HTML file with its CSP.
pub fn panel_files<'a>(
    plugins: impl IntoIterator<Item = (&'a PluginManifest, &'a BTreeMap<String, Vec<u8>>)>,
) -> BTreeMap<String, PanelFile> {
    let mut files = BTreeMap::new();
    for (manifest, ui_files) in plugins {
        if !manifest.is(PluginKind::Feature) || manifest.panels.is_empty() {
            continue;
        }
        for (path, bytes) in ui_files {
            if !path.starts_with("panel/") {
                continue;
            }
            let mime = crate::shell::mime_type(std::path::Path::new(path));
            let bytes = if mime == "text/html" {
                with_csp(bytes)
            } else {
                bytes.clone()
            };
            files.insert(file_url(&manifest.id, path), PanelFile { bytes, mime });
        }
    }
    files
}

/// Each loaded plugin's manifest with its `page/` and `panel/` members.
pub fn with_files(
    loaded: &[LoadedEntry],
) -> impl Iterator<Item = (&PluginManifest, &BTreeMap<String, Vec<u8>>)> {
    loaded
        .iter()
        .map(|entry| (entry.plugin.manifest(), entry.ui_files.as_ref()))
}

/// The panel files of the bundled plugins, built once: they live as long as the process, so
/// the browser may read them in place.
static FILES: OnceLock<BTreeMap<String, PanelFile>> = OnceLock::new();

/// The file at `url` (query and fragment ignored), for the scheme handler; `None` for anything
/// else.
pub fn resource(url: &str) -> Option<&'static PanelFile> {
    let path = url.split(['?', '#']).next().unwrap_or_default();
    if !path.starts_with(PANEL_ORIGIN) {
        return None;
    }
    FILES
        .get_or_init(|| {
            panel_files(with_files(
                &crate::analyzer::plugins::bundled().report.loaded,
            ))
        })
        .get(path)
}

/// A request of the panels window.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PanelRequest {
    /// The panels offered: `{"panels": [PanelInfo]}`.
    List,
    /// A command or a tool of `plugin`, for one of its panels: `{"value": <answer>}`.
    Call {
        plugin: String,
        #[serde(default)]
        command: Option<String>,
        #[serde(default)]
        tool: Option<String>,
        #[serde(default)]
        args: Value,
    },
    /// The plugin's texts and settings: `{"strings": {...}, "settings": {...}}`.
    Settings { plugin: String },
    /// The installed plugins, for Settings > Plugins (RB-080-51): `{"loading", "plugins":
    /// [PluginRow], "folders"}` ([`super::list`]). A read: it installs and trusts nothing.
    Installed,
    /// The page shows panel `panel` of plugin `plugin` now, so the panels dock, undock or move
    /// with it (RB-080-57): `{"value": null}`. It opens and runs nothing.
    Selected { plugin: String, panel: String },
    /// The page shows the core's panel `panel` now (`findings`, RB-080-69); as `selected`.
    SelectedCore { panel: String },
}

/// What a call names, checked: exactly one of a command and a tool, arguments within `limit`
/// bytes of JSON (`plugins.panelMessageMaxBytes`, RB-090-44).
///
/// # Errors
///
/// The reason as text; over the limit the failure's code and parameters, naming the setting.
pub fn checked_call(
    command: Option<String>,
    tool: Option<String>,
    args: &Value,
    limit: usize,
) -> Result<Call, String> {
    if args.to_string().len() > limit {
        return Err(super::commands::failure_text(
            &super::panel_limit::too_large(limit),
        ));
    }
    match (command, tool) {
        (Some(command), None) => Ok(Call::Command(command)),
        (None, Some(tool)) => Ok(Call::Tool(tool)),
        _ => Err("a call names exactly one of `command` and `tool`".to_owned()),
    }
}

/// A checked call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    Command(String),
    Tool(String),
}

/// The panel event of plugin `plugin` (`event` of [`super::runtime::PanelSink`]) as the
/// `pluginPanel` event's message.
#[must_use]
pub fn event_message(plugin: &str, event: Value) -> Value {
    let mut event = event;
    if let Value::Object(fields) = &mut event {
        fields.insert("plugin".to_owned(), Value::String(plugin.to_owned()));
    }
    json!({ "event": event })
}

/// A panel's settings, `settings` (the plugin's own keys), with the app's theme as `ui.theme`
/// (`system`, `light`, `dark`), so a panel follows the app and not only the system (RB-080-62).
#[must_use]
pub fn with_theme(settings: Value, theme: rb_config::Theme) -> Value {
    let mut fields = match settings {
        Value::Object(fields) => fields,
        _ => serde_json::Map::new(),
    };
    fields.insert(
        "ui.theme".to_owned(),
        serde_json::to_value(theme).unwrap_or(Value::Null),
    );
    Value::Object(fields)
}

#[cfg(test)]
#[path = "panels_tests.rs"]
mod tests;

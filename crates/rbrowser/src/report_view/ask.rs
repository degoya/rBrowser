//! A plugin's answer asks for its reports (RB-091-02): `{"$report": {"open": "<run>/<file>.html",
//! "window": "single" | "perReport", "remove": ["<run>", ...]}}`. The paths are relative to the
//! plugin's own `.rbrowser/out/<plugin-id>/`; [`super::guard`] checks them before anything
//! happens. Only the bundled plugins of [`PLUGINS`] may ask - a project or user plugin of the same
//! id cannot - and the key never reaches the caller (the panel, the command, MCP).

use rb_plugin_host::PluginSource;
use serde_json::Value;

/// The answer key.
pub const REPORT_KEY: &str = "$report";
/// The plugins that may ask: their out folders hold HTML reports.
pub const PLUGINS: &[&str] = &["lighthouse"];
/// The most run folders one answer removes.
pub const MAX_REMOVE: usize = 64;

/// Where a report opens (the plugin's setting `lighthouse.reportWindow`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Placement {
    /// The one report window, which shows the newest report asked for.
    #[default]
    Single,
    /// A window of its own per report, so two stand side by side.
    PerReport,
}

/// What an answer asks for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ask {
    /// The report to show.
    pub open: Option<String>,
    pub window: Placement,
    /// The run folders to delete (retention, the history's delete).
    pub remove: Vec<String>,
}

/// Whether plugin `plugin` from `source` may ask.
#[must_use]
pub fn may_ask(plugin: &str, source: PluginSource) -> bool {
    source == PluginSource::Bundled && PLUGINS.contains(&plugin)
}

/// `answer` without [`REPORT_KEY`], and what it asked for when well formed.
#[must_use]
pub fn split(answer: Value) -> (Value, Option<Ask>) {
    let mut answer = answer;
    let Some(asked) = answer
        .as_object_mut()
        .and_then(|fields| fields.remove(REPORT_KEY))
    else {
        return (answer, None);
    };
    let ask = parse(&asked);
    if ask.is_none() {
        tracing::debug!("a plugin answer's `$report` has no report or run; ignored");
    }
    (answer, ask)
}

fn parse(asked: &Value) -> Option<Ask> {
    let fields = asked.as_object()?;
    let open = fields
        .get("open")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let window = match fields.get("window").and_then(Value::as_str) {
        Some("perReport") => Placement::PerReport,
        _ => Placement::Single,
    };
    let remove: Vec<String> = fields
        .get("remove")
        .and_then(Value::as_array)
        .map(|names| {
            names
                .iter()
                .filter_map(Value::as_str)
                .take(MAX_REMOVE)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    (open.is_some() || !remove.is_empty()).then_some(Ask {
        open,
        window,
        remove,
    })
}

#[cfg(test)]
#[path = "ask_tests.rs"]
mod tests;

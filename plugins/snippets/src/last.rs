//! The last run (RB-070-04): `.rbrowser/out/snippets/last.json`, so `plugin.snippets.run_last`
//! (`Ctrl+Shift+Space D J`) runs the same snippet in the same views again. It names the snippet -
//! source and name, never its code - so a repeat goes through the catalogue and its trust rules
//! again; a planted file can only name a snippet the person could run anyway.

use rb_plugin_api::{PluginHost, ViewId};
use serde_json::{Value, json};

use crate::catalog::Source;
use crate::header::{self, MAX_NAME_CHARS};

/// The last run, relative to the plugin's out folder and to the project.
pub const LAST_FILE: &str = "last.json";
pub const LAST_PATH: &str = ".rbrowser/out/snippets/last.json";
/// The most views remembered.
pub const MAX_VIEWS: usize = 64;

/// A snippet and the views it ran in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Last {
    pub source: Source,
    pub name: String,
    pub views: Vec<ViewId>,
}

/// Writes `last`; a failure is logged, never answered (the run itself succeeded).
pub fn remember(host: &mut dyn PluginHost, last: &Last) {
    let views: Vec<ViewId> = last.views.iter().copied().take(MAX_VIEWS).collect();
    let text =
        json!({ "source": last.source.as_str(), "name": last.name, "views": views }).to_string();
    if let Err(failure) = host.out_write(LAST_FILE, text.as_bytes()) {
        let code = failure.code.unwrap_or_default();
        host.log("warn", &format!("the last run was not written: {code}"));
    }
}

/// The last run, when one was written and reads.
pub fn read(host: &mut dyn PluginHost) -> Option<Last> {
    let value: Value = serde_json::from_slice(&host.project_read(LAST_PATH).ok()?).ok()?;
    let source = value["source"].as_str().and_then(Source::parse)?;
    let name = value["name"]
        .as_str()
        .and_then(|name| header::line_text(name, MAX_NAME_CHARS))?;
    let views: Vec<ViewId> = value["views"]
        .as_array()?
        .iter()
        .take(MAX_VIEWS)
        .map(|id| id.as_u64().and_then(|id| ViewId::try_from(id).ok()))
        .collect::<Option<_>>()?;
    (!views.is_empty()).then_some(Last {
        source,
        name,
        views,
    })
}

#[cfg(test)]
#[path = "last_tests.rs"]
mod tests;

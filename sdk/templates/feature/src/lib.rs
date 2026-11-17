//! An rBrowser feature plugin: the command and MCP tool `count_views` answers how many views the
//! grid has and their widths.
//!
//! What the host promises a feature plugin (docs/plugins.md):
//!
//! - `run-command` and `call-tool` are called only with a command or tool the manifest declares;
//!   arguments and answers are JSON text, and an answer that is no JSON is refused.
//! - Every host function behind a capability the manifest does not grant is missing: a component
//!   that imports it is refused when it is installed.
//! - A call has a time and a fuel budget; a plugin that keeps failing is suspended.
//!
//! The logic lives here and is tested natively (`cargo test`); `src/guest.rs` only translates it
//! into the vocabulary of `wit/rbrowser-plugin.wit` and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

/// One view as the logic needs it.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub id: u32,
    pub width: u32,
}

/// The answer of `count_views`: `{"count": 2, "widths": [375, 1440]}`, widths sorted.
#[must_use]
pub fn count_views(views: &[View]) -> String {
    let mut widths: Vec<u32> = views.iter().map(|view| view.width).collect();
    widths.sort_unstable();
    let widths: Vec<String> = widths.iter().map(u32::to_string).collect();
    format!(
        "{{\"count\":{},\"widths\":[{}]}}",
        views.len(),
        widths.join(",")
    )
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

//! The source a view's documents get for one plugin: the configuration, the prologue
//! (`prologue.js`) and the plugin's page scripts, each in a function of its own that receives
//! `rbPlugin` and runs at its `run_at`, all in one strict function scope.

use std::fmt::Write as _;

use serde_json::json;

use super::{MAX_MESSAGE_BYTES, PAGE_TIMEOUT_MILLISECONDS, PageBridge};

/// The prologue, kept in its own file so it reads as JavaScript.
const PROLOGUE: &str = include_str!("prologue.js");

pub(super) fn page_source(bridge: &PageBridge, view: u32) -> String {
    let config = json!({
        "id": bridge.plugin_id,
        "view": view,
        "binding": bridge.binding,
        "receiver": bridge.receiver,
        "max": MAX_MESSAGE_BYTES,
        "timeout": PAGE_TIMEOUT_MILLISECONDS,
    });
    let mut source = format!("(() => {{\n\"use strict\";\nconst C = {config};\n{PROLOGUE}");
    for script in &bridge.scripts {
        // The JSON string is a JavaScript string literal; the newline before the closing brace
        // ends a line comment the script may end with.
        let _ = write!(
            source,
            "run({}, function (rbPlugin) {{\n{}\n}});\n",
            serde_json::Value::String(script.run_at.clone()),
            script.text
        );
    }
    source.push_str("})();\n");
    source
}

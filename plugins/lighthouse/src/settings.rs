//! The plugin's own setting (`settings.schema.json`, Settings > Checks > Lighthouse):
//! `lighthouse.throttling`, how Lighthouse slows network and CPU while it measures. Read through
//! `settings.get` and held to the schema again; a value that does not fit is the default.

use rb_plugin_api::PluginHost;

/// Lighthouse's throttling methods; the first is the default.
pub const THROTTLING: [&str; 3] = ["simulate", "devtools", "provided"];

/// `lighthouse.throttling`.
pub fn throttling(host: &mut dyn PluginHost) -> &'static str {
    let set = host
        .setting("throttling")
        .and_then(|text| serde_json::from_str::<String>(&text).ok());
    THROTTLING
        .iter()
        .find(|method| set.as_deref() == Some(**method))
        .copied()
        .unwrap_or(THROTTLING[0])
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;

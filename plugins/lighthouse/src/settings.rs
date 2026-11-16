//! The plugin's own settings (`settings.schema.json`, Settings > Checks > Lighthouse):
//! `lighthouse.throttling`, how Lighthouse slows network and CPU while it measures;
//! `lighthouse.reportWindow`, where a report opens; `lighthouse.keepRuns`, how many runs stay
//! (RB-091-02). Read through `settings.get` and held to the schema again; a value that does not
//! fit is the default.

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

/// Where a report opens: the one report window or a window per report; the first is the
/// default.
pub const REPORT_WINDOW: [&str; 2] = ["single", "perReport"];
/// The runs kept by default: every run (security review M1 of RB-091-02: an update never
/// prunes an existing history; only a number the person set does).
pub const KEEP_RUNS: u32 = 0;
/// The most runs the setting may keep.
pub const MAX_KEEP_RUNS: u32 = 500;

/// `lighthouse.reportWindow`.
pub fn report_window(host: &mut dyn PluginHost) -> &'static str {
    let set = host
        .setting("reportWindow")
        .and_then(|text| serde_json::from_str::<String>(&text).ok());
    REPORT_WINDOW
        .iter()
        .find(|window| set.as_deref() == Some(**window))
        .copied()
        .unwrap_or(REPORT_WINDOW[0])
}

/// `lighthouse.keepRuns`: 0 keeps every run.
pub fn keep_runs(host: &mut dyn PluginHost) -> u32 {
    host.setting("keepRuns")
        .and_then(|text| serde_json::from_str::<u32>(&text).ok())
        .filter(|keep| *keep <= MAX_KEEP_RUNS)
        .unwrap_or(KEEP_RUNS)
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;

//! The timing counters (RB-090-32, `rb_util::perf`) in the app: the report the MCP tool
//! `app_perf` and the diagnostics ZIP (`perf.json`) give, the command `debug.perf_counters`
//! (`Leader D B`), the setting `performance.counters` and the start-up time to the first view's
//! first load.
//!
//! The counters: `sync.wait`, `sync.replay.<engine>` (K1 for Chromium, K11 for Firefox and
//! WebKit), `sync.navigation`, `plugin.wait`, `plugin.call`, `ipc.query`, `startup.firstView`
//! (`rb_util::perf::names`). `scripts/perf.sh --counters` reads the report.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_config::PerformanceSettings;
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_util::perf::{self, Registry, Summary, names};
use serde_json::{Value, json};

use crate::control::Core;

/// The command: the counters now; `{ "reset"?: bool }` starts a measurement afresh after them.
pub const PERF_COUNTERS: &str = "debug.perf_counters";
/// The MCP tool of [`PERF_COUNTERS`].
pub const MCP_TOOL: &str = "app_perf";
/// The setting that switches the recording.
pub const COUNTERS_SETTING: &str = "performance.counters";
/// The i18n key of the notice the shell shows when the command runs from it.
pub const SUMMARY_KEY: &str = "perf.countersSummary";

/// The report of the process's counters.
pub fn report() -> Value {
    report_of(perf::global(), perf::mark_start().elapsed())
}

/// The report of `registry` at `uptime` since the process start.
pub fn report_of(registry: &Registry, uptime: Duration) -> Value {
    let counters: Vec<Value> = registry
        .snapshot()
        .iter()
        .map(|series| {
            json!({
                "name": series.name,
                "count": series.count,
                "window": series.window,
                "p50Ms": millis(series.p50),
                "p95Ms": millis(series.p95),
                "maxMs": millis(series.max),
                "meanMs": millis(series.mean),
            })
        })
        .collect();
    json!({
        "enabled": registry.enabled(),
        "uptimeMs": millis(uptime),
        "window": perf::WINDOW,
        "counters": counters,
    })
}

/// Milliseconds to two decimals.
fn millis(duration: Duration) -> f64 {
    (duration.as_secs_f64() * 100_000.0).round() / 100.0
}

/// The setting `performance.counters` into the recording.
pub fn apply_settings(settings: &PerformanceSettings) {
    perf::global().set_enabled(settings.counters);
}

/// Whether the process saw its first view load, recorded or not.
static FIRST_LOAD_SEEN: AtomicBool = AtomicBool::new(false);

/// A view finished a load: the first one of the process is its start-up time. A first load
/// while the counters are off records nothing, and no later load stands in for it.
pub fn first_view_loaded() {
    if FIRST_LOAD_SEEN.swap(true, Ordering::Relaxed) {
        return;
    }
    perf::global().record_once(names::STARTUP_FIRST_VIEW, perf::mark_start().elapsed());
}

/// The notice of the command run from the shell: the p95 of the Chromium replays, the shell
/// IPC and the plugin calls, and the start-up time; `-` for a counter without samples.
pub fn notification(snapshot: &[Summary]) -> CoreToShell {
    let p95 = |name: &str| {
        snapshot
            .iter()
            .find(|series| series.name == name)
            .map_or_else(
                || "-".to_owned(),
                |series| format!("{:.1}", millis(series.p95)),
            )
    };
    let args = BTreeMap::from([
        ("sync".to_owned(), p95(names::SYNC_REPLAY_CHROMIUM)),
        ("ipc".to_owned(), p95(names::IPC_QUERY)),
        ("plugin".to_owned(), p95(names::PLUGIN_CALL)),
        ("firstView".to_owned(), p95(names::STARTUP_FIRST_VIEW)),
    ]);
    CoreToShell::Notification(Notification {
        level: NotificationLevel::Info,
        message_key: SUMMARY_KEY.to_owned(),
        args,
        settings_key: Some(COUNTERS_SETTING.to_owned()),
    })
}

/// The definition: group DevTools & Debugging beside `debug.vitals` and `debug.resources`,
/// `Leader D B`, MCP `app_perf`; title key `commands.debug.perf_counters`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    Ok(vec![CommandDef {
        id: CommandId::new(PERF_COUNTERS)?,
        title_key: format!("commands.{PERF_COUNTERS}"),
        group: Group::Devtools,
        default_bindings: vec![Binding::Leader(Chord {
            keys: vec![Key::Char('D'), Key::Char('B')],
        })],
        mcp: McpCoverage::Covered(MCP_TOOL.to_owned()),
    }])
}

/// Registers [`PERF_COUNTERS`] with `registry`; returns what the registry refused (a test
/// holds it empty).
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| registry.register(def, handler(Arc::downgrade(core))).err())
        .collect()
}

/// The report; from the shell (no arguments) also the notice; `reset: true` empties the series
/// after the report.
fn handler(core: Weak<Core>) -> Handler {
    Arc::new(move |args: Value| {
        let reset = match args.get("reset") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(reset)) => *reset,
            Some(_) => {
                return Err(CommandError::InvalidArgs(
                    "`reset` is true or false".to_owned(),
                ));
            }
        };
        let mut answer = report();
        if args.is_null()
            && let Some(core) = core.upgrade()
        {
            core.notify(notification(&perf::global().snapshot()));
        }
        if reset {
            perf::global().reset();
        }
        answer["reset"] = json!(reset);
        Ok(answer)
    })
}

#[cfg(test)]
#[path = "counters_tests.rs"]
mod tests;

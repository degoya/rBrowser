//! The recorder in the app (RB-050-01): [`HostRecorder`] runs `rb_sync::recorder::Recorder` for
//! one window. While a recording runs it hears the SyncHub (`rb_sync::Observer`): the active
//! view's captured events with the views they were replayed into, and each replay's outcome
//! (`onlyIn`). The toolbar shows a recording indicator (`ui/src/recorder/RecordIndicator.vue`)
//! from the shell's state; a stop writes `.rbrowser/out/recordings/<name>.json`.
//!
//! | Part | Where |
//! | --- | --- |
//! | Model, clean-up, secrets, store, exporter boundary (no runtime) | `rb_sync::recorder` |
//! | The window's recorder, its view and navigation feed, the indicator | [`glue`] |
//! | Commands `recorder.*` (group `F`) | [`run_command`] |
//! | MCP `recorder_start`, `recorder_stop` (RB-030-01 registers them) | [`mcp`] |
//! | Export through the `recorder-export` plugin, MCP `recorder_export` (RB-050-02) | [`export`] |
//! | Check steps from the page menu, `recorder.check` (RB-090-60) | [`check`] |
//! | The Recorder panel: list, edit, save, export preview, `panels.recorder` (RB-090-60) | [`panel`] |
//! | A person's flow export into the flows folder, its trust kept (RB-090-66) | [`flow_file`] |
//! | Playing a recording through the flows runner, `recorder.play`, MCP `recorder_play` (RB-090-66) | [`play`] |
//! | The form autofill's fills as steps, its `record_fill` host post (RB-090-78) | [`autofill`] |
//!
//! Nothing starts with the app: the window's recorder is made on its first command, on the
//! core's runtime (never `tokio::spawn` - a command may come from the CEF UI thread).

pub mod autofill;
pub mod check;
pub mod export;
pub mod flow_file;
pub mod glue;
pub mod mcp;
pub mod panel;
pub mod panel_export;
pub mod play;

use std::sync::{Arc, Mutex, Weak};

use rb_commands::CommandError;
use serde_json::Value;

use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;

pub use glue::HostRecorder;

/// `Leader F S`: start a recording (`{ "name"?: string }`), or resume a paused one.
pub const START: &str = "recorder.start";
/// `Leader F T`: stop the recording and write it.
pub const STOP: &str = "recorder.stop";
/// `Leader F P`: pause the recording, or resume it.
pub const PAUSE: &str = "recorder.pause";
pub use check::CHECK;
pub use export::{EXPORT_FLOW, EXPORT_SPEC};
pub use play::PLAY;
/// `Leader F V`: the Recorder panel - the recordings, their steps, the export (RB-090-60).
pub const PANEL: &str = "panels.recorder";

/// The recorder of every live window, by host.
fn registry() -> &'static Mutex<Vec<(Weak<Host>, Arc<HostRecorder>)>> {
    static REGISTRY: Mutex<Vec<(Weak<Host>, Arc<HostRecorder>)>> = Mutex::new(Vec::new());
    &REGISTRY
}

/// The recorder of `host`'s window, once one was made.
pub fn for_host(host: &Host) -> Option<Arc<HostRecorder>> {
    let mut entries = lock(registry());
    entries.retain(|(weak, _)| weak.strong_count() > 0);
    entries
        .iter()
        .find(|(weak, _)| std::ptr::eq(weak.as_ptr(), host))
        .map(|(_, recorder)| recorder.clone())
}

/// The recorder of `core`'s window; made on first use.
pub fn recorder_of(core: &Core) -> Arc<HostRecorder> {
    let mut entries = lock(registry());
    entries.retain(|(weak, _)| weak.strong_count() > 0);
    if let Some((_, recorder)) = entries
        .iter()
        .find(|(weak, _)| std::ptr::eq(weak.as_ptr(), Arc::as_ptr(core.host())))
    {
        return recorder.clone();
    }
    let recorder = HostRecorder::new(core.host(), core.runtime());
    entries.push((Arc::downgrade(core.host()), recorder.clone()));
    recorder
}

/// Runs the command `id` when it is a `recorder.*` command; `None` otherwise.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    let export = match id {
        EXPORT_FLOW => Some(export::ExportFormat::Flow),
        EXPORT_SPEC => Some(export::ExportFormat::Spec),
        _ => None,
    };
    if let Some(format) = export {
        return Some(export::run_export_command(core, format, args).and_then(to_value));
    }
    let answer = match id {
        START => mcp::recorder_start(core, args),
        STOP => mcp::recorder_stop(core, &Value::Null),
        PAUSE => mcp::recorder_stop(core, &serde_json::json!({ "pause": true })),
        CHECK => return Some(check::run(core, args)),
        PLAY => return Some(play::run(core, args)),
        PANEL => {
            // A person's (RB-090-71): a folded dock opens.
            let recorder = crate::panel_dock::core_panels::RECORDER;
            crate::panel_dock::core_panels::open_unfolded(core, recorder);
            return Some(Ok(Value::Null));
        }
        _ => return None,
    };
    Some(answer.and_then(to_value))
}

fn to_value<T: serde::Serialize>(answer: T) -> Result<Value, CommandError> {
    serde_json::to_value(answer).map_err(|error| CommandError::Failed(error.to_string()))
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

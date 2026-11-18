//! The extension points of wave 0.1-w1b (contract 0.1-w1b, `docs/contracts-0.1-w1b.md`): the
//! shared places - command handlers, IPC dispatch, start-up, config reload - hand each request
//! to the job modules in turn, and each module answers only for what is its own (`None`
//! otherwise). A job fills its module's bodies and never edits this file, `control/handlers.rs`
//! or `ipc.rs`; until it lands, its commands answer `NotImplemented` and its messages `501`.
//!
//! | Module | Job | Commands | IPC |
//! | --- | --- | --- | --- |
//! | [`crate::emulation`] | RB-010-05 | `emulation.*` | `getEmulation` |
//! | [`crate::devtools`] | RB-010-06 | `devtools.*` | - |
//! | [`crate::settings_window`] | RB-010-08 | `settings.open` | `openSettings`, `getSettings`, `setSetting`, `pickPath` |
//! | [`crate::trust`] | RB-010-09 | `trust.*` | `trustDecision`, `getTrust`, `revokeTrust` |
//! | [`crate::instance`] | RB-010-11 | `project.open_recent` | - |
//! | [`crate::presets`] | RB-010-12 | `layout.*`, `environments.*` | `selectLayoutPreset`, `selectEnvironment`, `getDevices` |
//! | [`crate::analyzer`] | RB-010-13 | `breakpoints.analyze`, `breakpoints.append_detected` | `analyzeBreakpoints`, `applyBreakpoints` |
//! | [`crate::breakpoint_keys`] | RB-010-16 | registered by the module itself | `setBreakpoint`, `listProjectShortcuts` |
//! | [`crate::about`] | RB-010-18 | `app.about` | `getAbout` |
//! | [`crate::reload`] | RB-030-05, RB-030-04 | `reload.all`, `reload.css_only`, `reload.toggle_pause`, `wsl.restart_helper` | - |
//! | [`crate::sessions`] | RB-020-07 | registered by the module itself | - |
//! | [`crate::bookmarks`] | RB-070-06 | registered by the module itself | - |
//! | [`crate::checks`] | RB-060-01 | `checks.run_all` | `runChecks`, `highlightFinding` |
//! | [`crate::capture`] | RB-050-03 | `capture.*` | - |
//! | [`crate::sync_hub`] | RB-020-02 | `sync.*` | `setSync` |
//! | [`crate::nav_sync`] | RB-020-06 | `sync.toggle_view_navigation`; `view.back`, `view.forward`, `view.navigate`, `reload.all` | - |
//! | [`crate::scroll_sync`] | RB-020-04 | `sync.toggle_scroll` | - |
//! | [`crate::form_sync`] | RB-020-05 | `sync.toggle_focus` | - |
//! | [`crate::updates`] | RB-090-02, RB-090-14 | registered by the module itself | - |
//! | [`crate::engines`] | RB-040-01 | `view.external.*`, `engine.restart` | - |
//! | [`crate::recorder`] | RB-050-01, RB-090-60 | `recorder.*`, `panels.recorder` | `getRecordings`, `getRecording`, `saveRecording`, `exportRecording` |
//! | [`crate::visual`] | RB-050-04 | `visual.*` | - |
//! | [`crate::flows`] | RB-040-07 | registered by the module itself | `listFlows` (RB-060-11), `getFlowRun` (RB-090-60) |
//! | [`crate::layout::canvas_board`] | RB-080-04 | - | `canvasDrag`, `canvasPan`, `canvasStep`, `canvasSizePick`, `canvasEnginePick`, `canvasPiece` (RB-090-69) |
//! | [`crate::engines::set_engine`] | RB-080-33 | `view.set_engine` | - |
//! | [`crate::first_run`] | RB-091-05 | registered by the module itself | `setup` |

use rb_commands::CommandError;
use rb_ipc::{CoreToShell, ShellToCore};
use serde_json::Value;

use crate::control::Core;
use crate::ipc::Failure;
use crate::{
    about, analyzer, breakpoint_keys, capture, checks, devtools, emulation, engines, form_sync,
    instance, nav_sync, presets, recorder, reload, sessions, settings_window, sync_hub, trust,
    updates, visual,
};

/// Runs the command `id` in the job module that owns it; `None` when none does.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    emulation::run_command(core, id, args)
        .or_else(|| devtools::run_command(core, id, args))
        .or_else(|| settings_window::run_command(core, id, args))
        .or_else(|| trust::run_command(core, id, args))
        .or_else(|| instance::run_command(core, id, args))
        .or_else(|| presets::run_command(core, id, args))
        .or_else(|| analyzer::run_command(core, id, args))
        .or_else(|| about::run_command(core, id, args))
        .or_else(|| reload::run_command(core, id, args))
        .or_else(|| checks::run_command(core, id, args))
        .or_else(|| capture::run_command(core, id, args))
        .or_else(|| sync_hub::run_command(core, id, args))
        .or_else(|| nav_sync::run_command(core, id, args))
        .or_else(|| crate::scroll_sync::run_command(core, id, args))
        .or_else(|| form_sync::run_command(core, id, args))
        .or_else(|| engines::run_command(core, id, args))
        .or_else(|| recorder::run_command(core, id, args))
        .or_else(|| visual::run_command(core, id, args))
        .or_else(|| crate::perf::run_command(core, id, args))
}

/// Answers the shell's `message` in the job module that owns it; `None` when none does.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    emulation::ipc(core, message)
        .or_else(|| settings_window::ipc(core, message))
        .or_else(|| trust::ipc(core, message))
        .or_else(|| presets::ipc(core, message))
        .or_else(|| analyzer::ipc(core, message))
        .or_else(|| breakpoint_keys::ipc(core, message))
        .or_else(|| about::ipc(core, message))
        .or_else(|| checks::ipc(core, message))
        .or_else(|| sync_hub::ipc(core, message))
        .or_else(|| crate::flows::palette::ipc(core, message))
        .or_else(|| crate::vitals::ipc(core, message))
        .or_else(|| crate::status::ipc(core, message))
        .or_else(|| crate::portal::app::ipc(core, message))
        .or_else(|| crate::layout::canvas_board::ipc(core, message))
        .or_else(|| crate::extensions::ipc(core, message))
        .or_else(|| crate::toolbar_rows::ipc(core, message))
        .or_else(|| crate::activity_bar::ipc(core, message))
        .or_else(|| crate::text_scale::ipc(core, message))
        .or_else(|| crate::video::panel::ipc(core, message))
        .or_else(|| recorder::panel::ipc(core, message))
        .or_else(|| crate::flows::panel::ipc(core, message))
        .or_else(|| crate::first_run::ipc(core, message))
}

/// The core is up and the project's grid is built: each module starts what it needs (the
/// trust check, the instance pipe, DevTools following the active view, the analyser's check
/// on open).
pub fn on_start(core: &Core) {
    crate::layout::active_label::on_start(core);
    // rBrowser's own theme before the window is seen (RB-080-95).
    crate::app_theme::on_start(core);
    // The shell follows Windows' text size (RB-090-26).
    crate::text_scale::on_start(core);
    crate::view_footer::on_start(core);
    // The panels' icon rail the window was made with, kept as the settings say (RB-080-93).
    crate::panel_dock::on_start(core);
    // The engine badge of every label strip (RB-080-33).
    engines::set_engine::on_start(core);
    // The vitals, engine window and address in every label strip (RB-090-27).
    crate::cell_info::on_start(core);
    emulation::on_start(core);
    presets::on_start(core);
    // Before the open project is recorded as a recent one: a settings file that exists then was
    // there before this start (RB-091-05).
    crate::first_run::on_start(core);
    instance::on_start(core);
    devtools::on_start(core);
    analyzer::on_start(core);
    breakpoint_keys::on_start(core);
    sync_hub::on_start(core);
    // After the SyncHub: navigation sync asks it who leads, and the sessions' submit policy
    // plugs into it (RB-020-07).
    nav_sync::on_start(core);
    sessions::policy::on_start(core);
    // After the breakpoints: a bookmark's shortcut yields to theirs (RB-070-06).
    crate::bookmarks::on_start(core);
    crate::mcp::on_start(core);
    crate::replay::on_start(core);
    crate::cdp_proxy::on_start(core);
    updates::on_start(core);
    engines::on_start(core);
    // After the SyncHub: the catch-up asks it who leads (RB-020-09).
    crate::perf::on_start(core);
    crate::vitals::on_start(core);
    // The Framework DevTools probe and hooks follow the user settings (RB-080-24).
    crate::sync_agent::framework_tasks::on_start(core);
    crate::cookies::on_start(core);
    // The trust check off the CEF UI thread - it hashes the protected folders (RB-090-36) - and
    // after it, on its thread, what follows the checked trust: a trusted project's extension
    // offer (RB-080-06) and, after the commands, the plugins, which join the registry once
    // they are loaded (RB-060-12).
    trust::on_start_then(core, |core| {
        crate::extensions::on_start(core);
        crate::plugins::on_start(core);
    });
}

/// The project config was read again after a change and applied to the grid.
pub fn on_config_changed(core: &Core) {
    trust::on_config_changed(core);
    emulation::on_config_changed(core);
    presets::on_config_changed(core);
    settings_window::on_config_changed(core);
    breakpoint_keys::on_config_changed(core);
    sessions::on_config_changed(core);
    crate::bookmarks::on_config_changed(core);
    // The scroll mode first: the hub's agents take the project's mode (RB-090-33).
    crate::scroll_sync::on_config_changed(core);
    sync_hub::on_config_changed(core);
    nav_sync::on_config_changed(core);
    engines::on_config_changed(core);
    crate::plugins::on_config_changed(core);
    crate::extensions::on_config_changed(core);
}

#[cfg(test)]
#[path = "features_tests.rs"]
mod tests;

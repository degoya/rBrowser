//! The core's side of the shell IPC (contract 0.1, `rb-ipc`): every `cefQuery` of the shell
//! comes here as text and gets one answer. The messages of the grid, the command registry, the
//! shortcut editor and the user settings (`getSettings`, until RB-010-08 answers it with the
//! schema) are handled here; the feature jobs answer theirs through `crate::features` (wave
//! 0.1-w1b), the rest answer `501` until their jobs plug in (RB-010-16, RB-020-02).

use std::sync::Arc;
use std::time::{Duration, Instant};

use rb_engine::{Target, ViewHost};
use rb_ipc::{
    BreakpointSummary, CoreToShell, IpcErrorCode, LayoutState, ProjectInfo, ShellToCore,
    StateSnapshot, SyncState, encode_response, event_script, parse_request,
};

use crate::control::Core;
use crate::host::Host;

#[path = "ipc_push.rs"]
pub(crate) mod push;

/// A failed query: the code and message for `onFailure`.
pub type Failure = (IpcErrorCode, String);

/// Answers one query of the shell through `reply`: `runCommand`, `setSetting`, `getSettings`,
/// the shortcut editor's changes and `listFlows` off the calling thread (the CEF UI thread; a
/// command such as `diagnostics.create` takes a while, a setting or a binding is written to a
/// file, the settings and the flows are read from disk - RB-090-36, RB-099-19), the rest at
/// once.
///
/// The time from here to the answer is the timing counter `ipc.query` (RB-090-32).
pub fn dispatch_to(
    core: &Core,
    request: &str,
    reply: impl FnOnce(Result<String, Failure>) + Send + 'static,
) {
    let arrived = Instant::now();
    let reply = move |answer: Result<String, Failure>| {
        rb_util::perf::record(rb_util::perf::names::IPC_QUERY, arrived.elapsed());
        reply(answer);
    };
    match parse_request(request) {
        Ok(ShellToCore::RunCommand { id, args }) => {
            core.run_command_async(&id, args, move |result| {
                reply(
                    result
                        .map_err(command_failure)
                        .and_then(|_| encode(&CoreToShell::Ack)),
                );
            });
        }
        // A plugin panel's request may wait for its plugin (RB-060-12).
        Ok(ShellToCore::PluginPanel { request }) => {
            crate::plugins::ipc::dispatch(core, request, reply);
        }
        // The Recorder panel reads and writes the disk and waits for the export plugin (RB-090-60).
        Ok(message) if crate::recorder::panel::is_off_thread(&message) => {
            crate::recorder::panel::dispatch_off_thread(core, message, move |answer| {
                reply(answer.and_then(|answer| encode(&answer)));
            });
        }
        // The sitemap gallery reads `run.json`, which may be large (RB-090-86).
        Ok(message) if crate::sitemap::gallery_page::is_gallery_request(&message) => {
            crate::sitemap::gallery_page::dispatch_off_thread(core, message, move |answer| {
                reply(answer.and_then(|answer| encode(&answer)));
            });
        }
        // The sitemap start page's discovery reads the site's sitemaps (RB-099-09).
        Ok(message) if crate::sitemap::discover::is_request(&message) => {
            crate::sitemap::discover::dispatch(core, message, move |answer| {
                reply(answer.and_then(|answer| encode(&answer)));
            });
        }
        // A setting's file is read and written on a blocking thread, in order (RB-080-27).
        Ok(ShellToCore::SetSetting { key, value, scope }) => {
            crate::settings_window::set_setting_off_thread(core, key, value, scope, |answer| {
                reply(answer.and_then(|answer| encode(&answer)));
            });
        }
        // The shortcut editor's changes write the user settings file: on the same lane
        // (RB-099-19).
        Ok(
            message @ (ShellToCore::SetBindings { .. }
            | ShellToCore::ResetBinding { .. }
            | ShellToCore::ImportBindings { .. }),
        ) => {
            crate::settings_window::change_off_thread(
                core,
                move |core| binding_change(core, message),
                move |answer| reply(answer.and_then(|answer| encode(&answer))),
            );
        }
        // Read on the same lane, after every write before it (RB-090-36).
        Ok(ShellToCore::GetSettings { include_schema }) => {
            crate::settings_window::get_settings_off_thread(core, include_schema, |answer| {
                reply(answer.and_then(|answer| encode(&answer)));
            });
        }
        // The credential store may wait for an unlock prompt (Keychain, Secret Service): the
        // Basic auth status and the sign-in dialog's answer never block the UI thread (review w35).
        Ok(message) if crate::basic_auth::status::is_off_thread(&message) => {
            crate::basic_auth::status::dispatch_off_thread(core, message, move |answer| {
                reply(answer.and_then(|answer| encode(&answer)));
            });
        }
        Ok(ShellToCore::ListFlows) => {
            crate::flows::palette::list_off_thread(core, |answer| {
                reply(answer.and_then(|answer| encode(&answer)));
            });
        }
        Ok(message) => reply(answer(core, message)),
        Err(error) => reply(Err((error.code(), error.to_string()))),
    }
}

/// Answers one query of the shell on this thread (a `runCommand` included).
pub fn dispatch(core: &Core, request: &str) -> Result<String, Failure> {
    let message = parse_request(request).map_err(|error| (error.code(), error.to_string()))?;
    answer(core, message)
}

fn answer(core: &Core, message: ShellToCore) -> Result<String, Failure> {
    let host = core.host();
    let failed = |error: rb_engine::EngineError| {
        let code = match error {
            rb_engine::EngineError::UnknownView(_) => IpcErrorCode::NotFound,
            rb_engine::EngineError::Invalid(_) => IpcErrorCode::BadRequest,
            rb_engine::EngineError::NotImplemented => IpcErrorCode::NotImplemented,
            _ => IpcErrorCode::Internal,
        };
        (code, error.to_string())
    };
    let answer = match message {
        ShellToCore::GetState => CoreToShell::State(Box::new(snapshot(host))),
        // Every view whose navigation is coupled (RB-020-06).
        ShellToCore::Navigate { url } => {
            crate::nav_sync::navigate(host, Target::All, &url).map_err(failed)?;
            CoreToShell::Ack
        }
        ShellToCore::Back => {
            crate::nav_sync::back(host, Target::All).map_err(failed)?;
            CoreToShell::Ack
        }
        ShellToCore::Forward => {
            crate::nav_sync::forward(host, Target::All).map_err(failed)?;
            CoreToShell::Ack
        }
        ShellToCore::Reload { ignore_cache } => {
            crate::nav_sync::reload(host, Target::All, ignore_cache).map_err(failed)?;
            CoreToShell::Ack
        }
        ShellToCore::SetActiveView { view } => {
            host.set_active_view(view).map_err(failed)?;
            CoreToShell::Ack
        }
        ShellToCore::RunCommand { id, args } => {
            core.run_command(&id, args).map_err(command_failure)?;
            CoreToShell::Ack
        }
        ShellToCore::ListCommands => core.commands(),
        ShellToCore::DismissPalette => {
            core.ui()
                .hide_page_window(crate::page_window::PageWindow::Palette);
            CoreToShell::Ack
        }
        message @ ShellToCore::GetSettings { include_schema } => {
            match crate::features::ipc(core, &message) {
                Some(answer) => answer?,
                // Until RB-010-08 answers with the schema: the user settings in effect.
                None => core.settings_answer(include_schema)?,
            }
        }
        message @ (ShellToCore::SetBindings { .. }
        | ShellToCore::ResetBinding { .. }
        | ShellToCore::ImportBindings { .. }) => binding_change(core, message)?,
        ShellToCore::CheckBinding { id, binding } => core.check_binding(&id, &binding)?,
        ShellToCore::ExportBindings => core.export_bindings(),
        // The feature jobs of wave 0.1-w1b answer their own messages (`crate::features`).
        other => match crate::features::ipc(core, &other) {
            Some(answer) => answer?,
            None => {
                return Err((
                    IpcErrorCode::NotImplemented,
                    format!("{other:?} is not handled yet"),
                ));
            }
        },
    };
    encode(&answer)
}

/// The shortcut editor's changes of the user's bindings, which write the user settings file.
fn binding_change(core: &Core, message: ShellToCore) -> Result<CoreToShell, Failure> {
    match message {
        ShellToCore::SetBindings { id, bindings } => core.set_bindings(&id, &bindings),
        ShellToCore::ResetBinding { id } => core.reset_binding(id.as_deref()),
        ShellToCore::ImportBindings { json } => core.import_bindings(&json),
        other => Err((
            IpcErrorCode::Internal,
            format!("{other:?} is no change of the bindings"),
        )),
    }
}

fn encode(answer: &CoreToShell) -> Result<String, Failure> {
    encode_response(answer).map_err(|error| (error.code(), error.to_string()))
}

fn command_failure(error: rb_commands::CommandError) -> Failure {
    let code = match error {
        rb_commands::CommandError::UnknownCommand(_) => IpcErrorCode::NotFound,
        rb_commands::CommandError::InvalidArgs(_) => IpcErrorCode::BadRequest,
        rb_commands::CommandError::NotImplemented => IpcErrorCode::NotImplemented,
        _ => IpcErrorCode::Internal,
    };
    (code, error.to_string())
}

/// The whole state the toolbar and the cell labels show; the config and the shell state are read
/// once.
pub fn snapshot(host: &Host) -> StateSnapshot {
    let config = host.config();
    let views = host.views();
    let active_view = host.active_view();
    let url = active_view
        .and_then(|active| views.iter().find(|view| view.id == active))
        .or_else(|| views.first())
        .map(|view| view.url.clone())
        .unwrap_or_default();
    let shell = host.shell_state();
    let project = host.project_root().map(|root| ProjectInfo {
        name: root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        root: root.to_string_lossy().into_owned(),
        // RB-010-09 decides; until then nothing trust-gated applies.
        trusted: shell.trusted,
    });
    // The size the shell shows is the resolved one: a side the breakpoint leaves out comes from
    // its device preset (RB-010-12).
    let devices = crate::emulation::device_list();
    let breakpoints = config
        .breakpoints
        .iter()
        .map(|breakpoint| {
            let (width, height) = breakpoint
                .viewport(&devices)
                .map_or((0, 0), |viewport| (viewport.width, viewport.height));
            BreakpointSummary {
                id: breakpoint.id.clone(),
                label: breakpoint
                    .label
                    .clone()
                    .unwrap_or_else(|| breakpoint.id.clone()),
                width,
                height,
                device: breakpoint.device.clone(),
            }
        })
        .collect();
    // From the config and shell state read above, not read again (RB-094-10).
    let hidden = crate::layout::view_edit::addable_in(&config, shell.preset.as_deref());
    let bookmarks = crate::bookmarks::summaries_in(&config, shell.environment.as_deref());
    StateSnapshot {
        project,
        views,
        active_view,
        url,
        environment: shell.environment,
        environments: config.environments.clone(),
        layout: LayoutState {
            preset: shell.preset,
            presets: config.layouts.presets.keys().cloned().collect(),
            mode: shell.mode,
            focus: shell.focus,
            modes: crate::layout::ipc::mode_state(host, &shell.modes),
            hidden,
            minimap: shell.modes.minimap,
        },
        // The SyncHub's state (RB-020-02); the project's switches before it runs.
        sync: shell.sync.unwrap_or_else(|| SyncState {
            enabled: true,
            features: config.sync.clone(),
            excluded: Vec::new(),
            desynced: Vec::new(),
            navigation_decoupled: Vec::new(),
            browsersync: None,
            skip_next_click: false,
        }),
        breakpoints,
        sessions: crate::sessions::session_infos(&config),
        detected: shell.detected,
        devtools: shell.devtools,
        // The external views (RB-040-01).
        externals: shell.externals,
        recorder: shell.recorder,
        bookmarks,
        // The last sitemap capture (RB-050-05).
        sitemap: shell.sitemap,
        // The cell labels' Web Vitals and resources (RB-060-10).
        metrics: shell.metrics,
        // The views' frameworks, for the DevTools offer (RB-080-24).
        frameworks: shell.frameworks,
        // The running video and the activity bar's place and folded entries (RB-090-67).
        video: shell.video,
        activity_bar: shell.activity_bar,
        // The views' clock and animation rates while changed (RB-094-07).
        time_control: shell.time_control,
        // The plugin calls that report progress (RB-093-01, `crate::plugins::services`).
        plugin_progress: shell.plugin_progress,
        // The views with forced pseudo-class states (RB-094-16, `crate::force_state`).
        forced_states: shell.forced_states,
        // The devices of the menus (RB-094-20, `crate::engines::android::menu`).
        devices: shell.devices,
    }
}

/// How long a burst of view events is gathered before the state is pushed once.
const COALESCE: Duration = Duration::from_millis(30);

/// Pushes the state to the shell (`send` gets the script) after view events, once per burst:
/// nothing when it is the state sent last, only the metrics when nothing else changed
/// ([`push::StatePush`], RB-090-36).
pub fn spawn_state_forwarder(
    host: &Arc<Host>,
    runtime: &tokio::runtime::Handle,
    send: impl Fn(String) + Send + Sync + 'static,
) {
    let mut events = host.subscribe();
    let weak = Arc::downgrade(host);
    runtime.spawn(async move {
        use tokio::sync::broadcast::error::RecvError;
        let mut pushed = push::StatePush::default();
        loop {
            match events.recv().await {
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => return,
            }
            tokio::time::sleep(COALESCE).await;
            while events.try_recv().is_ok() {}
            let Some(host) = weak.upgrade() else {
                return;
            };
            if push::take_forced(&host) {
                pushed.forget();
            }
            if let Some(script) = pushed
                .next(snapshot(&host))
                .and_then(|message| event_script(&message).ok())
            {
                send(script);
            }
        }
    });
}

/// The script that pushes the current state to the shell.
pub fn state_event(host: &Host) -> Option<String> {
    event_script(&CoreToShell::State(Box::new(snapshot(host)))).ok()
}

#[cfg(test)]
#[path = "ipc_tests.rs"]
mod tests;

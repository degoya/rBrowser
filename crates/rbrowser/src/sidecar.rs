//! The Node sidecar in the app (RB-040-03): one `rb_sidecar::Sidecar` per process, made when the
//! core starts and started only on the first request; changed user settings reach it at once
//! (the timeout) or at its next start (the paths, `sidecar.restart`); the commands
//! `sidecar.restart` and `engines.update`, each naming its start and its end in the status bar
//! (RB-090-85); stopped when the app quits; its versions in the
//! diagnostics ZIP when it runs (the diagnostics never start Node).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use rb_commands::{CommandDef, CommandError, CommandRegistry, Handler};
use rb_config::UserSettings;
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_sidecar::command::{restart_def, restart_handler, update_def, update_handler};
use rb_sidecar::playwright::{InstallProgress, InstallReport, PROGRESS_KEY};
use rb_sidecar::{ConfiguredLauncher, Sidecar, SidecarError, SidecarSettings};
use serde_json::Value;
use tokio::runtime::Handle;

use crate::cdp::lock;
use crate::control::Core;

/// How long the diagnostics wait for the running sidecar's versions.
const VERSIONS_WAIT: Duration = Duration::from_secs(2);
/// How long quitting waits for the sidecar to stop: its grace to end on its own after its stdin
/// closed, and the kill after it.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(5);
/// A download's progress is shown in steps of this many percent.
const PROGRESS_STEP: u8 = 25;
/// The notice that `sidecar.restart` started (RB-090-85).
pub const RESTARTING_KEY: &str = "sidecar.notice.restarting";
/// The notice that the sidecar restarted.
pub const RESTARTED_KEY: &str = "sidecar.notice.restarted";
/// The notice that `engines.update` started (RB-090-85).
pub const UPDATING_KEY: &str = "sidecar.notice.updating";

/// The sidecar of this process and the runtime its tasks run on.
struct Running {
    sidecar: Arc<Sidecar<ConfiguredLauncher>>,
    runtime: Handle,
}

/// Set when the core registers its commands; taken when the app quits.
static SIDECAR: Mutex<Option<Running>> = Mutex::new(None);

/// What the sidecar reads from the user settings.
pub fn settings_of(user: &UserSettings) -> SidecarSettings {
    SidecarSettings {
        node_path: user.engines.node_path.clone(),
        rpc_timeout_ms: user.engines.node_rpc_timeout_ms,
        browsers_path: user.engines.playwright_browsers_path.clone(),
        lighthouse_browser_path: user.engines.lighthouse_browser_path.clone(),
    }
}

/// The definitions of `sidecar.restart` and `engines.update`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    Ok(vec![restart_def()?, update_def()?])
}

/// Registers `sidecar.restart` and `engines.update`; returns what the registry refused. Without
/// a launcher (no executable path) both answer with the error; a bad browsers folder or Node
/// path fails the start, so fixing the setting is enough.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    let settings = settings_of(&core.user_settings());
    let launcher = ConfiguredLauncher::beside_executable(settings.clone());
    let (restart, update) = match launcher {
        Ok(launcher) => {
            let sidecar = Arc::new(Sidecar::new(launcher, settings.rpc_timeout()));
            let runtime = core.runtime().clone();
            *lock(&SIDECAR) = Some(Running {
                sidecar: Arc::clone(&sidecar),
                runtime: runtime.clone(),
            });
            let weak = Arc::downgrade(core);
            let restart = restart_handler(
                Arc::clone(&sidecar),
                runtime.clone(),
                restarted_sink(weak.clone()),
            );
            let restart = announced(weak.clone(), RESTARTING_KEY, restart);
            let last = Arc::new(AtomicU8::new(u8::MAX));
            let update = update_handler(
                sidecar,
                runtime,
                progress_sink(weak.clone(), Arc::clone(&last)),
                done_sink(weak.clone()),
            );
            let update = announced(weak, UPDATING_KEY, fresh_progress(last, update));
            (restart, update)
        }
        Err(error) => {
            let error = rb_diag::redact(&error.to_string());
            tracing::warn!(%error, "the Node sidecar is not available");
            (unavailable(&error), unavailable(&error))
        }
    };
    defs.into_iter()
        .zip([restart, update])
        .filter_map(|(def, handler)| registry.register(def, handler).err())
        .collect()
}

/// The user settings changed: the sidecar takes the new ones (the timeout at once, the paths at
/// its next start).
pub fn on_user_settings_changed(core: &Core) {
    let settings = settings_of(&core.user_settings());
    if let Some(running) = lock(&SIDECAR).as_ref() {
        running.sidecar.apply_settings(settings);
    }
}

/// The app's sidecar, once the core registered its commands; `None` without a launcher (WebKit
/// is then unavailable, RB-040-04).
pub fn shared() -> Option<Arc<Sidecar<ConfiguredLauncher>>> {
    lock(&SIDECAR)
        .as_ref()
        .map(|running| Arc::clone(&running.sidecar))
}

/// Stops the sidecar if one runs (the app quits; after `crate::engines::shutdown`, so WebKit's
/// windows are closed by then): its stdin closes, a sidecar that does not end within
/// `rb_sidecar::STOP_GRACE` is killed. Called outside the runtime.
pub fn shutdown(runtime: &tokio::runtime::Runtime) {
    let Some(running) = lock(&SIDECAR).take() else {
        return;
    };
    runtime.block_on(async {
        if tokio::time::timeout(SHUTDOWN_WAIT, running.sidecar.shutdown())
            .await
            .is_err()
        {
            tracing::warn!("the Node sidecar did not stop in time");
        }
    });
}

/// `sidecar.versions` of the running sidecar, `None` when none runs or it does not answer in
/// time. Safe from any thread: the wait runs on a thread of its own.
pub fn versions() -> Option<Value> {
    let (sidecar, runtime) = {
        let guard = lock(&SIDECAR);
        let running = guard.as_ref()?;
        (Arc::clone(&running.sidecar), running.runtime.clone())
    };
    std::thread::scope(|scope| {
        scope
            .spawn(move || {
                runtime.block_on(async {
                    tokio::time::timeout(VERSIONS_WAIT, sidecar.versions_if_running())
                        .await
                        .ok()
                        .flatten()
                })
            })
            .join()
            .ok()
            .flatten()
    })
}

/// Fills the Node and Playwright versions of the diagnostics from a `sidecar.versions` answer.
pub fn apply_versions(versions: &mut rb_diag::Versions, answer: &Value) {
    let text = |key: &str| answer.get(key).and_then(Value::as_str).map(str::to_owned);
    if let Some(node) = text("node") {
        versions.node = Some(node);
    }
    if let Some(playwright) = text("playwright") {
        versions.playwright = Some(playwright);
    }
}

/// Whether `percent` reaches a step above `last` (which it then becomes): a download shows its
/// progress at 0, 25, 50, 75 and 100 percent, not at every update.
pub fn next_step(last: &AtomicU8, percent: u8) -> bool {
    let step = percent.min(100) / PROGRESS_STEP * PROGRESS_STEP;
    let previous = last.load(Ordering::Acquire);
    if previous != u8::MAX && step <= previous {
        return false;
    }
    last.store(step, Ordering::Release);
    true
}

fn unavailable(error: &str) -> Handler {
    let error = error.to_owned();
    Arc::new(move |_| Err(CommandError::Failed(error.clone())))
}

/// `handler` with the progress steps `last` forgotten before each run, so every `engines.update`
/// shows its progress from the start.
fn fresh_progress(last: Arc<AtomicU8>, handler: Handler) -> Handler {
    Arc::new(move |args| {
        last.store(u8::MAX, Ordering::Release);
        handler(args)
    })
}

/// `handler` that first names its start in the status bar (`key`): a restart or a download
/// runs for a while before its end is named (RB-090-85).
fn announced(core: Weak<Core>, key: &'static str, handler: Handler) -> Handler {
    Arc::new(move |args| {
        if let Some(core) = core.upgrade() {
            core.notify(notification(
                NotificationLevel::Info,
                key,
                BTreeMap::new(),
                None,
            ));
        }
        handler(args)
    })
}

/// The end of `sidecar.restart` in the status bar: restarted, or why not (RB-090-85).
fn restarted_sink(core: Weak<Core>) -> rb_sidecar::command::RestartSink {
    Arc::new(move |result: Result<(), SidecarError>| {
        if let Some(core) = core.upgrade() {
            core.notify(restarted_notice(&result));
        }
    })
}

fn restarted_notice(result: &Result<(), SidecarError>) -> CoreToShell {
    match result {
        Ok(()) => notification(
            NotificationLevel::Success,
            RESTARTED_KEY,
            BTreeMap::new(),
            None,
        ),
        Err(error) => error_notice(error),
    }
}

/// `error` as a notice: its own text, linked to the setting that fixes it.
fn error_notice(error: &SidecarError) -> CoreToShell {
    let args = error
        .i18n_args()
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect();
    notification(
        NotificationLevel::Error,
        error.i18n_key(),
        args,
        error.settings_key(),
    )
}

fn progress_sink(core: Weak<Core>, last: Arc<AtomicU8>) -> rb_sidecar::command::ProgressSink {
    Arc::new(move |progress: InstallProgress| {
        if !next_step(&last, progress.percent) {
            return;
        }
        if let Some(core) = core.upgrade() {
            let args = BTreeMap::from([
                ("browser".to_owned(), progress.browser),
                ("percent".to_owned(), progress.percent.to_string()),
            ]);
            core.notify(notification(
                NotificationLevel::Info,
                PROGRESS_KEY,
                args,
                None,
            ));
        }
    })
}

fn done_sink(core: Weak<Core>) -> rb_sidecar::command::DoneSink {
    Arc::new(move |result: Result<InstallReport, SidecarError>| {
        let Some(core) = core.upgrade() else {
            return;
        };
        let message = match result {
            Ok(report) => {
                // The WebKit views that found no build start now (RB-040-04).
                crate::engines::webkit::installed();
                let key = report.i18n_key();
                let version = report.version.unwrap_or(report.revision);
                let args = BTreeMap::from([
                    ("browser".to_owned(), report.browser),
                    ("version".to_owned(), version),
                ]);
                notification(NotificationLevel::Info, key, args, None)
            }
            Err(error) => {
                let detail = rb_diag::redact(&error.to_string());
                tracing::warn!(error = %detail, "the engines update failed");
                error_notice(&error)
            }
        };
        core.notify(message);
    })
}

fn notification(
    level: NotificationLevel,
    key: &str,
    args: BTreeMap<String, String>,
    settings_key: Option<&str>,
) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args,
        settings_key: settings_key.map(str::to_owned),
    })
}

#[cfg(test)]
#[path = "sidecar_tests.rs"]
mod tests;

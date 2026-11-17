//! The core beside the view manager: the one command registry with every command's handler,
//! the leader state machine fed from CEF's key events, the user settings in effect and the
//! settings registry built from the schemas (the wiring of wave 0.1: RB-010-17, RB-010-02,
//! RB-010-10, RB-010-03). Portable; the CEF side only forwards key events and IPC requests.

mod bindings;
pub mod cef_age;
pub mod commands_file;
pub mod diag_save;
mod handlers;
mod keys;
pub mod settings;
mod title;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::SystemTime;

use rb_commands::{ChordMachine, CommandError, CommandId, CommandRegistry, LEADER, Notice};
use rb_config::UserSettings;
use rb_diag::{CefBuild, DiagPaths};
use rb_ipc::{
    CommandInfo, CoreToShell, Notification, NotificationLevel, StatusAction, event_script,
};
use rb_settings::SettingsRegistry;
use serde_json::Value;
pub(crate) use title::command_title;
use tokio::runtime::Handle;

use crate::cdp::lock;
use crate::host::Host;
use crate::host::ui::UiBackend;

pub use bindings::BindingChange;
pub use handlers::{bundle_input, diagnostics_cli, register_all};
pub use keys::KeyDown;

/// What the launch read before the core starts.
#[derive(Clone, Debug, Default)]
pub struct CoreSetup {
    /// The user settings in effect.
    pub user: UserSettings,
    /// Where they live; `None` writes nothing back (tests, no `%APPDATA%`).
    pub user_path: Option<PathBuf>,
    /// The file at `user_path` exists but could not be read: nothing is written over it.
    pub user_load_failed: bool,
    /// The log and dump folders; `None` when the environment names none.
    pub diag: Option<DiagPaths>,
    /// The CEF build to check for its age ([`cef_age::embedded_build`]); `None` checks nothing.
    pub cef_build: Option<CefBuild>,
}

/// The core of one window.
pub struct Core {
    me: Weak<Core>,
    host: Arc<Host>,
    ui: Arc<dyn UiBackend>,
    runtime: Handle,
    registry: Arc<Mutex<CommandRegistry>>,
    chords: Mutex<ChordMachine>,
    user: Mutex<UserSettings>,
    user_path: Option<PathBuf>,
    user_load_failed: bool,
    /// The file's `shortcuts.bindings` entries the registry did not take (a plugin command not
    /// loaded yet, a refused binding); written back as they are.
    skipped: Mutex<BTreeMap<String, Vec<String>>>,
    /// The default bindings the registry left unbound, by command (`droppedDefaults`).
    dropped: Mutex<BTreeMap<String, Vec<String>>>,
    settings: SettingsRegistry,
    /// Notifications raised before the shell could show them.
    pending: Mutex<Vec<CoreToShell>>,
    shell_ready: AtomicBool,
    /// The status bar's messages (RB-080-15).
    status: crate::status::StatusLog,
}

impl Core {
    /// The core over `host`: registers every command, applies the user's bindings (what does
    /// not fit becomes a notification), builds the settings registry and queues the notice
    /// about an old CEF build for the shell.
    pub fn new(
        host: Arc<Host>,
        ui: Arc<dyn UiBackend>,
        runtime: Handle,
        setup: CoreSetup,
    ) -> Arc<Core> {
        let (settings, issues) = settings::settings_registry();
        for issue in issues {
            tracing::warn!(key = %issue.key, error = %issue.error, "a setting was not registered");
        }
        let chords = ChordMachine::new(settings::chord_settings(&setup.user));
        crate::local_time::set_zone(setup.user.ui.file_name_time);
        let core = Arc::new_cyclic(|me: &Weak<Core>| Core {
            me: me.clone(),
            host,
            ui,
            runtime,
            registry: Arc::new(Mutex::new(CommandRegistry::new())),
            chords: Mutex::new(chords),
            user: Mutex::new(setup.user.clone()),
            user_path: setup.user_path.clone(),
            user_load_failed: setup.user_load_failed,
            skipped: Mutex::new(BTreeMap::new()),
            dropped: Mutex::new(BTreeMap::new()),
            settings,
            pending: Mutex::new(Vec::new()),
            shell_ready: AtomicBool::new(false),
            status: crate::status::StatusLog::for_settings(&setup.user.ui.status_bar),
        });
        let notices = {
            let mut registry = lock(&core.registry);
            for error in register_all(&mut registry, &core, setup.diag.clone()) {
                tracing::warn!(%error, "a command was not registered");
            }
            let mut notices = registry.take_notices();
            *lock(&core.dropped) = bindings::dropped_defaults(&notices);
            notices.extend(registry.load_user_bindings(&setup.user.shortcuts.bindings));
            *lock(&core.skipped) =
                bindings::skipped_entries(&registry, &setup.user.shortcuts.bindings);
            notices
        };
        core.notices(notices);
        let today = rb_diag::cef::days_since_epoch(SystemTime::now());
        if let Some(message) = setup
            .cef_build
            .as_ref()
            .and_then(|build| cef_age::notification(build, today, &setup.user))
        {
            core.notify(message);
        }
        core.spawn_ticker();
        // The installed Firefox editions, enumerated once off the UI thread, so the first
        // settings window already lists them (RB-080-32).
        crate::engines::firefox::installed::refresh_in_background(&core);
        core
    }

    /// The view manager.
    pub fn host(&self) -> &Arc<Host> {
        &self.host
    }

    /// The runtime of the core's tasks.
    pub fn runtime(&self) -> &Handle {
        &self.runtime
    }

    /// Every known setting, core keys from the schemas.
    pub fn settings_registry(&self) -> &SettingsRegistry {
        &self.settings
    }

    /// The user settings in effect.
    pub fn user_settings(&self) -> UserSettings {
        lock(&self.user).clone()
    }

    /// Puts `user` in effect (the settings window wrote them, the file changed; RB-010-08); the
    /// leader's timing, the emulation, hot reload and the sidecar's settings follow at once.
    /// Writing the file is the caller's.
    pub fn replace_user_settings(&self, user: UserSettings) {
        lock(&self.chords).set_settings(settings::chord_settings(&user));
        let theme = user.ui.theme;
        let theme_before = std::mem::replace(&mut *lock(&self.user), user).ui.theme;
        crate::emulation::on_user_settings_changed(self);
        // Hot reload's polling and the WSL watcher follow the user settings (RB-030-05).
        crate::reload::on_config_changed(self);
        crate::sidecar::on_user_settings_changed(self);
        crate::mcp::on_user_settings_changed(self);
        crate::cdp_proxy::on_user_settings_changed(self);
        // The portal stops when switched off or moved (RB-080-05); it never starts here.
        crate::portal::app::on_user_settings_changed(self);
        // Throttling follows at once (RB-020-09).
        crate::perf::on_user_settings_changed(self);
        // The Framework DevTools modes (RB-080-24).
        crate::sync_agent::framework_tasks::on_user_settings_changed(self);
        crate::plugins::on_user_settings_changed(self);
        // The status bar's history size and visibility (RB-080-15).
        crate::status::on_user_settings_changed(self);
        // The active view's label strip (RB-080-04).
        crate::layout::active_label::on_user_settings_changed(self);
        // The footer under every view (RB-080-44).
        crate::view_footer::on_user_settings_changed(self);
        // The engine badge of every label strip (RB-080-33).
        crate::engines::set_engine::on_user_settings_changed(self);
        // The vitals, engine window and address in every label strip (RB-090-27).
        crate::cell_info::on_user_settings_changed(self);
        // A canvas shown follows `layoutModes.showPartialViews` (RB-080-55).
        crate::layout::canvas_ops::on_user_settings_changed(self);
        // rBrowser's own theme in every window and its native parts (RB-080-95) - only when
        // `ui.theme` changed: every window repaints on it (RB-090-36).
        if theme != theme_before {
            crate::app_theme::on_user_settings_changed(self);
        }
        // `ui.followWindowsTextSize` (RB-090-26).
        crate::text_scale::on_user_settings_changed(self);
        // `ui.fileNameTime` (RB-090-69).
        crate::local_time::on_user_settings_changed(self);
    }

    /// The status bar's messages (RB-080-15).
    pub fn status_log(&self) -> &crate::status::StatusLog {
        &self.status
    }

    /// Where the user settings live; `None` writes nothing back.
    pub fn user_settings_path(&self) -> Option<&std::path::Path> {
        self.user_path.as_deref()
    }

    /// The window side, for the jobs' window work (contract 0.1-w1b).
    pub fn ui(&self) -> &Arc<dyn UiBackend> {
        &self.ui
    }

    /// This core as an `Arc`, for work that outlives a call; `None` while it is dropped.
    pub fn arc(&self) -> Option<Arc<Core>> {
        self.me.upgrade()
    }

    /// The `settings` answer: the user settings in effect, each key with its origin. The
    /// schema (`includeSchema`) comes with the settings window, RB-010-08.
    pub fn settings_answer(
        &self,
        include_schema: bool,
    ) -> Result<CoreToShell, (rb_ipc::IpcErrorCode, String)> {
        if include_schema {
            return Err((
                rb_ipc::IpcErrorCode::NotImplemented,
                "the settings schema comes with the settings window (RB-010-08)".to_owned(),
            ));
        }
        Ok(CoreToShell::Settings {
            values: settings::user_values(&self.settings, &lock(&self.user)),
            schema: None,
        })
    }

    /// The `commands` answer: every command with its bindings.
    pub fn commands(&self) -> CoreToShell {
        let dropped = lock(&self.dropped).clone();
        let summaries = lock(&self.registry).summaries();
        // The shell's catalogue is the core's: a plugin command's title comes along (RB-080-82).
        let texts = summaries
            .iter()
            .filter_map(|summary| {
                title::plugin_title_message(summary.id.as_str(), &summary.title_key)
                    .map(|text| (summary.title_key.clone(), text))
            })
            .collect();
        let commands = summaries
            .into_iter()
            .map(|summary| CommandInfo {
                id: summary.id.to_string(),
                title_key: summary.title_key,
                group: summary.group,
                bindings: summary.bindings,
                default_bindings: summary.default_bindings,
                changed: summary.changed,
                mcp: summary.mcp,
                dropped_defaults: dropped
                    .get(summary.id.as_str())
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect();
        CoreToShell::Commands {
            leader: LEADER.to_string(),
            commands,
            texts,
        }
    }

    /// Registers commands that arrive after the start - the plugins' (RB-060-12) - with
    /// `register`: the user's bindings are applied to them too, and the notices about them are
    /// shown; the shell gets the new command list.
    pub fn register_late(
        &self,
        register: impl FnOnce(&mut CommandRegistry) -> Vec<rb_commands::CommandError>,
    ) {
        let notices = {
            let mut registry = lock(&self.registry);
            let before: std::collections::BTreeSet<String> =
                registry.iter().map(|def| def.id.to_string()).collect();
            for error in register(&mut registry) {
                tracing::warn!(%error, "a command was not registered");
            }
            let mut notices = registry.take_notices();
            lock(&self.dropped).extend(bindings::dropped_defaults(&notices));
            let user = lock(&self.user).shortcuts.bindings.clone();
            notices.extend(registry.load_user_bindings(&user));
            *lock(&self.skipped) = bindings::skipped_entries(&registry, &user);
            // Only what concerns the new commands; the rest was shown at the start.
            notices.retain(|notice| !before.contains(&notice.command));
            notices.retain(|notice| registry.iter().any(|def| def.id.as_str() == notice.command));
            notices
        };
        self.notices(notices);
        self.send(&self.commands());
    }

    /// Whether the registry knows the command `id` (the context menu's optional entries,
    /// RB-080-43).
    pub fn has_command(&self, id: &str) -> bool {
        CommandId::new(id).is_ok_and(|id| lock(&self.registry).get(&id).is_some())
    }

    /// Runs the command `id` (toolbar, palette, MCP) on this thread. The handlers return
    /// quickly but for `diagnostics.create`, which writes a ZIP. The registry is not held while
    /// the handler runs, so key presses go on.
    pub fn run_command(&self, id: &str, args: Value) -> Result<Value, CommandError> {
        let id = CommandId::new(id).map_err(|_| CommandError::UnknownCommand(id.to_owned()))?;
        run_handler(&self.registry, &id, args)
    }

    /// Runs the command `id` off the calling thread (the shell's `runCommand` arrives on the
    /// CEF UI thread) and hands the result to `done` there.
    pub fn run_command_async(
        &self,
        id: &str,
        args: Value,
        done: impl FnOnce(Result<Value, CommandError>) + Send + 'static,
    ) {
        let Ok(id) = CommandId::new(id) else {
            done(Err(CommandError::UnknownCommand(id.to_owned())));
            return;
        };
        let registry = self.registry.clone();
        self.runtime
            .spawn_blocking(move || done(run_handler(&registry, &id, args)));
    }

    /// Runs the command `id` off the UI thread (a shortcut); a failure becomes a notification.
    fn spawn_command(&self, id: CommandId) {
        let (registry, me) = (self.registry.clone(), self.me.clone());
        self.runtime.spawn_blocking(move || {
            let result = run_handler(&registry, &id, Value::Null);
            if let (Err(error), Some(core)) = (result, me.upgrade()) {
                tracing::warn!(command = %id, %error, "the command failed");
                let title = command_title(&lock(&registry), id.as_str());
                core.notify(failure_notification(&title, &error));
            }
        });
    }

    /// The title of command `id` for the status bar ([`command_title`]).
    pub fn command_title(&self, id: &str) -> String {
        command_title(&lock(&self.registry), id)
    }

    /// Sends `message` to the shell now (an event; lost while the shell is not loaded).
    pub fn send(&self, message: &CoreToShell) {
        match event_script(message) {
            Ok(script) => self.ui.send_to_shell(script),
            Err(error) => tracing::warn!(%error, "a message to the shell cannot be encoded"),
        }
    }

    /// Shows `message` (a notification) in the shell, or keeps it until the shell is ready. A
    /// notification goes into the status bar's log and to the bar and its history at once: they
    /// read the log when they load, so their event never waits (RB-080-15).
    pub fn notify(&self, message: CoreToShell) {
        self.notify_with(message, None);
    }

    /// [`Self::notify`] with a button in the status bar that runs `action`'s command
    /// (RB-090-71); the shell's own notification carries none.
    pub fn notify_with_action(&self, notification: Notification, action: StatusAction) {
        self.notify_with(CoreToShell::Notification(notification), Some(action));
    }

    fn notify_with(&self, message: CoreToShell, action: Option<StatusAction>) {
        if let CoreToShell::Notification(notification) = &message {
            let entry =
                self.status
                    .push_with(notification.clone(), action, crate::status::now_ms());
            match event_script(&CoreToShell::StatusMessage(entry)) {
                Ok(script) => self.ui.send_to_status_bar(script),
                Err(error) => tracing::warn!(%error, "a status message cannot be encoded"),
            }
        }
        if self.shell_ready.load(Ordering::Acquire) {
            self.send(&message);
        } else {
            lock(&self.pending).push(message);
        }
    }

    /// The shell has loaded: what waited for it is shown now.
    pub fn shell_ready(&self) {
        self.shell_ready.store(true, Ordering::Release);
        let pending = std::mem::take(&mut *lock(&self.pending));
        for message in pending {
            self.send(&message);
        }
    }

    /// The registry's notices as notifications.
    fn notices(&self, notices: Vec<Notice>) {
        for notice in notices {
            self.notify(notice_notification(notice));
        }
    }
}

/// Runs the handler of `id` with the registry's lock released first.
fn run_handler(
    registry: &Mutex<CommandRegistry>,
    id: &CommandId,
    args: Value,
) -> Result<Value, CommandError> {
    let handler = lock(registry)
        .handler(id)
        .ok_or_else(|| CommandError::UnknownCommand(id.to_string()))?;
    handler(args)
}

/// A binding notice of the registry as a warning (placeholders `{command}`, `{binding}`,
/// `{reason}`; the reason is an i18n key).
fn notice_notification(notice: Notice) -> CoreToShell {
    let mut args = std::collections::BTreeMap::from([("command".to_owned(), notice.command)]);
    if let Some(binding) = notice.binding {
        args.insert("binding".to_owned(), binding);
    }
    if let Some(reason) = notice.reason_key {
        args.insert("reason".to_owned(), reason);
    }
    CoreToShell::Notification(Notification {
        level: NotificationLevel::Warning,
        message_key: notice.message_key,
        args,
        settings_key: Some("shortcuts.bindings".to_owned()),
    })
}

/// The i18n key of the notification for a command that failed.
pub const COMMAND_FAILED_KEY: &str = "commands.notice.failed";

/// The notification of a failed command named `title` (a shortcut's, the context menu's;
/// [`command_title`]).
pub(crate) fn failure_notification(title: &str, error: &CommandError) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level: NotificationLevel::Error,
        message_key: COMMAND_FAILED_KEY.to_owned(),
        args: std::collections::BTreeMap::from([
            ("command".to_owned(), title.to_owned()),
            ("error".to_owned(), error.to_string()),
        ]),
        settings_key: None,
    })
}

/// A warning for the status bar: the text of `key`, linked to the setting `settings_key` when
/// one decides it.
pub(crate) fn warning_notification(key: &str, settings_key: Option<&str>) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level: NotificationLevel::Warning,
        message_key: key.to_owned(),
        args: std::collections::BTreeMap::new(),
        settings_key: settings_key.map(str::to_owned),
    })
}

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;

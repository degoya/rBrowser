//! Where the changes come from: the system's file events (ReadDirectoryChangesW on Windows,
//! inotify elsewhere), polling, or `rbrowser-wsl watch` inside the distribution of a WSL project
//! (file events over `\\wsl.localhost` are unreliable). Each source sends [`SourceEvent`]s into
//! one channel; the service decides what they do.

use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::time::{Duration, Instant};

use notify::{Config, PollWatcher, RecommendedWatcher, Watcher};
use rb_wsl::helper::{helper_args, helper_command_factory, helper_linux_path};
use rb_wsl::path::{Translator, WslPath, parse_unc};
use rb_wsl::protocol::{ErrorCode, Message};
use rb_wsl::settings::WslSettings;
use rb_wsl::supervisor::{HelperStatus, Session, Supervisor, SupervisorConfig};
use rb_wsl::watch::WatchRules;
use rb_wsl::watch::batch::{Batch, Debouncer};
use rb_wsl::watch::filter::Filter;
use rb_wsl::watch::tree::{Seen, TreeMode, TreeWatch};
use tokio::process::Command;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::status::WatcherStatus;
use super::watch_errors::WatchErrors;

/// What a source tells the service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceEvent {
    /// A debounced batch.
    Batch(Batch),
    /// The source's state changed.
    Status(WatcherStatus),
    /// The WSL helper is given up: fall back to polling.
    HelperGaveUp {
        /// The i18n key of the reason.
        reason_key: String,
        /// The reason for the log.
        detail: String,
    },
}

/// A failure to start a source: the i18n key and the detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartError {
    /// The i18n key of the message.
    pub key: String,
    /// The detail for the log.
    pub detail: String,
}

impl StartError {
    fn new(key: &str, detail: impl ToString) -> Self {
        Self {
            key: key.to_owned(),
            detail: detail.to_string(),
        }
    }
}

/// Which source watches the project at `root`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceChoice {
    /// The system's file events.
    Native,
    /// Polling (`reload.pollingFallback`).
    Polling,
    /// The helper in the distribution.
    Helper(WslPath),
}

/// The source for `root`: polling when asked for, the helper for a path in a distribution's
/// share, the system's events otherwise.
#[must_use]
pub fn choose(root: &Path, polling: bool) -> SourceChoice {
    if polling {
        return SourceChoice::Polling;
    }
    match root.to_str().map(parse_unc) {
        Some(Ok(wsl)) => SourceChoice::Helper(wsl),
        _ => SourceChoice::Native,
    }
}

/// The app's own watch on a thread of its own; dropping it ends the thread and the watch.
pub struct NativeWatch {
    stop: std_mpsc::Sender<Input>,
    folders: usize,
}

impl NativeWatch {
    /// The folders watched.
    #[must_use]
    pub fn folders(&self) -> u64 {
        u64::try_from(self.folders).unwrap_or(u64::MAX)
    }
}

impl Drop for NativeWatch {
    fn drop(&mut self) {
        // The thread may have ended already (its sink is gone).
        let _ = self.stop.send(Input::Stop);
    }
}

enum Input {
    Fs(notify::Result<notify::Event>),
    Stop,
}

/// Watches `root` with the system's events, or by polling every `poll`.
///
/// # Errors
///
/// [`StartError`] when the rules do not compile or the tree cannot be watched.
pub fn start_native(
    root: &Path,
    rules: &WatchRules,
    poll: Option<Duration>,
    sink: mpsc::UnboundedSender<SourceEvent>,
) -> Result<NativeWatch, StartError> {
    let filter = Filter::new(rules).map_err(|error| StartError::new(error.i18n_key(), &error))?;
    let (events, inbox) = std_mpsc::channel::<Input>();
    let handler = {
        let events = events.clone();
        move |event| {
            let _ = events.send(Input::Fs(event));
        }
    };
    let quiet = Duration::from_millis(rules.debounce_ms);
    let folders = match poll {
        Some(interval) => {
            let watcher = PollWatcher::new(handler, Config::default().with_poll_interval(interval))
                .map_err(|error| StartError::new("reload.error.watchFailed", error))?;
            spawn_tree(
                root,
                filter,
                watcher,
                TreeMode::PerFolder,
                inbox,
                sink,
                quiet,
            )?
        }
        None => {
            let watcher = RecommendedWatcher::new(handler, Config::default())
                .map_err(|error| StartError::new("reload.error.watchFailed", error))?;
            // ReadDirectoryChangesW covers a tree with one handle; inotify needs one per folder.
            let mode = if cfg!(windows) {
                TreeMode::Recursive
            } else {
                TreeMode::PerFolder
            };
            spawn_tree(root, filter, watcher, mode, inbox, sink, quiet)?
        }
    };
    Ok(NativeWatch {
        stop: events,
        folders,
    })
}

fn spawn_tree<W: Watcher + Send + 'static>(
    root: &Path,
    filter: Filter,
    watcher: W,
    mode: TreeMode,
    inbox: std_mpsc::Receiver<Input>,
    sink: mpsc::UnboundedSender<SourceEvent>,
    quiet: Duration,
) -> Result<usize, StartError> {
    let tree = TreeWatch::new(root, filter, watcher, mode)
        .map_err(|error| StartError::new(error.i18n_key(), &error))?;
    let folders = tree.folders();
    std::thread::Builder::new()
        .name("rb-reload-watch".into())
        .spawn(move || run_tree(tree, &inbox, &sink, quiet))
        .map_err(|error| StartError::new("reload.error.watchFailed", error))?;
    Ok(folders)
}

fn run_tree<W: Watcher>(
    mut tree: TreeWatch<W>,
    inbox: &std_mpsc::Receiver<Input>,
    sink: &mpsc::UnboundedSender<SourceEvent>,
    quiet: Duration,
) {
    let mut debounce = Debouncer::new(quiet);
    let mut errors = WatchErrors::default();
    loop {
        let next = match debounce.deadline() {
            Some(deadline) => {
                match inbox.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(next) => Some(next),
                    Err(std_mpsc::RecvTimeoutError::Timeout) => None,
                    Err(std_mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
            None => match inbox.recv() {
                Ok(next) => Some(next),
                Err(_) => return,
            },
        };
        let now = Instant::now();
        match next {
            None => {}
            Some(Input::Stop) => return,
            // Polling reports an unreadable entry on every pass: logged and rescanned once.
            Some(Input::Fs(Err(error))) => {
                if errors.on_error(&error.to_string(), &error.paths, now) {
                    tracing::warn!(%error, "the project watch reported an error");
                    debounce.push_rescan(now);
                }
            }
            Some(Input::Fs(Ok(event))) => match tree.on_event(&event) {
                Ok(seen) => {
                    for error in errors.on_event(&event.paths) {
                        tracing::info!(%error, "the project watch error is over");
                    }
                    for seen in seen {
                        match seen {
                            Seen::Change(change) => debounce.push(change, now),
                            Seen::Rescan => {
                                debounce.push_rescan(now);
                                if let Err(error) = tree.rewalk() {
                                    tracing::warn!(%error, "the project cannot be walked again");
                                }
                            }
                        }
                    }
                }
                Err(error) => {
                    let failed = WatcherStatus::Failed {
                        reason_key: error.i18n_key().to_owned(),
                        detail: error.to_string(),
                    };
                    let _ = sink.send(SourceEvent::Status(failed));
                    return;
                }
            },
        }
        if let Some(batch) = debounce.take(Instant::now())
            && sink.send(SourceEvent::Batch(batch)).is_err()
        {
            return;
        }
    }
}

/// How to start the helper: its command and timings, and the share path polling falls back to.
pub struct HelperSpec {
    /// The distribution it runs in.
    pub distro: String,
    /// A command per start.
    pub factory: Box<dyn FnMut() -> Command + Send>,
    /// Restart and handshake timings.
    pub config: SupervisorConfig,
}

/// The spec of the packaged (or `wsl.helperPath`) helper for the project `wsl`, in `wsl.distro`
/// when set.
///
/// # Errors
///
/// [`StartError`] when a path has no form inside the distribution.
pub fn helper_spec(wsl: &WslPath, settings: &WslSettings) -> Result<HelperSpec, StartError> {
    let path_error = |error: rb_wsl::path::PathError| StartError::new(error.i18n_key(), &error);
    let distro = settings.distro().unwrap_or(&wsl.distro).to_owned();
    let translator = Translator::new(&distro).map_err(path_error)?;
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let helper = helper_linux_path(
        &translator,
        &exe_dir.display().to_string(),
        settings.helper_path(),
    )
    .map_err(path_error)?;
    let args = helper_args(
        &distro,
        &helper,
        &["watch".to_owned(), wsl.linux_path.clone()],
    )
    .map_err(path_error)?;
    let factory = helper_command_factory(rb_wsl::distro::wsl_exe(), args);
    Ok(HelperSpec {
        distro,
        factory: Box::new(factory),
        config: SupervisorConfig::default(),
    })
}

/// The helper at work; dropping it stops the helper.
pub struct HelperWatch {
    supervisor: Option<Supervisor>,
    task: JoinHandle<()>,
}

impl HelperWatch {
    /// Stops the helper and waits for it.
    pub async fn shutdown(mut self) {
        self.task.abort();
        if let Some(supervisor) = self.supervisor.take() {
            supervisor.shutdown().await;
        }
    }
}

impl Drop for HelperWatch {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Starts the helper of `spec` with `rules` on `runtime`.
pub fn start_helper(
    spec: HelperSpec,
    rules: WatchRules,
    sink: mpsc::UnboundedSender<SourceEvent>,
    runtime: &Handle,
) -> HelperWatch {
    let _entered = runtime.enter();
    let (messages, mut inbox) = mpsc::unbounded_channel();
    let session = Session {
        greeting: vec![Message::Watch { rules }],
        messages: Some(messages),
    };
    let supervisor = Supervisor::start_session(spec.factory, spec.config, session);
    let mut status = supervisor.subscribe();
    let distro = spec.distro;
    let task = tokio::spawn(async move {
        loop {
            let event = tokio::select! {
                message = inbox.recv() => match message {
                    Some(message) => from_message(message, &distro),
                    None => return,
                },
                changed = status.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    let current = status.borrow_and_update().clone();
                    from_status(current)
                }
            };
            if let Some(event) = event
                && sink.send(event).is_err()
            {
                return;
            }
        }
    });
    HelperWatch {
        supervisor: Some(supervisor),
        task,
    }
}

/// What a message of the helper means.
fn from_message(message: Message, distro: &str) -> Option<SourceEvent> {
    match message {
        Message::Changes { changes } => Some(SourceEvent::Batch(Batch::Changes(changes))),
        Message::Rescan => Some(SourceEvent::Batch(Batch::Rescan)),
        Message::Watching { folders } => Some(SourceEvent::Status(WatcherStatus::Helper {
            distro: distro.to_owned(),
            folders,
        })),
        Message::Error {
            code: code @ (ErrorCode::BadRule | ErrorCode::WatchFailed),
            message,
            ..
        } => Some(SourceEvent::HelperGaveUp {
            reason_key: if code == ErrorCode::BadRule {
                "reload.error.badRule"
            } else {
                "reload.error.watchFailed"
            }
            .to_owned(),
            detail: message,
        }),
        other => {
            tracing::debug!(
                kind = other.kind(),
                "a message of the WSL helper is ignored"
            );
            None
        }
    }
}

/// What a state of the supervisor means.
fn from_status(status: HelperStatus) -> Option<SourceEvent> {
    match status {
        HelperStatus::Starting { .. } | HelperStatus::Restarting { .. } => {
            Some(SourceEvent::Status(WatcherStatus::Starting))
        }
        HelperStatus::Failed {
            reason_key, detail, ..
        } => Some(SourceEvent::HelperGaveUp { reason_key, detail }),
        HelperStatus::Running { .. } | HelperStatus::Stopped => None,
    }
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;

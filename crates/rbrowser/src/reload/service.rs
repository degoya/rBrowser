//! Hot reload at work for one window: a source of changes (see [`super::source`]), the plan of
//! each batch and its work in the views, the commands (reload, CSS only, pause), the interval
//! reload and the watcher's state for the toolbar.
//!
//! Each start of a source is a generation with its own channel and consumer task; events of an
//! older generation are dropped, so a stopped helper can never overwrite the state of the new
//! watch.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use rb_engine::{Target, ViewHost};
use rb_wsl::settings::WslSettings;
use rb_wsl::watch::batch::Batch;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, watch};

use super::classify::{HotMode, plan, rules_changed};
use super::interval::IntervalReload;
use super::mcp::{ReloadAnswer, ReloadArgs, ReloadKind};
use super::options::{ReloadOptions, read_ignore_file};
use super::perform::{HowTo, Work, perform};
use super::source::{
    HelperSpec, HelperWatch, NativeWatch, SourceChoice, SourceEvent, StartError, choose,
    helper_spec, start_helper, start_native,
};
use super::status::{WatcherInfo, WatcherStatus};

#[path = "service_interval.rs"]
mod interval_task;

/// The longest the interval task sleeps before it looks at the options again.
const INTERVAL_LOOK: Duration = Duration::from_secs(1);

/// Makes the helper's start spec for a WSL project (replaced in tests).
pub type HelperSpecFn = Box<
    dyn Fn(&rb_wsl::path::WslPath, &WslSettings) -> Result<HelperSpec, StartError> + Send + Sync,
>;

/// Hot reload for one window.
#[derive(Clone)]
pub struct HotReload {
    inner: Arc<Inner>,
}

struct Inner {
    host: Arc<dyn ViewHost>,
    runtime: Handle,
    spec: HelperSpecFn,
    state: Mutex<State>,
    info: watch::Sender<WatcherInfo>,
}

/// The running source; held only so that dropping it stops the watch.
enum Source {
    None,
    Native { _watch: NativeWatch },
    Helper { _watch: HelperWatch },
}

struct State {
    root: Option<PathBuf>,
    options: ReloadOptions,
    wsl: WslSettings,
    environment: Option<String>,
    paused: bool,
    status: WatcherStatus,
    generation: u64,
    source: Source,
    interval: Option<IntervalReload>,
    /// An interval step is reloading: a second step at the same time does nothing.
    interval_busy: bool,
}

impl HotReload {
    /// Starts hot reload for the project at `root` (none: only the commands work) on `runtime`.
    pub fn start(
        host: Arc<dyn ViewHost>,
        runtime: Handle,
        root: Option<PathBuf>,
        options: ReloadOptions,
        wsl: WslSettings,
    ) -> Self {
        Self::start_with(host, runtime, root, options, wsl, Box::new(helper_spec))
    }

    /// [`Self::start`] with the helper's spec made by `spec`.
    pub fn start_with(
        host: Arc<dyn ViewHost>,
        runtime: Handle,
        root: Option<PathBuf>,
        options: ReloadOptions,
        wsl: WslSettings,
        spec: HelperSpecFn,
    ) -> Self {
        let mode = options.mode;
        let state = State {
            root,
            options,
            wsl,
            environment: None,
            paused: false,
            status: WatcherStatus::Off,
            generation: 0,
            source: Source::None,
            interval: None,
            interval_busy: false,
        };
        let (info, _) = watch::channel(WatcherInfo::new(WatcherStatus::Off, false, mode, None));
        let service = Self {
            inner: Arc::new(Inner {
                host,
                runtime,
                spec,
                state: Mutex::new(state),
                info,
            }),
        };
        service.inner.restart_later();
        service.inner.reset_interval();
        service.spawn_interval();
        service
    }

    /// New options, project or WSL settings (a config reload, a user-settings write): the
    /// source starts again only when one of them changed, since a restart walks the tree again
    /// and restarts the helper.
    pub fn apply(&self, root: Option<PathBuf>, options: ReloadOptions, wsl: WslSettings) {
        let changed = {
            let mut state = self.inner.state();
            let changed = state.root != root || state.options != options || state.wsl != wsl;
            state.root = root;
            state.options = options;
            state.wsl = wsl;
            changed
        };
        if changed {
            self.inner.restart_later();
        }
        self.inner.reset_interval();
    }

    /// The active environment changed (its interval may differ).
    pub fn set_environment(&self, environment: Option<String>) {
        let changed = {
            let mut state = self.inner.state();
            let changed = state.environment != environment;
            state.environment = environment;
            changed
        };
        if changed {
            self.inner.reset_interval();
        }
    }

    /// Pauses or resumes hot reload for this session; whether it is paused now.
    pub fn toggle_pause(&self) -> bool {
        let paused = {
            let mut state = self.inner.state();
            state.paused = !state.paused;
            state.paused
        };
        self.inner.publish();
        paused
    }

    /// `wsl.restart_helper`: the WSL helper of the project starts again as a new generation,
    /// also after it was given up and polling stood in. `false` (nothing changes) when the
    /// project is not watched through the helper.
    pub fn restart_helper(&self) -> bool {
        let helper = {
            let state = self.inner.state();
            state.options.mode != HotMode::Off
                && state.root.as_deref().is_some_and(|root| {
                    matches!(choose(root, state.options.polling), SourceChoice::Helper(_))
                })
        };
        if helper {
            self.inner.restart_later();
        }
        helper
    }

    /// Input in a view: the interval reload waits.
    pub fn note_input(&self) {
        if let Some(interval) = self.inner.state().interval.as_mut() {
            interval.note_input(Instant::now());
        }
    }

    /// The watcher's state now.
    #[must_use]
    pub fn info(&self) -> WatcherInfo {
        self.inner.info.borrow().clone()
    }

    /// The watcher's state as it changes.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<WatcherInfo> {
        self.inner.info.subscribe()
    }

    /// Runs `future` to its end from a command handler, which runs on a blocking thread of the
    /// runtime (`Core::run_command_async`, the shortcuts, the instance channel). `None` on a
    /// thread outside the runtime - the CEF UI thread, which CDP needs to answer, so waiting
    /// there would hang the window: nothing runs.
    pub fn block_on<F: std::future::Future>(&self, future: F) -> Option<F::Output> {
        Handle::try_current().ok()?;
        Some(tokio::task::block_in_place(|| {
            self.inner.runtime.block_on(future)
        }))
    }

    /// `reload.all`, `reload.css_only` and MCP `reload`: by hand, so pages with HMR reload too.
    pub async fn reload(&self, args: ReloadArgs) -> ReloadAnswer {
        let work = match args.mode {
            ReloadKind::Full => Work::Full,
            ReloadKind::Css => Work::all_css(),
        };
        let how = HowTo {
            respect_hmr: false,
            fallback_full: false,
            ignore_cache: args.ignore_cache,
        };
        let views = perform(self.inner.host.as_ref(), args.target, &work, how).await;
        if work == Work::Full {
            self.inner.reloaded();
        }
        ReloadAnswer {
            mode: args.mode,
            views,
        }
    }
}

impl Inner {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Publishes the state for the toolbar and `project_info`.
    fn publish(&self) {
        let info = {
            let state = self.state();
            let interval = state
                .options
                .interval(state.environment.as_deref())
                .map(|period| period.as_secs());
            WatcherInfo::new(
                state.status.clone(),
                state.paused,
                state.options.mode,
                interval,
            )
        };
        self.info.send_replace(info);
    }

    fn set_status(&self, status: WatcherStatus) {
        tracing::info!(state = status.label_key(), "the project watch changed");
        self.state().status = status;
        self.publish();
    }

    /// Stops the source and starts the one the options call for, as a new generation.
    fn restart_source(self: &Arc<Self>) {
        let (sink, inbox) = mpsc::unbounded_channel();
        let (generation, choice, root, rules, poll, wsl) = {
            let mut state = self.state();
            state.generation += 1;
            state.source = Source::None;
            let Some(root) = state.root.clone() else {
                drop(state);
                self.set_status(WatcherStatus::Off);
                return;
            };
            if state.options.mode == HotMode::Off {
                drop(state);
                self.set_status(WatcherStatus::Off);
                return;
            }
            let rules = state.options.rules(read_ignore_file(&root).as_deref());
            let choice = choose(&root, state.options.polling);
            let poll = state.options.poll_interval();
            (
                state.generation,
                choice,
                root,
                rules,
                poll,
                state.wsl.clone(),
            )
        };
        let (source, status) = match choice {
            SourceChoice::Native => match start_native(&root, &rules, None, sink.clone()) {
                Ok(watch) => {
                    let folders = watch.folders();
                    (
                        Source::Native { _watch: watch },
                        WatcherStatus::Native { folders },
                    )
                }
                Err(error) => self.poll(&root, &rules, poll, sink.clone(), Some(error)),
            },
            SourceChoice::Polling => self.poll(&root, &rules, poll, sink.clone(), None),
            SourceChoice::Helper(wsl_path) => match (self.spec)(&wsl_path, &wsl) {
                Ok(spec) => (
                    Source::Helper {
                        _watch: start_helper(spec, rules, sink.clone(), &self.runtime),
                    },
                    WatcherStatus::Starting,
                ),
                Err(error) => self.poll(&root, &rules, poll, sink.clone(), Some(error)),
            },
        };
        if self.install(generation, source, status) {
            self.consume(generation, inbox);
        }
    }

    /// Puts `source` and `status` in effect when `generation` is still the current one; a
    /// newer start has won otherwise and `source` is dropped (stopped). Whether it was put.
    fn install(&self, generation: u64, source: Source, status: WatcherStatus) -> bool {
        {
            let mut state = self.state();
            if state.generation != generation {
                return false;
            }
            state.source = source;
        }
        self.set_status(status);
        true
    }

    /// Hands the events of `generation` to [`Self::on_event`], in order.
    fn consume(self: &Arc<Self>, generation: u64, mut inbox: mpsc::UnboundedReceiver<SourceEvent>) {
        let me = Arc::downgrade(self);
        self.runtime.spawn(async move {
            while let Some(event) = inbox.recv().await {
                let Some(inner) = me.upgrade() else {
                    return;
                };
                inner.on_event(generation, event).await;
            }
        });
    }

    /// [`Self::restart_source`] off the calling thread: walking a tree takes a while.
    fn restart_later(self: &Arc<Self>) {
        let me = Arc::clone(self);
        self.runtime.spawn_blocking(move || me.restart_source());
    }

    /// Polls `root`; `reason` is the failure polling stands in for.
    fn poll(
        &self,
        root: &std::path::Path,
        rules: &rb_wsl::watch::WatchRules,
        interval: Duration,
        sink: mpsc::UnboundedSender<SourceEvent>,
        reason: Option<StartError>,
    ) -> (Source, WatcherStatus) {
        if let Some(reason) = &reason {
            tracing::warn!(key = %reason.key, detail = %reason.detail, "polling stands in");
        }
        match start_native(root, rules, Some(interval), sink) {
            Ok(watch) => {
                let folders = watch.folders();
                let reason_key = reason.map(|reason| reason.key);
                (
                    Source::Native { _watch: watch },
                    WatcherStatus::Polling {
                        folders,
                        reason_key,
                    },
                )
            }
            Err(error) => (
                Source::None,
                WatcherStatus::Failed {
                    reason_key: error.key,
                    detail: error.detail,
                },
            ),
        }
    }

    async fn on_event(self: &Arc<Self>, generation: u64, event: SourceEvent) {
        if self.state().generation != generation {
            return;
        }
        match event {
            SourceEvent::Status(status) => self.set_status(status),
            SourceEvent::HelperGaveUp { reason_key, detail } => {
                let (root, rules, poll) = {
                    let mut state = self.state();
                    state.source = Source::None;
                    let Some(root) = state.root.clone() else {
                        return;
                    };
                    let rules = state.options.rules(read_ignore_file(&root).as_deref());
                    (root, rules, state.options.poll_interval())
                };
                let reason = StartError {
                    key: reason_key,
                    detail,
                };
                let (sink, inbox) = mpsc::unbounded_channel();
                let (source, status) = self.poll(&root, &rules, poll, sink, Some(reason));
                // A restart may have come while the tree was walked.
                if self.install(generation, source, status) {
                    self.consume(generation, inbox);
                }
            }
            SourceEvent::Batch(batch) => self.on_batch(batch).await,
        }
    }

    async fn on_batch(self: &Arc<Self>, batch: Batch) {
        if rules_changed(&batch) {
            // New rules make a new generation; this batch still counts.
            self.restart_later();
        }
        let (mode, paused, respect_hmr) = {
            let state = self.state();
            (state.options.mode, state.paused, state.options.respect_hmr)
        };
        if paused {
            return;
        }
        let Some(work) = Work::from_plan(plan(&batch, mode)) else {
            return;
        };
        let how = HowTo {
            respect_hmr,
            fallback_full: true,
            ignore_cache: false,
        };
        let done = perform(self.host.as_ref(), Target::All, &work, how).await;
        tracing::debug!(views = done.len(), ?work, "hot reload");
        if work == Work::Full {
            self.reloaded();
        }
    }

    fn reloaded(&self) {
        if let Some(interval) = self.state().interval.as_mut() {
            interval.reloaded(Instant::now());
        }
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;

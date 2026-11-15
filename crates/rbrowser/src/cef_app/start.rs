//! What happens once CEF is up (`OnContextInitialized`, UI thread): the shell's scheme and
//! query handler, the CDP router on the pipe, the host, the window, the grid, the core with the
//! commands and the keyboard, the config watch and the state pushes to the shell.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use cef::*;
use rb_config::ProjectConfig;
use tokio::runtime::Handle;

use super::backend::CefUi;
use super::pipe::{self, PipeEnds};
use super::views::{CORE, HOST};
use super::{queries, scheme, window};
use crate::cdp::{CdpRouter, CdpTransport, lock};
use crate::control::{Core, CoreSetup};
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::UiBackend;
use crate::watch::{ConfigWatch, watch_config};
use crate::{commands, features, ipc};

/// What the launch prepared before CEF started.
pub struct Launch {
    /// The project config (or the default without a project).
    pub config: ProjectConfig,
    /// The project root.
    pub root: Option<PathBuf>,
    /// The workspace was trusted when `config` was loaded (RB-010-09).
    pub trusted: bool,
    /// The runtime of the host's tasks.
    pub runtime: Handle,
    /// Our ends of the CDP pipe.
    pub pipe: Option<PipeEnds>,
    /// The built shell beside the executable.
    pub ui_dir: PathBuf,
    /// The window title.
    pub title: String,
    /// The user settings and the diagnostics folders for the core.
    pub setup: CoreSetup,
    /// Open the window without taking the focus (`--background`, RB-010-11).
    pub background: bool,
}

/// Set once by the launch, read once by [`Startup`].
pub static LAUNCH: OnceLock<Launch> = OnceLock::new();
/// The config watch, alive as long as the app.
static WATCH: Mutex<Option<ConfigWatch>> = Mutex::new(None);

/// A transport for an app without a pipe: every call fails at once.
struct NoPipe;

impl CdpTransport for NoPipe {
    fn send(&self, _frame: Vec<u8>) -> Result<(), rb_engine::EngineError> {
        Err(rb_engine::EngineError::Closed)
    }
}

wrap_browser_process_handler! {
    pub struct Startup {}

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            start();
        }
    }
}

fn start() {
    let Some(launch) = LAUNCH.get() else {
        tracing::error!("CEF started without a launch");
        return;
    };
    scheme::register(launch.ui_dir.clone());
    queries::install();
    let cdp = match launch.pipe {
        Some(ends) => pipe::connect(ends),
        None => {
            let router = CdpRouter::new(Arc::new(NoPipe));
            router.on_closed();
            router
        }
    };
    let ui: Arc<dyn UiBackend> = Arc::new(CefUi);
    let host = Host::new(ui.clone(), cdp, launch.runtime.clone());
    if HOST.set(host.clone()).is_err() {
        tracing::error!("the host was started twice");
        return;
    }
    window::open(&launch.title, launch.background);
    super::ui_watchdog::start(&launch.runtime);
    crate::trust::start_trust(&host, launch.trusted);
    // Before the first grid: its cells are built with their footers (RB-080-44).
    host.set_footer_settings(launch.setup.user.ui.view_footer.clone());
    if let Err(error) = host.open_config(launch.config.clone(), launch.root.clone()) {
        tracing::error!(%error, "the project's grid cannot be built; showing the default grid");
        if let Err(error) = host.open_config(default_config(), launch.root.clone()) {
            tracing::error!(%error, "the default grid cannot be built");
        }
    }
    let core = Core::new(
        host.clone(),
        ui.clone(),
        launch.runtime.clone(),
        launch.setup.clone(),
    );
    if CORE.set(core.clone()).is_err() {
        tracing::error!("the core was started twice");
        return;
    }
    features::on_start(&core);
    crate::reload::on_start(&core, &launch.runtime);
    if let Some(root) = &launch.root {
        let weak = Arc::downgrade(&core);
        match watch_config(root, move || {
            let Some(core) = weak.upgrade() else {
                return;
            };
            match commands::reload_config(core.host()) {
                Ok(()) => {
                    features::on_config_changed(&core);
                    crate::reload::on_config_changed(&core);
                }
                Err(error) => tracing::warn!(%error, "the changed config was not applied"),
            }
        }) {
            Ok(watch) => *lock(&WATCH) = Some(watch),
            Err(error) => tracing::warn!(%error, "the config is not watched"),
        }
    }
    ipc::spawn_state_forwarder(&host, &launch.runtime, move |script| {
        ui.send_to_shell(script)
    });
}

/// Stops the config watch and hot reload (before CEF shuts down and the runtime stops).
pub fn stop() {
    super::ui_watchdog::stop();
    lock(&WATCH).take();
    crate::reload::on_shutdown();
}

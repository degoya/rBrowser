//! The host's declarative interception (RB-070-02, `docs/plugins.md`, "Network rules"): a
//! plugin granted `network-rules` never sits in the request path. The host asks it for its rules
//! (`@rules`, `crate::plugins::network`), validates them ([`rule`]) and applies them to every
//! Chromium view through CDP `Fetch` ([`executor`]): a mock is `Fetch.fulfillRequest`, a block
//! `Fetch.failRequest`, a rewrite `Fetch.continueRequest`. Rules made from project content apply
//! only in a trusted workspace ([`state::Rules::set_trusted`]); the person's own block list
//! applies always. The same document carries the session's cache switch
//! (`Network.setCacheDisabled`) and its "clear site data" counter (`Storage.clearDataForOrigin`).
//!
//! Firefox (BiDi `network.addIntercept`) and WebKit (the sidecar's `page.route`) are not
//! intercepted yet: the `$network` state marks their views unsupported.
//!
//! The HTTP debugger (RB-080-37) builds on the same loop: breakpoints hold a request or a
//! response for the person ([`held`], [`answer`]), rules may rewrite the response, serve a local
//! file ([`local`]) or swap the host ([`rewrite`]), and the plugin's asks ([`document`]) run a
//! continue, an abort, a compose or a repeat ([`compose`], [`runner`]) - everything that leaves
//! the window masked ([`mask`]).

pub mod action;
pub mod answer;
pub mod compose;
pub mod debug;
pub mod document;
pub mod executor;
pub mod fetch;
pub mod glob;
pub mod held;
pub mod local;
pub mod mask;
pub mod rewrite;
pub mod rule;
pub mod runner;
pub mod state;

use std::sync::{Arc, Mutex, OnceLock, Weak};

use rb_engine::{ViewHost as _, ViewId};
use tokio::sync::watch;

use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;
use state::Rules;

/// The argument the state arrives under in every call of a plugin granted `network-rules`.
pub const STATE_KEY: &str = "$network";

/// The rules of one window and the loop applying them.
struct Running {
    host: Weak<Host>,
    rules: Arc<Rules>,
    /// Dropped with the entry: the loop stops.
    _stop: watch::Sender<bool>,
}

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

/// A random id of this run of the app: the plugin keeps its session switches (cache off, the
/// clear counter) only for the session that set them.
#[must_use]
pub fn session_id() -> &'static str {
    static SESSION: OnceLock<String> = OnceLock::new();
    SESSION.get_or_init(|| {
        let mut bytes = [0u8; 8];
        if getrandom::fill(&mut bytes).is_err() {
            // Not a secret: any value distinct from the last run's serves.
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| {
                    elapsed.as_secs() ^ u64::from(elapsed.subsec_nanos())
                });
            bytes = now.to_le_bytes();
        }
        rb_util::hex(&bytes)
    })
}

impl executor::Window for Host {
    fn sessions(&self) -> Vec<(ViewId, String)> {
        self.cdp_sessions()
    }

    fn urls(&self) -> Vec<(ViewId, String)> {
        self.views()
            .into_iter()
            .map(|view| (view.id, view.url))
            .collect()
    }

    fn project_root(&self) -> Option<std::path::PathBuf> {
        Host::project_root(self)
    }
}

/// Starts the rules of `core`'s window (once per window) and answers them.
pub fn start(core: &Core) -> Arc<Rules> {
    let mut running = lock(&RUNNING);
    if let Some(entry) = running.as_ref()
        && std::ptr::eq(entry.host.as_ptr(), Arc::as_ptr(core.host()))
    {
        return Arc::clone(&entry.rules);
    }
    let weak = Arc::downgrade(core.host());
    let views = weak.clone();
    let rules = Arc::new(Rules::new(
        Box::new(move || {
            views
                .upgrade()
                .map(|host| {
                    host.views()
                        .into_iter()
                        .map(|view| (view.id, view.engine))
                        .collect()
                })
                .unwrap_or_default()
        }),
        session_id().to_owned(),
    ));
    rules.set_trusted(core.host().shell_state().trusted);
    rules.set_runner(Arc::downgrade(&rules), runner::for_host(core.host()));
    let (stop, stopped) = watch::channel(false);
    let host = core.host();
    let window: Arc<dyn executor::Window> = Arc::clone(host) as Arc<dyn executor::Window>;
    core.runtime().spawn(executor::run(
        Arc::clone(&rules),
        Arc::clone(host.cdp()),
        window,
        host.subscribe(),
        stopped,
    ));
    *running = Some(Running {
        host: weak,
        rules: Arc::clone(&rules),
        _stop: stop,
    });
    rules
}

/// The rules of the running window, once [`start`] ran.
#[must_use]
pub fn current() -> Option<Arc<Rules>> {
    lock(&RUNNING)
        .as_ref()
        .filter(|entry| entry.host.strong_count() > 0)
        .map(|entry| Arc::clone(&entry.rules))
}

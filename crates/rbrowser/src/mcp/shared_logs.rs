//! One set of log buffers ([`rb_mcp::LogStore`]) and one collector (`super::collect`) per window,
//! shared by the MCP server and the plugins granted `console-logs` (`crate::plugins::console`)
//! (RB-090-34): each holder keeps an [`Arc`] of [`SharedLogs`], the first one starts the
//! collector, and the collector stops when the last one lets go. The sizes and the network switch
//! come from the user settings (`mcp.consoleLogLimit`, `mcp.networkRequestLimit`,
//! `mcp.errorLimit`, `mcp.collectNetwork`).

use std::sync::{Arc, Mutex, Weak};

use rb_mcp::LogStore;
use tokio::sync::watch;

use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;

/// The buffers of one window and its collector.
pub struct SharedLogs {
    host: Weak<Host>,
    store: Arc<LogStore>,
    /// Dropped with the last holder: the collector stops.
    _stop: watch::Sender<bool>,
}

impl SharedLogs {
    /// The buffers.
    #[must_use]
    pub fn store(&self) -> &Arc<LogStore> {
        &self.store
    }

    /// The window the buffers follow.
    #[must_use]
    pub fn host(&self) -> &Weak<Host> {
        &self.host
    }

    fn is_of(&self, host: &Arc<Host>) -> bool {
        std::ptr::eq(self.host.as_ptr(), Arc::as_ptr(host))
    }
}

static SHARED: Mutex<Weak<SharedLogs>> = Mutex::new(Weak::new());

/// The buffers of `core`'s window: the ones another holder keeps, or new ones with their
/// collector.
pub fn acquire(core: &Core) -> Arc<SharedLogs> {
    let mut slot = lock(&SHARED);
    shared_or_new(
        &mut slot,
        |logs| logs.is_of(core.host()),
        || {
            let settings = core.user_settings().mcp;
            let store = Arc::new(LogStore::new(super::backend::log_limits(&settings)));
            if settings.collect_network {
                store.want_network();
            }
            let (stop, stopped) = watch::channel(false);
            super::collect::spawn(core, Arc::clone(&store), stopped);
            SharedLogs {
                host: Arc::downgrade(core.host()),
                store,
                _stop: stop,
            }
        },
    )
}

/// The user settings changed: the held buffers take the new sizes, and the requests are
/// collected from now on when `mcp.collectNetwork` says so (switched off, they stay on until the
/// buffers go).
pub fn apply_settings(settings: &rb_config::McpSettings) {
    let Some(shared) = lock(&SHARED).upgrade() else {
        return;
    };
    shared
        .store
        .set_limits(super::backend::log_limits(settings));
    if settings.collect_network {
        shared.store.want_network();
    }
}

/// What `slot` holds when it is still alive and `same`, else a new one from `make`, kept there.
fn shared_or_new<T>(
    slot: &mut Weak<T>,
    same: impl Fn(&T) -> bool,
    make: impl FnOnce() -> T,
) -> Arc<T> {
    if let Some(held) = slot.upgrade().filter(|held| same(held)) {
        return held;
    }
    let fresh = Arc::new(make());
    *slot = Arc::downgrade(&fresh);
    fresh
}

#[cfg(test)]
#[path = "shared_logs_tests.rs"]
mod tests;

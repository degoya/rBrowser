//! Chrome on an Android device on USB as a view of the grid (RB-095-02): over ADB, with the full
//! CDP of Chrome - navigation, screenshots, input - on real hardware. The device's breakpoint
//! (`android-<serial>`, written by `view.add_android`) names the engine `android`; the view has
//! the device's own size and emulates nothing. The Chromium stand-in in the cell shows the page
//! at that size and its navigations are followed on the device ([`follow`]).
//!
//! | Part | Where |
//! | --- | --- |
//! | Which adb: the setting, PATH, ANDROID_HOME, ANDROID_SDK_ROOT, the usual SDK folders, the downloaded one | [`locate`] |
//! | The Platform Tools downloaded after consent, never shipped (`tools.adb.install`) | [`adb_tool`] |
//! | Running adb; the device list and screen; the forward | [`adb`], [`devices`], [`forward`] |
//! | The forward on 127.0.0.1 only, checked; Chrome's socket dialled on loopback | [`forward`], [`engine`] |
//! | CDP over the forward | [`cdp`] |
//! | A view's loopback port reversed to this machine (RB-096-12) | [`reverse`], [`reach`], [`reverse_ledger`] |
//! | The engine and its views (`rb_engine::Engine`, `ViewHandle`) | [`engine`], [`view`] |
//! | `view.add_android` (`Leader V ;`), MCP `view_add_android`, `android_devices` | [`commands`] |
//! | The devices of the main menu and the toolbar's "+" (RB-094-20) | [`menu`] |
//!
//! Settings: `engines.android.enabled` (off), `engines.android.adbPath` and
//! `engines.android.reverseLocalhost` (on), all the person's.
//! Not yet: the device's screencast in the cell, and the sync's replay of clicks and keys into
//! the device (the engine has no `rb_sync::Replay`, so events to it are skipped, never desync).

pub mod adb;
pub mod adb_tool;
pub mod cdp;
pub mod commands;
pub mod devices;
pub mod engine;
mod follow;
pub mod forward;
pub mod locate;
pub mod menu;
pub mod reach;
pub mod reverse;
pub mod reverse_ledger;
pub mod view;

#[cfg(test)]
pub(crate) mod fake_chrome;

use std::sync::{Arc, Mutex, Weak};

use rb_config::EngineKind;
use rb_engine::external::ExternalStatus;

use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;

pub use engine::AndroidEngine;

/// The Android engine of one window.
struct HostAndroid {
    host: Weak<Host>,
    engine: Arc<AndroidEngine>,
}

fn registry() -> &'static Mutex<Vec<HostAndroid>> {
    static REGISTRY: Mutex<Vec<HostAndroid>> = Mutex::new(Vec::new());
    &REGISTRY
}

/// The Android engine of `host`'s window, once it started.
pub fn for_host(host: &Host) -> Option<Arc<AndroidEngine>> {
    let mut entries = lock(registry());
    entries.retain(|entry| entry.host.strong_count() > 0);
    entries
        .iter()
        .find(|entry| std::ptr::eq(entry.host.as_ptr(), host))
        .map(|entry| entry.engine.clone())
}

/// The engine of `core`'s window with the user's settings; starts nothing.
pub(crate) fn prepare(core: &Core) -> Arc<AndroidEngine> {
    AndroidEngine::new(
        core.user_settings().engines.android,
        adb_tool::default_store(),
    )
}

/// Plugs in after the external windows started: the follow task and the registry.
pub(crate) fn attach(core: &Core, engine: Arc<AndroidEngine>) {
    follow::watch(core.host(), &engine, core.runtime());
    // A loopback address the device cannot reach is told in this window (RB-096-12).
    if let Some(me) = core.arc() {
        let weak: Weak<Core> = Arc::downgrade(&me);
        engine.set_notifier(Arc::new(move |message: rb_ipc::CoreToShell| {
            if let Some(core) = weak.upgrade() {
                core.notify(message);
            }
        }));
    }
    lock(registry()).push(HostAndroid {
        host: Arc::downgrade(core.host()),
        engine,
    });
    menu::refresh(core);
}

/// The app quits: every reverse rBrowser holds goes before the windows close, within `wait`
/// of its own (security review S2 of RB-096-12: the windows' close shares a few seconds, and
/// one adb call may take longer).
pub(crate) fn clear_on_quit(runtime: &tokio::runtime::Runtime, wait: std::time::Duration) {
    let Some(reverses) = reach::Reverses::existing() else {
        return;
    };
    let done = runtime.block_on(async move { tokio::time::timeout(wait, reverses.clear()).await });
    if done.is_err() {
        tracing::warn!("the adb reverses were not all removed in time");
    }
}

/// The settings changed: the engine takes them, and switched on (or given an adb) the views
/// that could not start before start again.
pub fn on_config_changed(core: &Core) {
    let Some(engine) = for_host(core.host()) else {
        return;
    };
    engine.set_settings(core.user_settings().engines.android);
    // Android or the reverses switched off: every reverse rBrowser set goes (RB-096-12).
    if !engine.reverses_wanted() {
        let clearing = engine.clone();
        core.runtime()
            .spawn(async move { clearing.clear_reverses().await });
    }
    // The menus' devices follow the switch and the adb path (RB-094-20).
    menu::refresh(core);
    if !engine.is_enabled() {
        return;
    }
    let Some(engines) = super::for_host(core.host()) else {
        return;
    };
    let windows = engines.windows();
    for view in windows.list() {
        let idle = matches!(
            view.status,
            ExternalStatus::Unavailable | ExternalStatus::Failed
        );
        if view.engine == EngineKind::Android
            && idle
            && let Err(error) = windows.restart(EngineKind::Android, Some(view.view))
        {
            tracing::debug!(view = %view.view, %error, "the Android view did not start");
        }
    }
}

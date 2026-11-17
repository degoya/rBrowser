//! Self-update with the Cargo feature `self-update` and `updates.enabled` on: at the start, off
//! the CEF UI thread on the core's runtime (never `Handle::current()` there), the switched update
//! this version is the target of is proven (`confirm_started`), and with `updates.check` at
//! `onStart` the channel's manifest is checked and only logged. Phase 1 ships the placeholder
//! root, so the check ends at `update.not_configured` before any request. Offering and
//! installing arrive with RB-110-05.

use rb_config::{UpdateChannel, UpdateCheck};
use rb_update::{Channel, Gate, HttpFetcher};

use crate::control::Core;

/// Where the newest release's assets are served from after the move to GitHub (RB-090-12).
const RELEASE_BASE: &str = "https://github.com/degoya/rBrowser/releases/latest/download/";

pub(super) fn on_start(core: &Core) {
    let settings = core.user_settings();
    let Some(data) = rb_config::local_dir() else {
        tracing::warn!("no local data folder; self-update does nothing");
        return;
    };
    let check = settings.updates.check == UpdateCheck::OnStart;
    let channel = match settings.updates.channel {
        UpdateChannel::Stable => Channel::Stable,
        UpdateChannel::Beta => Channel::Beta,
    };
    let gate = Gate {
        feature_built: true,
        enabled: settings.updates.enabled,
    };
    let version = env!("CARGO_PKG_VERSION");
    core.runtime().spawn_blocking(move || {
        // Only the program the update switched proves it, not a build sharing the data.
        let proven = match std::env::current_exe() {
            Ok(exe) => rb_update::install::recover::confirm_started(&data, &exe, version)
                .map_err(|error| format!("{error:#}")),
            Err(error) => Err(format!("the running program is unknown: {error}")),
        };
        match proven {
            Ok(true) => tracing::info!(version, "the update to this version is proven"),
            Ok(false) => {}
            Err(error) => tracing::warn!(%error, "the update journal could not be read"),
        }
        if !check {
            return;
        }
        let fetcher = HttpFetcher::new();
        match rb_update::check(
            gate,
            &fetcher,
            RELEASE_BASE,
            channel,
            &data,
            rb_update::Utc::now(),
        ) {
            Ok(manifest) if manifest.updates_from(version) => {
                tracing::info!(offered = %manifest.version, "a newer rBrowser is published");
            }
            Ok(_) => tracing::debug!("rBrowser is up to date"),
            Err(error) => tracing::info!(code = error.code(), "no update check: {error}"),
        }
    });
}

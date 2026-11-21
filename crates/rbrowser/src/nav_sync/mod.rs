//! Navigation sync in the app (RB-020-06): [`NavSyncHost`] runs `rb_sync::NavigationSync` for
//! one window. When input sync does not take the followers to the leader's page within
//! `sync.navigationSettleMs`, they load it (a SPA route moves through the history API, the
//! agent's `navigate` command); explicit navigation - the toolbar, the address bar, back,
//! forward, the shortcuts and MCP with `target: "all"` - reaches every view whose navigation is
//! coupled and names the others as skipped (an explicit reload reaches every view: each reloads
//! its own page, so coupling does not matter); a view with `breakpoints[].syncNavigation: false` (or decoupled with
//! `Leader S U`) keeps its own page and gets its own address field in its label strip; and
//! Browsersync's ghost mode in a page switches sync off with a notice (`sync.detectBrowsersync`).
//!
//! | Part | Where |
//! | --- | --- |
//! | When to check, who follows how (no runtime) | `rb_sync::NavigationSync`, `rb_sync::nav_url` |
//! | Timer, CDP `Page.navigatedWithinDocument`, the follows | [`NavSyncHost`] |
//! | The Browsersync guard | [`BrowsersyncGuard`] |
//! | Commands (`sync.toggle_view_navigation`; back, forward, navigate, reload) | [`commands`] |
//! | What `views_list` and `sync_set` need (RB-030-01) | [`mcp`] |
//! | A decoupled view's own address field (RB-092-03) | `crate::cell_info::edit`, `cef_app/address_field.rs` |
//! | The Browsersync notice in the shell | `ui/src/sync/` |

mod browsersync;
pub mod commands;
mod glue;
pub mod mcp;

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use rb_config::ProjectConfig;
use rb_engine::{EngineError, Target, ViewHost, ViewId};
use rb_sync::NavOptions;
use serde::Serialize;

use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;
use crate::host::nav::normalize_url;

pub use browsersync::{BrowsersyncGuard, GuardAction};
pub use commands::run_command;
pub use glue::NavSyncHost;

/// `Leader S U`: the navigation of the active view (`{ "view"?: id, "enabled"?: bool }`)
/// decoupled or coupled again; a flip without `enabled`.
pub const TOGGLE_VIEW_NAVIGATION: &str = "sync.toggle_view_navigation";
/// The i18n key of the notice when Browsersync switched sync off; placeholder `{origin}`.
pub const BROWSERSYNC_PAUSED_KEY: &str = "sync.browsersync.paused";
/// The setting behind the Browsersync guard.
pub const DETECT_BROWSERSYNC_KEY: &str = "sync.detectBrowsersync";

/// The navigation sync of every live window, by host.
fn registry() -> &'static Mutex<Vec<(Weak<Host>, Arc<NavSyncHost>)>> {
    static REGISTRY: Mutex<Vec<(Weak<Host>, Arc<NavSyncHost>)>> = Mutex::new(Vec::new());
    &REGISTRY
}

/// The navigation sync of `host`'s window, once [`on_start`] ran for it.
pub fn for_host(host: &Host) -> Option<Arc<NavSyncHost>> {
    let mut entries = lock(registry());
    entries.retain(|(weak, _)| weak.strong_count() > 0);
    entries
        .iter()
        .find(|(weak, _)| std::ptr::eq(weak.as_ptr(), host))
        .map(|(_, nav)| nav.clone())
}

/// Starts the navigation sync of `core`'s window (after the SyncHub, `sync_hub::on_start`).
pub fn on_start(core: &Core) {
    if for_host(core.host()).is_some() {
        return;
    }
    let nav = NavSyncHost::start(core);
    lock(registry()).push((Arc::downgrade(core.host()), nav));
}

/// The project config changed: the options, the decoupled views and the guard follow.
pub fn on_config_changed(core: &Core) {
    if let Some(nav) = for_host(core.host()) {
        nav.config_changed();
    }
}

/// What the shell and MCP show of navigation sync, in the sync state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NavStatus {
    /// The views whose navigation is decoupled, in grid order.
    pub decoupled: Vec<ViewId>,
    /// The site whose Browsersync switched sync off, while its notice stands.
    pub browsersync: Option<String>,
}

/// `host`'s navigation sync status; empty before it runs.
pub fn status(host: &Host) -> NavStatus {
    for_host(host).map(|nav| nav.status()).unwrap_or_default()
}

/// The options of `config`: `sync.navigationIncludeHash`, `sync.navigationSettleMs` and the
/// environments' base URLs.
pub fn options_from(config: &ProjectConfig) -> NavOptions {
    NavOptions {
        include_hash: config.sync.navigation_include_hash,
        settle: Duration::from_millis(u64::from(config.sync.navigation_settle_ms)),
        environments: config
            .environments
            .iter()
            .map(|environment| environment.url.clone())
            .collect(),
    }
}

/// The views of `host` whose breakpoint has `syncNavigation: false` in `config` (the shown
/// breakpoint, else the cell's).
pub fn configured_decoupled(host: &Host, config: &ProjectConfig) -> Vec<ViewId> {
    host.views()
        .iter()
        .filter(|info| {
            info.breakpoint
                .as_ref()
                .or(info.cell_breakpoint.as_ref())
                .and_then(|id| config.breakpoints.iter().find(|bp| &bp.id == id))
                .is_some_and(|breakpoint| !breakpoint.sync_navigation)
        })
        .map(|info| info.id)
        .collect()
}

/// The views an explicit navigation of `target` reaches: a view id is that view; `all` is every
/// view whose navigation is coupled - the active view when none is (every view before
/// navigation sync runs).
pub fn targets(host: &Host, target: Target) -> Result<Vec<ViewId>, EngineError> {
    Ok(split_targets(host, target)?.0)
}

/// [`targets`], and the views of `all` it leaves alone because their navigation is decoupled.
fn split_targets(host: &Host, target: Target) -> Result<(Vec<ViewId>, Vec<ViewId>), EngineError> {
    let views = host.target_views(Some(target))?;
    let (Target::All, Some(nav)) = (target, for_host(host)) else {
        return Ok((views, Vec::new()));
    };
    let (coupled, decoupled): (Vec<ViewId>, Vec<ViewId>) =
        views.into_iter().partition(|view| nav.is_coupled(*view));
    if coupled.is_empty() {
        let active = host.target_views(None)?;
        let skipped = decoupled
            .into_iter()
            .filter(|view| !active.contains(view))
            .collect();
        return Ok((active, skipped));
    }
    Ok((coupled, decoupled))
}

/// What an explicit navigation reached (the answer of `view.navigate`, `view.back`,
/// `view.forward` and `reload.all`; MCP's navigation tools wait on `views`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavOutcome {
    /// The views it reached, in grid order.
    pub views: Vec<ViewId>,
    /// The views of `all` it left alone: their navigation is decoupled.
    pub skipped: Vec<ViewId>,
    /// The views where it failed, and why; the others went ahead.
    pub failed: Vec<ViewFailure>,
}

/// A view an explicit navigation failed in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ViewFailure {
    /// The view.
    pub view: ViewId,
    /// Why.
    pub error: String,
}

/// Runs `step` in each of `views`; one view's failure stops none of the others. An error only
/// when it failed in every view.
fn per_view(
    views: Vec<ViewId>,
    skipped: Vec<ViewId>,
    mut step: impl FnMut(ViewId) -> Result<(), EngineError>,
) -> Result<NavOutcome, EngineError> {
    let mut outcome = NavOutcome {
        skipped,
        ..NavOutcome::default()
    };
    let mut first_error = None;
    for view in views {
        match step(view) {
            Ok(()) => outcome.views.push(view),
            Err(error) => {
                tracing::warn!(%view, %error, "explicit navigation failed");
                outcome.failed.push(ViewFailure {
                    view,
                    error: error.to_string(),
                });
                first_error.get_or_insert(error);
            }
        }
    }
    match first_error {
        Some(error) if outcome.views.is_empty() => Err(error),
        _ => Ok(outcome),
    }
}

/// Loads `url` in the views of `target` ([`targets`]).
pub fn navigate(host: &Host, target: Target, url: &str) -> Result<NavOutcome, EngineError> {
    let url = normalize_url(url)?;
    let (views, skipped) = split_targets(host, target)?;
    per_view(views, skipped, |view| {
        host.navigate(Target::View(view), &url)
    })
}

/// History back in the views of `target`.
pub fn back(host: &Host, target: Target) -> Result<NavOutcome, EngineError> {
    let (views, skipped) = split_targets(host, target)?;
    per_view(views, skipped, |view| host.go_back(Target::View(view)))
}

/// History forward in the views of `target`.
pub fn forward(host: &Host, target: Target) -> Result<NavOutcome, EngineError> {
    let (views, skipped) = split_targets(host, target)?;
    per_view(views, skipped, |view| host.go_forward(Target::View(view)))
}

/// Reloads the views of `target` - with `all` every view, the decoupled ones too: each reloads
/// its own page, which keeps it where it is.
pub fn reload(host: &Host, target: Target, ignore_cache: bool) -> Result<NavOutcome, EngineError> {
    let views = host.target_views(Some(target))?;
    per_view(views, Vec::new(), |view| {
        host.reload(Target::View(view), ignore_cache)
    })
}

/// Couples `view`'s navigation or decouples it (`Leader S U`, `sync_set`); `true` when that
/// changed.
pub fn couple(host: &Host, view: ViewId, coupled: bool) -> Result<bool, EngineError> {
    match for_host(host) {
        Some(nav) => nav.couple(view, coupled),
        None => Err(EngineError::Invalid(
            "navigation sync is not started".to_owned(),
        )),
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

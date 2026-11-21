//! The pages of the last session (RB-092-04): every view's page is kept in the project's
//! browser profile - when the window closes and every [`SAVE_EVERY`] while it runs, so a crash
//! loses little - and, with `startup.restore = last`, opened again when the project opens; a
//! view whose navigation was decoupled (RB-020-06) is decoupled again. A URL on the command line
//! always wins. `views.restore_last` (`Leader R L`, MCP `views_restore_last`) opens the pages of
//! the last session at any time, in every view or in one.
//!
//! | Part | Where |
//! | --- | --- |
//! | What a view keeps, the file's check, which view an entry belongs to, the start rule | [`model`] |
//! | The file in the profile, written atomically | [`store`] |
//! | The command, its chord and tool | [`defs`] |
//! | The setting `startup.restore` | `rb_config::StartupRestore` |
//! | The tool `views_restore_last` | `rb_mcp::tools::restore_last` |
//!
//! Privacy: the pages are kept only in the profile under the local app data folder (beside
//! Chromium's own history of the same pages), never in `config.json` or `config.local.json`,
//! outside what the diagnostics ZIP collects, and no log line or error names a URL - they may
//! carry tokens.

pub mod defs;
pub mod model;
pub mod store;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rb_commands::CommandError;
use rb_engine::{EngineError, Target, ViewHost, ViewId};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;

pub use defs::{RESTORE_LAST, TOOL, register};
pub use model::{LastPages, restores_at_start};

/// How often the pages are kept while the window is open.
pub const SAVE_EVERY: Duration = Duration::from_secs(15);
/// The i18n key of the notice when no pages of a last session are kept.
pub const NONE_KEY: &str = "project.restore_last.none";
/// The setting the notice points to.
const SETTING_KEY: &str = "startup.restore";

/// A URL was given on the command line: the start opens it, never the last pages.
static URL_ON_COMMAND_LINE: AtomicBool = AtomicBool::new(false);
/// The window closed: nothing is kept any more (the views go one by one).
static CLOSED: AtomicBool = AtomicBool::new(false);
/// The pages of the last session, read once at the start.
static PREVIOUS: Mutex<Option<LastPages>> = Mutex::new(None);
/// What was written last, so an unchanged state is not written again.
static WRITTEN: Mutex<Option<LastPages>> = Mutex::new(None);

/// Records, before the window opens, whether a URL was given on the command line.
pub fn set_url_on_command_line(given: bool) {
    URL_ON_COMMAND_LINE.store(given, Ordering::Release);
}

/// The core is up, after the environment in effect (`presets::on_start`) and navigation sync
/// (`nav_sync::on_start`): the last session's pages are read, opened with `startup.restore =
/// last`, and the keeping starts. Nothing without a profile (tests).
pub fn on_start(core: &Core) {
    let Some(profile) = crate::sessions::profile() else {
        return;
    };
    let previous = store::read(profile);
    lock(&PREVIOUS).clone_from(&previous);
    let setting = core.user_settings().startup.restore;
    if let Some(pages) = previous
        && restores_at_start(setting, URL_ON_COMMAND_LINE.load(Ordering::Acquire))
    {
        match apply(core.host(), &pages, Target::All) {
            Ok(applied) => tracing::info!(
                views = applied.views.len(),
                decoupled = applied.decoupled.len(),
                "the views open the pages of the last session"
            ),
            Err(error) => tracing::warn!(%error, "the last pages cannot be opened"),
        }
    }
    spawn_keeping(core);
}

/// The main window is asked to close (UI thread, before the views go): the pages are kept a
/// last time and the keeping stops.
pub fn on_closing(host: &Host) {
    if CLOSED.swap(true, Ordering::AcqRel) {
        return;
    }
    keep(host);
}

/// The arguments of `views.restore_last`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Args {
    /// `"all"` or a view id; every view when absent.
    #[serde(default)]
    target: Option<Target>,
}

/// What [`apply`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    /// The views that open their last page, in grid order.
    pub views: Vec<ViewId>,
    /// The views decoupled again.
    pub decoupled: Vec<ViewId>,
}

/// Runs `views.restore_last`: the last session's pages in the views of `target` (every view
/// when absent). Answers `views`, `decoupled` and `stored` (whether a last session is kept);
/// without one (or an empty one) the status bar says so.
pub fn run(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let args: Args = if args.is_null() {
        Args::default()
    } else {
        serde_json::from_value(args.clone())
            .map_err(|error| CommandError::InvalidArgs(error.to_string()))?
    };
    let previous = lock(&PREVIOUS).clone();
    let applied = match &previous {
        Some(pages) => {
            apply(core.host(), pages, args.target.unwrap_or(Target::All)).map_err(failed)?
        }
        None => Applied::default(),
    };
    let stored = previous
        .as_ref()
        .is_some_and(|pages| !pages.views.is_empty());
    if !stored {
        core.notify(CoreToShell::Notification(Notification {
            level: NotificationLevel::Info,
            message_key: NONE_KEY.to_owned(),
            args: std::collections::BTreeMap::new(),
            settings_key: Some(SETTING_KEY.to_owned()),
        }));
    }
    Ok(serde_json::json!({
        "views": applied.views,
        "decoupled": applied.decoupled,
        "stored": stored,
    }))
}

/// Opens `pages` in the views of `target` against the environment in effect, decoupling first
/// what was decoupled, so a coupled view does not take the others along. One view's failure
/// stops none of the others; the log names the view, never the URL.
pub(crate) fn apply(
    host: &Host,
    pages: &LastPages,
    target: Target,
) -> Result<Applied, EngineError> {
    let wanted = host.target_views(Some(target))?;
    let infos = host.views();
    let views: Vec<(ViewId, Option<&str>)> = infos
        .iter()
        .map(|info| (info.id, info.cell_breakpoint.as_deref()))
        .collect();
    let config = host.config();
    let environment = host.shell_state().environment;
    let restores = model::resolve(pages, &views, &config, environment.as_deref());
    let mut applied = Applied::default();
    for restore in restores.iter().filter(|r| wanted.contains(&r.view)) {
        if restore.decoupled {
            match crate::nav_sync::couple(host, restore.view, false) {
                Ok(_) => applied.decoupled.push(restore.view),
                Err(error) => {
                    tracing::warn!(view = %restore.view, %error, "a view is not decoupled again");
                }
            }
        }
    }
    for restore in restores.iter().filter(|r| wanted.contains(&r.view)) {
        let Some(url) = &restore.url else {
            continue;
        };
        match host.navigate(Target::View(restore.view), url) {
            Ok(()) => applied.views.push(restore.view),
            // The error may quote the URL: only the view is named.
            Err(_) => tracing::warn!(view = %restore.view, "a view cannot open its last page"),
        }
    }
    Ok(applied)
}

/// What `host`'s views keep now, in grid order; a view without a page to keep keeps its entry
/// of `before` ([`model::capture_over`]).
pub(crate) fn snapshot(host: &Host, before: Option<&LastPages>) -> LastPages {
    let infos = host.views();
    let decoupled = crate::nav_sync::status(host).decoupled;
    let views: Vec<model::ViewSnapshot<'_>> = infos
        .iter()
        .map(|info| model::ViewSnapshot {
            cell_breakpoint: info.cell_breakpoint.as_deref(),
            url: &info.url,
            decoupled: decoupled.contains(&info.id),
        })
        .collect();
    let config = host.config();
    let environment = host.shell_state().environment;
    model::capture_over(&views, &config, environment.as_deref(), before)
}

/// Writes what the views keep into the profile, unless it is unchanged or empty. A view without
/// a page to keep now (still loading, blank, an error page) keeps its entry from before, and an
/// empty state keeps the whole file.
fn keep(host: &Host) {
    let Some(profile) = crate::sessions::profile() else {
        return;
    };
    let mut written = lock(&WRITTEN);
    // What was written last, else the last session's file read at the start.
    let before = written.clone().or_else(|| lock(&PREVIOUS).clone());
    let pages = snapshot(host, before.as_ref());
    if pages.views.is_empty() {
        return;
    }
    if written.as_ref() == Some(&pages) {
        return;
    }
    match store::write(profile, &pages) {
        Ok(()) => *written = Some(pages),
        Err(error) => tracing::warn!(%error, "the last pages cannot be kept"),
    }
}

/// Keeps the pages every [`SAVE_EVERY`] on the core's runtime, off its workers, until the
/// window closes.
fn spawn_keeping(core: &Core) {
    let Some(strong) = core.arc() else {
        return;
    };
    let weak = Arc::downgrade(&strong);
    drop(strong);
    core.runtime().spawn(async move {
        let start = tokio::time::Instant::now() + SAVE_EVERY;
        let mut ticks = tokio::time::interval_at(start, SAVE_EVERY);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticks.tick().await;
            if CLOSED.load(Ordering::Acquire) {
                break;
            }
            let Some(core) = weak.upgrade() else {
                break;
            };
            let host = Arc::clone(core.host());
            drop(core);
            if tokio::task::spawn_blocking(move || keep(&host))
                .await
                .is_err()
            {
                break;
            }
        }
    });
}

fn failed(error: EngineError) -> CommandError {
    match error {
        EngineError::UnknownView(_) | EngineError::Invalid(_) => {
            CommandError::InvalidArgs(error.to_string())
        }
        other => CommandError::Failed(other.to_string()),
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

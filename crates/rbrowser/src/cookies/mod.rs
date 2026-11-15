//! Copying cookies (RB-040-05): the cookies of a Chromium session go into the Firefox and
//! WebKit views of that session, so a login in the grid holds in their windows too - Chromium's
//! views share one cookie jar per session (RB-020-07), Firefox and WebKit have their own.
//!
//! | Part | Where |
//! | --- | --- |
//! | A CDP cookie, the arguments, the report (counts and reasons, never a value) | [`model`] |
//! | Chromium's cookie as Firefox and WebKit take it (the mapping table) | [`mapping`] |
//! | Which cookies: the sites shown, the environment, `domains`, the exclusion list | [`filter`] |
//! | Which views: the targets by session, the views a session is read through | [`plan`] |
//! | The run: read once per session, set per engine, reload | [`copy`] |
//! | The window's jars: CDP, `FirefoxEngine::set_cookies`, the sidecar's `webkit.view.cookies` | [`jars`] |
//! | `engines.cookies.copy` `onStart` and `onNavigate` | [`auto`] |
//! | The command `cookies.copy` (`Leader E J`), MCP `cookies_copy` | [`defs`], `rb_mcp::tools::cookies` |
//!
//! The values are secrets: they travel from the CDP answer into the engines' commands and
//! nowhere else - no log line, error, notification or answer names one, and every type that
//! holds one hides it in its `Debug`. A Firefox view of a named session lives in that session's
//! own user context (`rb_bidi::storage`), so the cookies of `admin` reach only `admin`'s views.

pub mod auto;
pub mod copy;
pub mod defs;
pub mod filter;
pub mod jars;
pub mod mapping;
pub mod model;
pub mod plan;

#[cfg(test)]
#[path = "log_tests.rs"]
mod log_tests;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rb_commands::CommandError;
use rb_engine::ViewHost;
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::Value;

pub use defs::{COPY, COPY_TOOL, definitions, register};
pub use model::{CopyArgs, CopyReport, Source, Targets};

use crate::control::Core;
use crate::host::Host;
use copy::Options;
use plan::PlanError;

/// The notice after a copy; placeholders `{copied}`, `{views}`, `{skipped}`.
pub const COPIED_KEY: &str = "cookies.notice.copied";
/// The notice when views got nothing; placeholders `{failed}`, `{error}`.
pub const FAILED_KEY: &str = "cookies.notice.failed";
/// The note when there is no Firefox or WebKit view.
pub const NO_VIEWS_KEY: &str = "cookies.noExternalViews";
/// The setting the notices link to.
const SETTINGS_KEY: &str = "engines.cookies.copy";
/// The most sites `domains` may name.
const MAX_DOMAINS: usize = 256;

/// Runs `cookies.copy` (registered by [`register`]): waits for the copy on a thread of the
/// runtime (a shortcut, MCP); on another thread (CEF's UI thread, which CDP needs to answer)
/// it starts the copy and answers `{ "started": true }`. Either way a notice tells the person.
pub fn run(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let args: CopyArgs = if args.is_null() {
        CopyArgs::default()
    } else {
        serde_json::from_value(args.clone())
            .map_err(|error| CommandError::InvalidArgs(error.to_string()))?
    };
    if args
        .domains
        .as_ref()
        .is_some_and(|domains| domains.len() > MAX_DOMAINS)
    {
        return Err(CommandError::InvalidArgs(format!(
            "domains: at most {MAX_DOMAINS} sites"
        )));
    }
    if tokio::runtime::Handle::try_current().is_err() {
        let Some(core) = core.arc() else {
            return Err(CommandError::Failed("the window is closed".to_owned()));
        };
        core.runtime().clone().spawn(async move {
            if let Err(error) = copy_now(&core, args, true).await {
                tracing::warn!(%error, "cookies were not copied");
                notify_failure(&core, &error.to_string());
            }
        });
        return Ok(serde_json::json!({ "started": true }));
    }
    let runtime = core.runtime().clone();
    let report = tokio::task::block_in_place(|| runtime.block_on(copy_now(core, args, true)))?;
    serde_json::to_value(report).map_err(|error| CommandError::Failed(error.to_string()))
}

/// Copies by `args` in `core`'s window and answers the report; `notify` puts the outcome into
/// a notice. Without a Firefox or WebKit view the report says so in its `message`.
pub async fn copy_now(
    core: &Core,
    args: CopyArgs,
    notify: bool,
) -> Result<CopyReport, CommandError> {
    let host = core.host().clone();
    let settings = core.user_settings().engines.cookies;
    let plan = match plan::plan(&host.views(), &args) {
        Ok(plan) => plan,
        Err(PlanError::NoExternalViews) => {
            if notify {
                core.notify(notice(
                    NotificationLevel::Info,
                    NO_VIEWS_KEY,
                    BTreeMap::new(),
                ));
            }
            return Ok(CopyReport {
                message: Some(rb_i18n::t(NO_VIEWS_KEY)),
                ..CopyReport::default()
            });
        }
        Err(error @ (PlanError::UnknownView(_) | PlanError::NotExternal(_))) => {
            return Err(CommandError::InvalidArgs(error.to_string()));
        }
        Err(error @ PlanError::NoReader(_)) => {
            return Err(CommandError::Failed(error.to_string()));
        }
    };
    let options = Options {
        domains: args.domains,
        all_domains: settings.domain_filter == "all",
        environment_host: environment_host(&host),
        exclude: settings.exclude,
        reload: settings.reload,
        now: now(),
    };
    let jars = jars::HostJars { host };
    let report = copy::run(&jars, plan, &options).await;
    tracing::info!(
        copied = report.copied,
        skipped = report.skipped(),
        failed = report.failed(),
        views = report.views.len(),
        "cookies copied into Firefox and WebKit"
    );
    if notify {
        core.notify(outcome_notice(&report));
    }
    Ok(report)
}

/// Starts the automatic copies of `engines.cookies.copy` for `core`'s window.
pub fn on_start(core: &Core) {
    auto::on_start(core);
}

/// The notice of `report`: what was copied, or why views got nothing.
fn outcome_notice(report: &CopyReport) -> CoreToShell {
    let failed = report.failed();
    if failed > 0 {
        let error = report
            .views
            .iter()
            .find_map(|view| view.error.clone())
            .unwrap_or_default();
        let args = BTreeMap::from([
            ("failed".to_owned(), failed.to_string()),
            ("error".to_owned(), error),
        ]);
        return notice(NotificationLevel::Warning, FAILED_KEY, args);
    }
    let args = BTreeMap::from([
        ("copied".to_owned(), report.copied.to_string()),
        ("views".to_owned(), report.views.len().to_string()),
        ("skipped".to_owned(), report.skipped().to_string()),
    ]);
    notice(NotificationLevel::Info, COPIED_KEY, args)
}

fn notify_failure(core: &Core, error: &str) {
    let args = BTreeMap::from([
        ("failed".to_owned(), "1".to_owned()),
        ("error".to_owned(), error.to_owned()),
    ]);
    core.notify(notice(NotificationLevel::Warning, FAILED_KEY, args));
}

fn notice(level: NotificationLevel, key: &str, args: BTreeMap<String, String>) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args,
        settings_key: Some(SETTINGS_KEY.to_owned()),
    })
}

/// The host of the current environment's URL, if one is chosen.
fn environment_host(host: &Arc<Host>) -> Option<String> {
    let current = host.shell_state().environment?;
    let config = host.config();
    let environment = config
        .environments
        .iter()
        .find(|environment| environment.id == current)?;
    filter::cookie_host(&environment.url)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

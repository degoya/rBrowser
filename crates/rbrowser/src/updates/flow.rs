//! The update's flows in the app (RB-090-14): the check, the download, the person's "Install and
//! restart", MCP's request and the quit. Network and disk run on the core's blocking threads,
//! never on CEF's UI thread; every outcome is the process's state (`state.rs`) and a notice
//! (`notices.rs`).
//!
//! "Install and restart" in one click (the pattern of rCommander's RC-100-84): offered and not
//! downloaded, it downloads with the intent to install, verifies, hands over to the updater and
//! closes the window; already downloaded, it hands over at once. With `updates.autoInstall` at
//! `onQuit` an offer is downloaded after the check, and quitting hands a ready download over; a
//! download still running is left alone and quitting goes on. The updater waits for every
//! window of the data folder, so neither the click nor the quit hands over while other projects'
//! windows run: the click says so, the quit leaves the download to the last window. The check
//! itself is `check.rs`.

use std::sync::{Arc, OnceLock};

use rb_config::{UpdateAutoInstall, UpdateChannel, UserSettings};
use rb_ipc::NotificationLevel;
use rb_mcp::ToolFailure;
use rb_update::auto_install::AutoMode;
use rb_update::install::InstallError;
use rb_update::install::handover::{self, Running};
use rb_update::{Channel, HttpFetcher, InstallKind, Offer, Progress, UpdateWay};
use serde_json::{Value, json};

use super::mcp::{CHECK_TOOL, INSTALL_TOOL, STATUS_TOOL};
use super::notices::{self, Notice};
use super::state::{self, Facts, Phase};
use crate::control::Core;

/// The running version.
pub(super) const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Who asked for a check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Origin {
    /// The start (`updates.check` at `onStart`): quiet unless something is offered or refused.
    Start,
    /// A person (`update.check`, `update.install`): every outcome is a notice.
    Person,
    /// An MCP client: the answer is the status; an offer is still shown.
    Mcp,
}

/// How this installation was installed, detected once. A portable folder without the shipped
/// `rbrowser-updater` (a hand-made copy, a macOS bundle) cannot switch itself: it counts as
/// unknown and is offered the download.
pub(super) fn install_kind() -> InstallKind {
    static KIND: OnceLock<InstallKind> = OnceLock::new();
    *KIND.get_or_init(|| {
        let Some(folder) = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
        else {
            return InstallKind::Unknown;
        };
        let kind = rb_update::install_kind::detect(&folder);
        let updater = rb_update::install::process::shipped_updater(&folder);
        if kind.installs_itself() && !updater.is_file() {
            tracing::info!(
                "no rbrowser-updater beside the program; updates are offered as a download"
            );
            return InstallKind::Unknown;
        }
        kind
    })
}

/// The channel this build reads: the person's choice, or with `auto` (the default) beta for a
/// pre-release build and stable otherwise.
pub(super) fn channel(settings: &UserSettings) -> Channel {
    let setting = match settings.updates.channel {
        UpdateChannel::Auto => None,
        UpdateChannel::Stable => Some(Channel::Stable),
        UpdateChannel::Beta => Some(Channel::Beta),
    };
    rb_update::effective_channel(VERSION, setting)
}

pub(super) fn facts(settings: &UserSettings, offered: Option<&str>) -> Facts {
    let failed_before = match (rb_config::local_dir(), offered) {
        (Some(data), Some(version)) => handover::failed_before(&data, version),
        _ => false,
    };
    Facts {
        enabled: settings.updates.enabled,
        installed: VERSION.to_owned(),
        channel: channel(settings),
        kind: install_kind(),
        auto_install: match settings.updates.auto_install {
            UpdateAutoInstall::Off => AutoMode::Off,
            UpdateAutoInstall::OnQuit => AutoMode::OnQuit,
        },
        failed_before,
        other_windows: super::active::other_windows(),
    }
}

fn offered() -> Option<String> {
    state::with(|state| state.offer.as_ref().map(|offer| offer.version.clone()))
}

/// The answer of `update_status`.
pub(super) fn status(settings: &UserSettings) -> Value {
    let facts = facts(settings, offered().as_deref());
    state::with(|state| state.status(&facts))
}

pub(super) fn show(core: &Core, (notification, action): Notice) {
    match action {
        Some(action) => core.notify_with_action(notification, action),
        None => core.notify(rb_ipc::CoreToShell::Notification(notification)),
    }
}

pub(super) fn fail(core: &Core, code: &str, tell: bool) {
    tracing::warn!(code, "the update did not go ahead");
    state::with(|state| {
        state.phase = Phase::Failed;
        state.problem = Some(code.to_owned());
        state.progress = None;
        state.install_when_ready = false;
    });
    if tell {
        show(core, notices::failure(code));
    }
}

/// Downloads `offer`'s file on a blocking thread; `install` hands it over once verified.
pub(super) fn start_download(core: &Arc<Core>, offer: &Offer, install: bool) {
    let (Some(artifact), Some(data)) = (offer.artifact.clone(), rb_config::local_dir()) else {
        show(core, notices::download(offer));
        return;
    };
    let progress = Progress::default();
    let begun = state::with(|state| {
        if state.phase == Phase::Downloading {
            state.install_when_ready |= install;
            return false;
        }
        state.phase = Phase::Downloading;
        state.progress = Some(progress.clone());
        state.install_when_ready = install;
        state.problem = None;
        true
    });
    if !begun {
        return;
    }
    let version = offer.version.clone();
    show(
        core,
        notices::plain(
            NotificationLevel::Info,
            "update.notice.downloading",
            &[
                ("version", version.clone()),
                ("size", notices::megabytes(artifact.size)),
            ],
        ),
    );
    let core = core.clone();
    let runtime = core.runtime().clone();
    runtime.spawn_blocking(move || {
        let folder = handover::download_dir(&data);
        // Another window's or an earlier run's verified download is taken as it is.
        let downloaded = match rb_update::already_downloaded(&artifact, &folder) {
            Some(path) => Ok(path),
            None => rb_update::check_space(&artifact, None).and_then(|()| {
                rb_update::download_verified(&HttpFetcher::new(), &artifact, &folder, &progress)
            }),
        };
        let path = match downloaded {
            Ok(path) => path,
            Err(error) => return fail(&core, error.code(), true),
        };
        let install = state::with(|state| {
            state.phase = Phase::Ready;
            state.ready = Some(path);
            state.progress = None;
            state.install_when_ready
        });
        if install {
            hand_over(&core, true);
        } else {
            show(
                &core,
                notices::plain(
                    NotificationLevel::Info,
                    "update.notice.readyOnQuit",
                    &[("version", version)],
                ),
            );
        }
    });
}

fn running() -> Result<Running, InstallError> {
    let failed = |detail: String| InstallError::new("update.failed", detail);
    let executable = std::env::current_exe()
        .map_err(|error| failed(format!("the running program is unknown: {error}")))?;
    let data_dir =
        rb_config::local_dir().ok_or_else(|| failed("no local data folder".to_owned()))?;
    Ok(Running {
        executable,
        version: VERSION.to_owned(),
        kind: install_kind(),
        data_dir,
        args: std::env::args_os()
            .skip(1)
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect(),
    })
}

/// Hands the ready download to the updater: `restart` closes the window, which ends the app.
fn hand_over_ready(restart: bool) -> Result<String, InstallError> {
    let (offer, ready) = state::with(|state| (state.offer.clone(), state.ready.clone()));
    let (Some(offer), Some(ready)) = (offer, ready) else {
        return Err(InstallError::new("update.failed", "nothing is downloaded"));
    };
    let artifact = offer.artifact.as_ref().ok_or_else(|| {
        InstallError::new("update.failed", "the offer has no file for this build")
    })?;
    let plan = handover::plan(&running()?, &offer.version, artifact, &ready, restart)?;
    handover::hand_over(plan)?;
    state::with(|state| state.phase = Phase::Installing);
    Ok(offer.version)
}

fn hand_over(core: &Core, restart: bool) {
    // The updater waits for every window of the data folder; with other projects' windows open
    // it would wait in vain and fail the version. The download stays; nothing is marked failed.
    let others = super::active::other_windows();
    if others > 0 {
        let version = state::with(|state| {
            state.install_when_ready = false;
            if state.ready.is_some() {
                state.phase = Phase::Ready;
            }
            state.offer.as_ref().map(|offer| offer.version.clone())
        });
        tracing::info!(
            others,
            "other rBrowser windows run; the update waits for them"
        );
        show(
            core,
            notices::other_windows(others, version.as_deref().unwrap_or(VERSION)),
        );
        return;
    }
    match hand_over_ready(restart) {
        Ok(version) => {
            tracing::info!(%version, "the update is handed to the updater; the window closes");
            show(
                core,
                notices::plain(
                    NotificationLevel::Info,
                    "update.notice.installing",
                    &[("version", version)],
                ),
            );
            core.ui().close_window();
        }
        Err(error) => {
            tracing::error!(code = error.code, detail = %error.detail, "the update cannot be handed over");
            fail(core, error.code, true);
        }
    }
}

/// `update.install`, a person's click: see the module documentation.
pub(super) fn install(core: &Core) -> Value {
    let Some(core) = core.arc() else {
        return json!({ "installing": false });
    };
    let phase = state::with(|state| state.phase);
    if phase == Phase::Installing {
        return json!({ "installing": true });
    }
    let runtime = core.runtime().clone();
    runtime.spawn_blocking(move || {
        let offer = match state::with(|state| state.offer.clone()) {
            Some(offer) => offer,
            None => {
                super::check::check_now(&core, Origin::Person);
                match state::with(|state| state.offer.clone()) {
                    Some(offer) => offer,
                    None => return,
                }
            }
        };
        go_ahead(&core, &offer);
    });
    json!({ "installing": true })
}

fn go_ahead(core: &Arc<Core>, offer: &Offer) {
    match install_kind().update_way() {
        UpdateWay::SelfUpdate if offer.artifact.is_some() => {
            if state::with(|state| state.ready.is_some()) {
                hand_over(core, true);
            } else {
                start_download(core, offer, true);
            }
        }
        UpdateWay::SelfUpdate | UpdateWay::DownloadOnly => show(core, notices::download(offer)),
        UpdateWay::Command(_) => show(core, notices::offer(offer, install_kind())),
    }
}

/// The MCP tools, blocking: the status, a check, and the install that asks the person.
pub(super) fn mcp(core: &Arc<Core>, tool: &str) -> Result<Value, ToolFailure> {
    let settings = core.user_settings();
    match tool {
        STATUS_TOOL => Ok(status(&settings)),
        CHECK_TOOL => Ok(super::check::check_now(core, Origin::Mcp)),
        INSTALL_TOOL => {
            if !settings.updates.enabled {
                return Err(
                    ToolFailure::new("update.disabled", "self-update is switched off")
                        .with_setting("updates.enabled"),
                );
            }
            if offered().is_none() {
                super::check::check_now(core, Origin::Mcp);
            }
            let Some(offer) = state::with(|state| state.offer.clone()) else {
                return Ok(
                    json!({ "asked": false, "installed": false, "status": status(&settings) }),
                );
            };
            show(core, notices::mcp_asks(&offer));
            Ok(json!({
                "asked": true,
                "installed": false,
                "offered": offer.version,
                "note": "the person decides with the button in rBrowser's status bar; no tool installs",
            }))
        }
        other => Err(ToolFailure::new(
            "unknown_tool",
            format!("{other} is no update tool"),
        )),
    }
}

/// The quit: hands a ready download over with `onQuit`; see the module documentation.
pub(super) fn on_quit() {
    let settings = crate::control::settings::load_user().settings;
    if !super::gate_open(true, &settings) {
        return;
    }
    adopt_download();
    let facts = facts(&settings, offered().as_deref());
    let decision = state::with(|state| {
        if state.phase == Phase::Installing {
            return Err("installing");
        }
        state.on_quit(&facts).map_err(|wait| wait.as_str())
    });
    if let Err(reason) = decision {
        tracing::debug!(reason, "quitting installs no update");
        return;
    }
    match hand_over_ready(false) {
        Ok(version) => tracing::info!(%version, "the update installs now that rBrowser quits"),
        Err(error) => {
            tracing::error!(code = error.code, detail = %error.detail, "the update cannot be installed at quit");
        }
    }
}

/// A verified download of the offer that another window or an earlier run left in the folder is
/// this window's ready download too.
fn adopt_download() {
    let Some(data) = rb_config::local_dir() else {
        return;
    };
    let artifact = state::with(|state| match (&state.ready, &state.offer) {
        (None, Some(offer)) => offer.artifact.clone(),
        _ => None,
    });
    let Some(artifact) = artifact else {
        return;
    };
    if let Some(path) = rb_update::already_downloaded(&artifact, &handover::download_dir(&data)) {
        state::with(|state| {
            if state.ready.is_none() {
                state.ready = Some(path);
            }
        });
    }
}

//! The Web Store in the running app (RB-080-23): the person's install by id or store URL
//! (`extensions.install_web_store`), the check for updates (`extensions.check_updates`, and the
//! start when `checkUpdatesAtStart` is on) with its question in the window, and the MCP tool
//! `extension_install`, which installs only after the person's word in the window. Everything
//! downloads, verifies and unpacks on a blocking thread ([`super::install`]); what is installed
//! loads at the next start ([`super::start`]). The removal is [`super::remove`] (RB-080-30);
//! the one reading of a written reference, the commands' `ref` and the tools' `id`, is
//! [`ref_arg`].

use std::sync::Arc;

use rb_ipc::{CoreToShell, ExtensionPromptKind, Notification, NotificationLevel};
use serde_json::{Value, json};

use super::install::{Candidate, InstallFailure, Installer};
use super::prompt::{self, PromptRequest};
use super::store::{ExtensionStore, InstalledExtension};
use super::webstore::{HttpHops, chromium_version, parse_ref};
use crate::control::Core;

/// The settings key of the switch, which the notices link to.
pub const ALLOW_KEY: &str = "engines.chromium.extensions.allowWebStore";
/// The notice: installed from the Web Store.
pub const INSTALLED_KEY: &str = "extensions.notice.installed";
/// The notice: updated from the Web Store.
pub const UPDATED_KEY: &str = "extensions.notice.updated";
/// The notice: every installed extension is current.
pub const UP_TO_DATE_KEY: &str = "extensions.notice.up_to_date";
/// The notice: an install or update failed.
pub const FAILED_KEY: &str = "extensions.notice.web_store_failed";
/// The notice: the Web Store is switched off.
pub const STORE_OFF_KEY: &str = "extensions.notice.web_store_off";
/// The notice: nothing is installed from the Web Store.
pub const NOTHING_INSTALLED_KEY: &str = "extensions.notice.nothing_installed";
/// The MCP tool that installs.
pub const INSTALL_TOOL: &str = "extension_install";
/// The MCP tool that removes ([`super::remove`]).
pub const REMOVE_TOOL: &str = "extension_remove";
/// The error code while the Web Store is off.
pub const OFF_CODE: &str = "extensions.web_store_off";
/// The error code when the person declined.
pub const DECLINED_CODE: &str = "extensions.declined";

/// The publisher key an install must carry a proof of: the Web Store's.
const PUBLISHER: [u8; 32] = super::crx3::WEB_STORE_PUBLISHER_KEY_HASH;

/// A notification with the Web Store switch linked.
pub fn notice(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
        settings_key: Some(ALLOW_KEY.to_owned()),
    })
}

/// The store on disk; `None` without a local folder.
pub fn store() -> Option<ExtensionStore> {
    ExtensionStore::default_root().map(ExtensionStore::new)
}

/// The ids installed from the Web Store.
pub fn installed_ids() -> Vec<String> {
    store()
        .map(|store| store.installed().into_iter().map(|i| i.id).collect())
        .unwrap_or_default()
}

/// Why the Web Store cannot be used now.
pub(super) fn refused(core: &Core) -> Option<String> {
    let settings = core.user_settings().engines.chromium.extensions;
    if !settings.allow_web_store {
        return Some(rb_i18n::t(STORE_OFF_KEY));
    }
    None
}

/// Runs `work` with an installer over the default store and a client of its own.
fn with_installer<T>(
    core: &Core,
    work: impl FnOnce(&Installer<'_>) -> Result<T, InstallFailure>,
) -> Result<T, String> {
    with_installer_over(core, &HttpHops::new(), work)
}

/// Runs `work` with an installer over the default store and `hops` (one client, one TLS
/// setup, for a whole run).
fn with_installer_over<T>(
    core: &Core,
    hops: &HttpHops,
    work: impl FnOnce(&Installer<'_>) -> Result<T, InstallFailure>,
) -> Result<T, String> {
    if let Some(reason) = refused(core) {
        return Err(reason);
    }
    let store = store().ok_or_else(|| "no local folder for the extensions".to_owned())?;
    let installer = Installer {
        fetcher: hops,
        store: &store,
        chromium_version: chromium_version(Some(env!("RB_CEF_BUILD_VERSION"))),
        publisher_key_hash: PUBLISHER,
    };
    work(&installer).map_err(|error| error.to_string())
}

/// Installs the extension `written` names (an id or a store URL): the person's own command.
/// Blocking.
///
/// # Errors
///
/// Not a reference, the Web Store off, or any step of the install.
pub fn install_by_ref(core: &Core, written: &str) -> Result<InstalledExtension, String> {
    let id = parse_ref(written)
        .ok_or_else(|| "not an extension id or a Chrome Web Store URL".to_owned())?;
    with_installer(core, |installer| installer.install_by_id(&id))
}

/// Downloads and verifies `written`'s extension without unpacking it. Blocking.
fn fetch_by_ref(core: &Core, written: &str) -> Result<Candidate, String> {
    let id = parse_ref(written)
        .ok_or_else(|| "not an extension id or a Chrome Web Store URL".to_owned())?;
    with_installer(core, |installer| installer.fetch(&id))
}

/// Unpacks `candidate` into the store. Blocking.
fn install_candidate(core: &Core, candidate: &Candidate) -> Result<InstalledExtension, String> {
    with_installer(core, |installer| installer.install(candidate))
}

/// Asks the Web Store for a newer version of every installed extension (one client for the
/// run; the version check downloads nothing, [`super::update_check`]); each one found is a
/// question in the window and installed only on the person's word. `report` says whether a
/// notice is shown when nothing is newer (the command: yes; the start: no). Blocking.
pub fn check_updates(core: &Arc<Core>, report: bool) {
    let installed = store().map(|store| store.installed()).unwrap_or_default();
    if installed.is_empty() {
        if report {
            core.notify(notice(NotificationLevel::Info, NOTHING_INSTALLED_KEY, &[]));
        }
        return;
    }
    let mut newer = Vec::new();
    let hops = HttpHops::new();
    for extension in installed {
        match with_installer_over(core, &hops, |installer| installer.newer(&extension)) {
            Ok(Some(candidate)) => newer.push((extension, candidate)),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(id = %extension.id, "the update check failed: {error}");
                core.notify(notice(
                    NotificationLevel::Warning,
                    FAILED_KEY,
                    &[("id", extension.id.clone()), ("error", error)],
                ));
            }
        }
    }
    if newer.is_empty() {
        if report {
            core.notify(notice(NotificationLevel::Info, UP_TO_DATE_KEY, &[]));
        }
        return;
    }
    for (extension, candidate) in newer {
        let request = PromptRequest {
            kind: ExtensionPromptKind::Update,
            id: candidate.id.clone(),
            name: candidate.name.clone(),
            version: candidate.version.clone(),
            current_version: Some(extension.version.clone()),
        };
        let worker = core.clone();
        prompt::ask(
            core,
            request,
            Box::new(move |accepted| {
                if !accepted {
                    tracing::info!(id = %candidate.id, "the update was declined");
                    return;
                }
                let runtime = worker.runtime().clone();
                runtime.spawn_blocking(move || {
                    let message = match install_candidate(&worker, &candidate) {
                        Ok(installed) => {
                            super::remove::push_list(&worker);
                            notice(
                                NotificationLevel::Info,
                                UPDATED_KEY,
                                &[("name", installed.name), ("version", installed.version)],
                            )
                        }
                        Err(error) => notice(
                            NotificationLevel::Error,
                            FAILED_KEY,
                            &[("id", candidate.id.clone()), ("error", error)],
                        ),
                    };
                    worker.notify(message);
                });
            }),
        );
    }
}

/// The core is up: the check for updates when the person switched it on.
pub fn on_start(core: &Arc<Core>) {
    let settings = core.user_settings().engines.chromium.extensions;
    if !(settings.enabled && settings.allow_web_store && settings.check_updates_at_start) {
        return;
    }
    let worker = core.clone();
    core.runtime()
        .spawn_blocking(move || check_updates(&worker, false));
}

/// The string `field` of `args`, trimmed; `None` without one or when it is empty. The one
/// reading of a written reference: the commands' `ref` and the MCP tools' `id` (RB-080-30).
pub fn ref_arg(args: &Value, field: &str) -> Option<String> {
    args.as_object()
        .and_then(|map| map.get(field))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|written| !written.is_empty())
        .map(str::to_owned)
}

/// The argument of `tool` (`extension_install`, `extension_remove`): `id`, an extension id or
/// a store URL, as written.
///
/// # Errors
///
/// Anything but an object with a string `id` that names an extension.
pub fn tool_args(args: &Value, tool: &str) -> Result<String, rb_mcp::ToolFailure> {
    let written = ref_arg(args, "id").ok_or_else(|| {
        rb_mcp::ToolFailure::new(
            "invalid_arguments",
            format!("{tool} takes {{ id: <extension id or Chrome Web Store URL> }}"),
        )
    })?;
    if parse_ref(&written).is_none() {
        return Err(rb_mcp::ToolFailure::new(
            "invalid_arguments",
            format!(
                "{tool}: id is not an extension id (32 letters a to p) or a Chrome Web Store URL"
            ),
        ));
    }
    Ok(written)
}

/// The MCP tool `extension_install`: downloads and verifies the extension, asks the person in
/// the window, and unpacks it only on their word.
///
/// # Errors
///
/// Arguments, the Web Store off, the download or the check, the person's no, the unpacking.
pub async fn host_tool(core: &Arc<Core>, args: Value) -> Result<Value, rb_mcp::ToolFailure> {
    let written = tool_args(&args, INSTALL_TOOL)?;
    if refused(core).is_some() {
        return Err(rb_mcp::ToolFailure::new(
            OFF_CODE,
            "Web Store installs are switched off (engines.chromium.extensions.allowWebStore)",
        )
        .with_setting(ALLOW_KEY));
    }
    let fetcher = core.clone();
    let candidate = tokio::task::spawn_blocking(move || fetch_by_ref(&fetcher, &written))
        .await
        .map_err(|error| rb_mcp::ToolFailure::new("internal", error.to_string()))?
        .map_err(|error| rb_mcp::ToolFailure::new("extensions.install_failed", error))?;
    let (sent, heard) = tokio::sync::oneshot::channel();
    let request = PromptRequest {
        kind: ExtensionPromptKind::Install,
        id: candidate.id.clone(),
        name: candidate.name.clone(),
        version: candidate.version.clone(),
        current_version: store()
            .and_then(|store| store.installed_one(&candidate.id))
            .map(|installed| installed.version),
    };
    prompt::ask(
        core,
        request,
        Box::new(move |accepted| {
            let _ = sent.send(accepted);
        }),
    );
    if !heard.await.unwrap_or(false) {
        return Err(rb_mcp::ToolFailure::new(
            DECLINED_CODE,
            "the person did not confirm the install in the window",
        ));
    }
    let worker = core.clone();
    let installed = tokio::task::spawn_blocking(move || install_candidate(&worker, &candidate))
        .await
        .map_err(|error| rb_mcp::ToolFailure::new("internal", error.to_string()))?
        .map_err(|error| rb_mcp::ToolFailure::new("extensions.install_failed", error))?;
    super::remove::push_list(core);
    core.notify(notice(
        NotificationLevel::Info,
        INSTALLED_KEY,
        &[
            ("name", installed.name.clone()),
            ("version", installed.version.clone()),
        ],
    ));
    Ok(json!({
        "id": installed.id,
        "name": installed.name,
        "version": installed.version,
        "restartNeeded": true,
    }))
}

#[cfg(test)]
#[path = "web_tests.rs"]
mod tests;

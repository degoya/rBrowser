//! Removing an extension installed from the Chrome Web Store (RB-080-30): the person's command
//! `extensions.remove` (`ref`: an id or a store URL; the settings' list of the installed
//! extensions has a Remove button per row), and the MCP tool `extension_remove`, which removes
//! only after the person's word in the window ([`super::prompt`]). A removal deletes the
//! extension's folder under rBrowser's local store with every version, the pointer, the
//! retired folders and the staging leftovers ([`super::store`]); what this start loaded stays
//! loaded until the next start, which the notice says. Like every Web Store command it is
//! refused while the Web Store is off, and it is refused while another rBrowser window's
//! install of the same extension runs (the store is shared by every instance). Unpacked folders are never removed here: they are
//! referenced where they are and a project's belong to the project. Blocking: the callers run
//! the removal on a blocking thread.
//!
//! The settings' list: `listWebStoreExtensions` is answered with `ack`, and the list follows as
//! the event `webStoreExtensions`, read off the UI thread ([`push_list`]); the same event goes
//! out after every change of the store (an install, an update, a removal, by the person or by
//! MCP), so an open list is never stale.

use std::sync::Arc;

use rb_ipc::{CoreToShell, ExtensionPromptKind, NotificationLevel, ShellToCore, WebStoreExtension};
use serde_json::{Value, json};

use super::prompt::{self, PromptRequest};
use super::start::{self, StartPlan};
use super::store::{ExtensionStore, InstalledExtension};
use super::web::{self, notice, store};
use super::webstore::parse_ref;
use crate::control::Core;
use crate::ipc::Failure;

/// The notice: an extension was removed.
pub const REMOVED_KEY: &str = "extensions.notice.removed";
/// The notice: a removal failed.
pub const REMOVE_FAILED_KEY: &str = "extensions.notice.remove_failed";
/// The error code when nothing of the id is installed from the Web Store.
pub const NOT_INSTALLED_CODE: &str = "extensions.not_installed";
/// The error code when the removal failed.
pub const REMOVE_FAILED_CODE: &str = "extensions.remove_failed";

/// What a removal took.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removed {
    /// The extension id.
    pub id: String,
    /// The manifest's name; the id when no manifest named it.
    pub name: String,
    /// The version that was active; `None` when the pointer named none.
    pub version: Option<String>,
    /// Whether this start loaded it: it stays loaded until the next start.
    pub loaded: bool,
}

/// Removes the extension `written` names (an id or a store URL) from `store`; `started` says
/// whether this start loaded it. Blocking.
///
/// # Errors
///
/// Not a reference, nothing of the id in the store, an install of it under way, or a file
/// that cannot be deleted.
pub fn remove_in(
    store: &ExtensionStore,
    started: &StartPlan,
    written: &str,
) -> Result<Removed, String> {
    let id = parse_ref(written)
        .ok_or_else(|| "not an extension id or a Chrome Web Store URL".to_owned())?;
    let installed = store.installed_one(&id);
    let there = store.remove(&id).map_err(|error| error.to_string())?;
    if !there {
        return Err(format!("{id} is not installed from the Chrome Web Store"));
    }
    let loaded = started.web_store.iter().any(|loaded| loaded.id == id);
    let name = installed
        .as_ref()
        .map_or_else(|| id.clone(), |installed| installed.name.clone());
    Ok(Removed {
        id,
        name,
        version: installed.map(|installed| installed.version),
        loaded,
    })
}

/// [`remove_in`] over the default store and this start's plan: the person's own command.
/// Refused while the Web Store is off, like the install. Blocking.
///
/// # Errors
///
/// The Web Store off, no local folder, or [`remove_in`]'s.
pub fn remove_by_ref(core: &Core, written: &str) -> Result<Removed, String> {
    if let Some(reason) = web::refused(core) {
        return Err(reason);
    }
    let store = store().ok_or_else(|| "no local folder for the extensions".to_owned())?;
    remove_in(&store, &start::started(), written)
}

/// The notice after a removal.
pub fn removed_notice(removed: &Removed) -> CoreToShell {
    notice(
        NotificationLevel::Info,
        REMOVED_KEY,
        &[("name", removed.name.clone())],
    )
}

/// The notice after a failed removal of `written`.
pub fn failed_notice(written: &str, error: String) -> CoreToShell {
    notice(
        NotificationLevel::Error,
        REMOVE_FAILED_KEY,
        &[("id", written.to_owned()), ("error", error)],
    )
}

/// The settings' list: `installed` with whether `started` loaded each.
pub fn rows(installed: &[InstalledExtension], started: &StartPlan) -> Vec<WebStoreExtension> {
    installed
        .iter()
        .map(|extension| WebStoreExtension {
            id: extension.id.clone(),
            name: extension.name.clone(),
            version: extension.version.clone(),
            loaded: started
                .web_store
                .iter()
                .any(|loaded| loaded.id == extension.id),
        })
        .collect()
}

/// Sends the shell the list of the installed extensions (the default store, this start's
/// plan), read on a blocking thread: after `listWebStoreExtensions` and after every change of
/// the store.
pub fn push_list(core: &Arc<Core>) {
    let worker = core.clone();
    core.runtime().spawn_blocking(move || {
        let installed = store().map(|store| store.installed()).unwrap_or_default();
        worker.send(&CoreToShell::WebStoreExtensions {
            items: rows(&installed, &start::started()),
        });
    });
}

/// Answers `listWebStoreExtensions` with `ack`; the list follows as `webStoreExtensions`
/// ([`push_list`]). `None` for every other message.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    matches!(message, ShellToCore::ListWebStoreExtensions).then(|| {
        if let Some(core) = core.arc() {
            push_list(&core);
        }
        Ok(CoreToShell::Ack)
    })
}

/// The MCP tool `extension_remove`: names the installed extension in the window (the prompt
/// says every version goes) and removes it only on the person's word; the answer names the
/// version that was active at the removal, read again then. Off with the Web Store, like the
/// install.
///
/// # Errors
///
/// Arguments, the Web Store off, nothing of the id installed, the person's no, the removal.
pub async fn host_tool(core: &Arc<Core>, args: Value) -> Result<Value, rb_mcp::ToolFailure> {
    let written = web::tool_args(&args, web::REMOVE_TOOL)?;
    if web::refused(core).is_some() {
        return Err(rb_mcp::ToolFailure::new(
            web::OFF_CODE,
            "the Web Store is switched off (engines.chromium.extensions.allowWebStore)",
        )
        .with_setting(web::ALLOW_KEY));
    }
    let id = parse_ref(&written)
        .ok_or_else(|| rb_mcp::ToolFailure::new("args.invalid", "id names no extension"))?;
    let looked_up = id.clone();
    let installed = tokio::task::spawn_blocking(move || {
        store().and_then(|store| store.installed_one(&looked_up))
    })
    .await
    .map_err(|error| rb_mcp::ToolFailure::new("mcp.internal", error.to_string()))?
    .ok_or_else(|| {
        rb_mcp::ToolFailure::new(
            NOT_INSTALLED_CODE,
            format!("{id} is not installed from the Chrome Web Store"),
        )
    })?;
    let (sent, heard) = tokio::sync::oneshot::channel();
    let request = PromptRequest {
        kind: ExtensionPromptKind::Remove,
        id: installed.id.clone(),
        name: installed.name.clone(),
        version: installed.version.clone(),
        current_version: None,
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
            web::DECLINED_CODE,
            "the person did not confirm the removal in the window",
        ));
    }
    let worker = core.clone();
    let removed = tokio::task::spawn_blocking(move || remove_by_ref(&worker, &id))
        .await
        .map_err(|error| rb_mcp::ToolFailure::new("mcp.internal", error.to_string()))?
        .map_err(|error| rb_mcp::ToolFailure::new(REMOVE_FAILED_CODE, error))?;
    core.notify(removed_notice(&removed));
    push_list(core);
    Ok(json!({
        "id": removed.id,
        "name": removed.name,
        "version": removed.version,
        "restartNeeded": removed.loaded,
    }))
}

#[cfg(test)]
#[path = "remove_tests.rs"]
mod tests;

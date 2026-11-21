//! Deleting a flow (RB-090-91): `flows.delete { name }` (`Leader F D`) removes
//! `<automation.flows.dir>/<name>.ts` - a person's, from the Flows panel's "Delete" or a key;
//! MCP has no tool (an agent never deletes project files). Without a name the command opens the
//! Flows panel.
//!
//! The name is checked as `flows.run` checks it: a flow name, no path, no `..`, a file of the
//! flows folder. The flow that runs now is not deleted - its run reads the file -, and the panel
//! says why its "Delete" waits. `.rbrowser/flows/` is protected (K9): the click is the person's
//! decision for exactly that file, so a trusted project stays trusted when its protected parts
//! afterwards are the old ones without this file ([`only_this_removed`]) - under the trust store's
//! lock, as a flow saved from the Recorder panel keeps it ([`crate::recorder::flow_file`]); any
//! other difference is left to the trust prompt, which the check after the delete raises. The
//! file goes on a blocking
//! thread (a WSL project's disk is a network share); a notice says how it went, and
//! `flowRunChanged` has an open Flows panel list the flows again.

use std::sync::Arc;
use std::time::SystemTime;

use rb_commands::CommandError;
use rb_ipc::{CoreToShell, NotificationLevel};
use serde_json::{Value, json};

use super::app::notification;
use super::catalog::{relative, valid_name};
use super::exclusive::{exclusively, running_flow};
use super::queue::waiting;
use crate::control::Core;
use crate::recorder::flow_file::{
    FlowFileError, Target, keep_trust, only_this_removed, path_of, refuse_links, trusted_state,
};

/// The i18n keys of the notices; `{flow}` and `{error}` are placeholders.
pub mod notice {
    /// The flow was deleted.
    pub const DELETED: &str = "flows.notice.deleted";
    /// The flow was not deleted.
    pub const DELETE_FAILED: &str = "flows.notice.delete_failed";
}

/// Why a flow was not deleted.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DeleteError {
    /// Not a flow name.
    #[error("{0:?} is no flow name (letters, digits, - _ and . only)")]
    BadName(String),
    /// `automation.flows.dir` leaves `.rbrowser/flows`.
    #[error("automation.flows.dir {0:?} must be .rbrowser/flows or a folder inside it")]
    BadDir(String),
    /// No flow of that name.
    #[error("there is no flow {0:?}")]
    NotFound(String),
    /// The flow runs now or waits in a Run all; its run reads the file.
    #[error(
        "the flow {0:?} is running or waits to run: delete it once the run has ended, or cancel \
         it (flows.cancel)"
    )]
    Running(String),
    /// A folder on the way to the flow is a link out of the project's own folders.
    #[error("{0} is a link; a flow is only deleted in the project's own folders")]
    Linked(String),
    /// The file could not be deleted.
    #[error("the flow could not be deleted: {0}")]
    Io(String),
}

impl From<FlowFileError> for DeleteError {
    fn from(error: FlowFileError) -> Self {
        match error {
            FlowFileError::BadName(name) => Self::BadName(name),
            FlowFileError::BadDir(dir) => Self::BadDir(dir),
            FlowFileError::Linked(path) => Self::Linked(path),
            FlowFileError::Exists(reason) | FlowFileError::Io(reason) => Self::Io(reason),
        }
    }
}

/// A flow deleted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeletedFlow {
    /// Relative to the project root, with `/`.
    pub path: String,
    /// The project's trust covers what is left: it was trusted before and nothing else changed.
    pub trust_kept: bool,
}

/// The flows a delete leaves alone: the one running and those a Run all still waits to start.
fn busy_flows() -> Vec<String> {
    running_flow().into_iter().chain(waiting()).collect()
}

/// Deletes `target`'s flow unless it is `busy` (running or waiting to), and keeps the project's trust for exactly
/// that change when it had it.
///
/// # Errors
///
/// A bad name or folder, no such flow, the flow running, a linked folder, or the file cannot be
/// deleted.
pub fn delete_flow(target: &Target<'_>, busy: &[String]) -> Result<DeletedFlow, DeleteError> {
    let path = path_of(target)?;
    if busy.iter().any(|flow| flow == target.name) {
        return Err(DeleteError::Running(target.name.to_owned()));
    }
    let shown = relative(target.root, &path);
    // The folders on the way, not the file: deleting a link removes the link alone.
    if let Some(folder) = path.parent() {
        refuse_links(target.root, folder)?;
    }
    if !path.is_file() {
        return Err(DeleteError::NotFound(target.name.to_owned()));
    }
    let before = trusted_state(target);
    std::fs::remove_file(&path).map_err(|error| DeleteError::Io(error.to_string()))?;
    tracing::info!("flow deleted");
    let trust_kept = match (before, target.store) {
        (Some(before), Some(store)) => keep_trust(target, store, &before, |after| {
            only_this_removed(&before, after, &shown)
        }),
        _ => false,
    };
    Ok(DeletedFlow {
        path: shown,
        trust_kept,
    })
}

/// `flows.delete`: without `name` the Flows panel opens; with one the name is checked, then the
/// flow is deleted on a blocking thread of the core's runtime. Answers at once; the outcome is a
/// notice.
///
/// # Errors
///
/// A `name` that is no flow name, the flow running, or no project open.
pub fn command(core: &Arc<Core>, args: &Value) -> Result<Value, CommandError> {
    let name = match args.get("name") {
        None | Some(Value::Null) => {
            let flows = crate::panel_dock::core_panels::FLOWS;
            crate::panel_dock::core_panels::open_unfolded(core, flows);
            return Ok(Value::Null);
        }
        Some(Value::String(name)) => name.clone(),
        Some(_) => {
            return Err(CommandError::InvalidArgs(
                "`name` is the flow's name, a string".to_owned(),
            ));
        }
    };
    if !valid_name(&name) {
        return Err(CommandError::InvalidArgs(
            DeleteError::BadName(name).to_string(),
        ));
    }
    if busy_flows().contains(&name) {
        return Err(CommandError::Failed(DeleteError::Running(name).to_string()));
    }
    let root = core
        .host()
        .project_root()
        .ok_or_else(|| CommandError::Failed("no project is open".to_owned()))?;
    let dir = core.user_settings().automation.flows.dir;
    let store = crate::trust::store_path(core);
    let now = crate::trust::rfc3339(SystemTime::now());
    let core = Arc::clone(core);
    let answer = json!({ "name": name });
    core.runtime().clone().spawn_blocking(move || {
        let target = Target {
            root: &root,
            dir: &dir,
            name: &name,
            store: store.as_deref(),
            now: &now,
        };
        // No run of this flow starts between the check and the delete.
        let outcome = exclusively(|| delete_flow(&target, &busy_flows()));
        core.notify(outcome_notice(&name, &outcome));
        if outcome.is_ok() {
            // As after a saved flow: the trust check runs again and asks when the decision did
            // not move with the change.
            crate::trust::on_config_changed(&core);
        }
        core.send(&CoreToShell::FlowRunChanged);
    });
    Ok(answer)
}

/// The notice of a delete: done, or why not.
#[must_use]
pub fn outcome_notice(name: &str, outcome: &Result<DeletedFlow, DeleteError>) -> CoreToShell {
    let flow = ("flow", name.to_owned());
    match outcome {
        Ok(_) => notification(NotificationLevel::Info, notice::DELETED, &[flow]),
        Err(error) => {
            tracing::warn!(%error, "a flow was not deleted");
            notification(
                NotificationLevel::Error,
                notice::DELETE_FAILED,
                &[flow, ("error", rb_diag::redact(&error.to_string()))],
            )
        }
    }
}

#[cfg(test)]
#[path = "delete_tests.rs"]
mod tests;

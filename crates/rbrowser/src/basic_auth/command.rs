//! `environment.auth` (`Leader E A`): without arguments the shell's bar opens and asks for the
//! environment, user and password; with them the credential is stored (or removed with
//! `remove: true`) in the platform's credential store. MCP never runs it (`omitted(secrets)`,
//! refused by `crate::mcp::backend` as well). Its errors are fixed text: serde names the value
//! it could not take, and a password typed into the wrong field must reach no notice or log.

use std::collections::BTreeMap;

use rb_commands::CommandError;
use rb_engine::{Target, ViewHost};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde::Deserialize;
use serde_json::{Value, json};

use super::service::service;
use super::store::Credential;
use super::{AUTH, SETTINGS_KEY};
use crate::control::Core;

/// The notice after a credential was stored; placeholder `{environment}`.
pub const SAVED_KEY: &str = "auth.notice.saved";
/// The notice after a credential was stored for an environment not yet listed under
/// `basicAuth.environments`; placeholder `{environment}`.
pub const NOT_LISTED_KEY: &str = "auth.notice.notListed";
/// The notice after a credential was removed; placeholder `{environment}`.
pub const REMOVED_KEY: &str = "auth.notice.removed";
/// The longest environment id taken.
const MAX_ID: usize = 128;

/// What `environment.auth` was asked to do.
pub enum Action {
    /// Open the shell's bar.
    Open,
    /// Store `credential` for `environment`.
    Set {
        environment: String,
        credential: Credential,
    },
    /// Remove the credential of `environment`.
    Remove { environment: String },
}

/// The arguments as sent; no `Debug`, it holds the password.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    environment: String,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    remove: bool,
}

fn invalid(what: &str) -> CommandError {
    CommandError::InvalidArgs(format!(
        "{what}; expected {{ environment, user, password }} or {{ environment, remove: true }}"
    ))
}

/// The action `args` asks for.
///
/// # Errors
///
/// [`CommandError::InvalidArgs`] with fixed text, never a value of `args`.
pub fn parse(args: &Value) -> Result<Action, CommandError> {
    if args.is_null() || args.as_object().is_some_and(serde_json::Map::is_empty) {
        return Ok(Action::Open);
    }
    let args = Args::deserialize(args).map_err(|_| invalid("unreadable arguments"))?;
    let plain_id = !args.environment.is_empty()
        && args.environment.len() <= MAX_ID
        && args
            .environment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !plain_id {
        return Err(invalid("environment is no environment id"));
    }
    match (args.remove, args.user, args.password) {
        (true, None, None) => Ok(Action::Remove {
            environment: args.environment,
        }),
        (false, Some(user), Some(password)) => {
            let credential =
                Credential::new(&user, &password).map_err(|error| invalid(&error.to_string()))?;
            Ok(Action::Set {
                environment: args.environment,
                credential,
            })
        }
        _ => Err(invalid("user and password go together, remove alone")),
    }
}

/// Runs `environment.auth` with `args`.
///
/// # Errors
///
/// Invalid arguments, no project, an unknown environment, the credential store.
pub fn run(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let action = parse(args)?;
    if matches!(action, Action::Open) {
        core.ui().focus_shell();
        core.send(&CoreToShell::UiCommand {
            id: AUTH.to_owned(),
            args: None,
        });
        return Ok(Value::Null);
    }
    let host = core.host();
    let root = host
        .project_root()
        .ok_or_else(|| CommandError::Failed(super::service::AuthError::NoProject.to_string()))?;
    let project = crate::instance::file::key(&root);
    let config = host.config();
    let failed = |error: super::service::AuthError| CommandError::Failed(error.to_string());
    match action {
        Action::Set {
            environment,
            credential,
        } => {
            let origin = service()
                .set(&project, &config, &environment, &credential)
                .map_err(failed)?;
            let listed = config.basic_auth.uses(&environment);
            let key = if listed { SAVED_KEY } else { NOT_LISTED_KEY };
            let level = if listed {
                NotificationLevel::Info
            } else {
                NotificationLevel::Warning
            };
            core.notify(notice(level, key, &environment));
            // The views behind the challenge load now (Chromium asks again on the reload).
            if listed && let Err(error) = host.reload(Target::All, false) {
                tracing::debug!(%error, "the views were not reloaded after the credential");
            }
            Ok(json!({ "environment": environment, "origin": origin.to_string(), "stored": true }))
        }
        Action::Remove { environment } => {
            let removed = service()
                .remove(&project, &config, &environment)
                .map_err(failed)?;
            core.notify(notice(NotificationLevel::Info, REMOVED_KEY, &environment));
            Ok(json!({ "environment": environment, "removed": removed }))
        }
        Action::Open => Ok(Value::Null),
    }
}

fn notice(level: NotificationLevel, key: &str, environment: &str) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: BTreeMap::from([("environment".to_owned(), environment.to_owned())]),
        settings_key: Some(SETTINGS_KEY.to_owned()),
    })
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;

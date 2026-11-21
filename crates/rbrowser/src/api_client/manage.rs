//! The ops of the panel's collections and variables (RB-096-11): a collection made, renamed,
//! duplicated or deleted ([`super::folders`]), a request duplicated or moved, the Variables tab
//! read and saved, a private value set, the `$secret` names without a value
//! ([`super::variables`]). The panel's alone ([`super::protocol::permitted`]); a change to the
//! private values comes back as [`Outcome::secrets`], which the window writes to
//! `config.local.json` - the answer never holds a value.

use rb_plugin_types::{Failure, FailureKind};
use serde_json::json;

use super::error;
use super::ops::{Context, Outcome};
use super::protocol::{Op, SecretText};
use super::resolve::EnvReader;
use super::store::Store;
use super::variables::{self, Wanted};

/// Runs one of this module's ops.
///
/// # Errors
///
/// The op's own; `bad_args` for an op that is not one of them.
pub fn run(op: Op, ctx: &Context, store: &Store, env: &EnvReader) -> Result<Outcome, Failure> {
    Ok(match op {
        Op::CreateFolder { folder } => {
            Outcome::answer(json!({ "folder": store.create_folder(&folder)? }))
        }
        Op::RenameFolder { folder, to } => {
            Outcome::answer(json!({ "folder": store.rename_folder(&folder, &to)? }))
        }
        Op::DuplicateFolder { folder } => {
            let (folder, count) = store.duplicate_folder(&folder)?;
            Outcome::answer(json!({ "folder": folder, "count": count }))
        }
        Op::DeleteFolder { folder } => {
            let count = store.delete_folder(&folder)?;
            Outcome::answer(json!({ "deleted": folder.trim_matches('/'), "count": count }))
        }
        Op::Duplicate { id, name } => {
            Outcome::answer(json!({ "id": store.duplicate(&id, name.as_deref())? }))
        }
        Op::Move { id, folder } => {
            Outcome::answer(json!({ "id": store.move_request(&id, &folder)? }))
        }
        Op::Variables => Outcome::answer(variables::read(ctx, store)?),
        Op::SaveVariables {
            file,
            base,
            private,
            renamed,
            deleted,
        } => {
            let (secrets, file) = variables::save(
                ctx,
                store,
                Wanted {
                    file,
                    base,
                    private,
                    renamed,
                    deleted,
                },
            )?;
            // The window writes the file after the private values (`variables::commit`).
            let mut outcome = kept(json!({ "saved": true }), secrets);
            outcome.env_file = Some(file);
            outcome
        }
        Op::SetSecret {
            name,
            value: SecretText(value),
            environment,
        } => {
            let secrets = variables::set_secret(ctx, &name, &value, environment.as_deref())?;
            kept(json!({ "set": name.trim() }), secrets)
        }
        Op::Secrets { names, environment } => Outcome::answer(json!({
            "missing": variables::missing_secrets(ctx, store, &names, environment, env)?
        })),
        other => {
            return Err(error(FailureKind::BadData, "bad_args").with_param("reason", other.id()));
        }
    })
}

fn kept(answer: serde_json::Value, secrets: serde_json::Value) -> Outcome {
    let mut outcome = Outcome::answer(answer);
    outcome.secrets = Some(secrets);
    outcome
}

#[cfg(test)]
#[path = "manage_tests.rs"]
mod tests;

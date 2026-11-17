//! The app's side of the portal's offer (RB-090-71, [`super::offer`]): the status message with
//! its button when a portal page names a loopback origin the shown environment does not list
//! (unless `portal.offerAssetOrigins` is off), and `portal.add_asset_origin`, which adds it to
//! that one environment's `assetOrigins` in the file the list comes from - `config.local.json`
//! when it sets the list, else `config.json` - touching nothing else of the file, through the
//! settings' checks (`settings_window::edit_project_file`: validation, the trust gate, the
//! one-change lock), and then reads the config again, so the portal's listeners follow at once.
//! A person's click runs it: no MCP tool does ([`super::offer::refuse_mcp`]).

use std::sync::{Arc, Mutex, Weak};

use rb_commands::CommandError;
use rb_config::ConfigFile;
use rb_ipc::{CoreToShell, Notification, NotificationLevel, StatusAction};
use serde_json::{Value, json};

use super::offer::{
    ADD_ASSET_ORIGIN, AddError, ENVIRONMENTS_KEY, Offer, OfferBook, Sink, environment_for,
    origin_to_add, target_file, with_file_origin,
};
use crate::cdp::lock;
use crate::control::Core;
use crate::settings_window::edit_project_file;
use crate::settings_window::write::{reason, refused};

/// The i18n key of the offer.
pub const OFFER_KEY: &str = "portal.offer.unmapped";
/// The i18n key of the offer's button.
pub const ADD_LABEL_KEY: &str = "portal.offer.add";
/// The i18n key of the notice that the origin was added.
pub const ADDED_KEY: &str = "portal.offer.added";

/// What was offered, and the offers that wait for a click, oldest first.
static BOOK: Mutex<OfferBook> = Mutex::new(OfferBook::new());

/// The running portal's sink: each offer reaches `core`'s status bar.
pub fn sink(core: Weak<Core>) -> Sink {
    Arc::new(move |page_origin: &str, origin: &str| {
        if let Some(core) = core.upgrade() {
            offered(&core, page_origin, origin);
        }
    })
}

/// Forgets what was offered and what waits: the portal started.
pub fn reset() {
    lock(&BOOK).clear();
}

/// A page of `page_origin` named the loopback `origin`, which the portal does not forward: the
/// status bar offers to add it to the environment shown (the first of the page's origin), once
/// while that environment is shown. A page of no environment has none to add it to, and with
/// `portal.offerAssetOrigins` off nothing is offered; the log line stays the evidence.
pub fn offered(core: &Core, page_origin: &str, origin: &str) {
    if !core.user_settings().portal.offer_asset_origins || !lock(&BOOK).note(page_origin, origin) {
        return;
    }
    let config = core.host().config();
    let Some(environment) = environment_for(&config.environments, page_origin) else {
        tracing::debug!("a portal page of no environment named an unmapped loopback origin");
        return;
    };
    let offer = Offer {
        environment: environment.id.clone(),
        origin: origin.to_owned(),
    };
    lock(&BOOK).wait(offer.clone());
    let notification = Notification {
        level: NotificationLevel::Warning,
        message_key: OFFER_KEY.to_owned(),
        args: names(&offer),
        settings_key: Some(ENVIRONMENTS_KEY.to_owned()),
    };
    let action = StatusAction {
        command: ADD_ASSET_ORIGIN.to_owned(),
        label_key: ADD_LABEL_KEY.to_owned(),
        label_args: [("origin".to_owned(), offer.origin.clone())].into(),
        args: offer.args(),
    };
    core.notify_with_action(notification, action);
}

/// `portal.add_asset_origin`: adds the origin `args` name (`{ "origin", "environment" }`; none:
/// the oldest offer that waits) to the environment's `assetOrigins`, unless it lists exactly
/// that origin already; then the config is read again and the portal follows.
///
/// # Errors
///
/// No offer, wrong arguments, no open project, a config that cannot be read, or the write was
/// refused.
pub fn run(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let oldest = lock(&BOOK).oldest();
    let offer = Offer::from_args(args, oldest)?;
    let root = core
        .host()
        .project_root()
        .ok_or_else(|| CommandError::Failed("no project is open".to_owned()))?;
    let config =
        crate::project::load(&root).map_err(|error| CommandError::Failed(error.to_string()))?;
    let to_add = match origin_to_add(&config.environments, &offer.environment, &offer.origin) {
        Ok(to_add) => to_add,
        Err(error @ AddError::NotLoopback) => {
            return Err(CommandError::InvalidArgs(error.to_string()));
        }
        Err(error @ AddError::UnknownEnvironment) => {
            return Err(CommandError::Failed(error.to_string()));
        }
    };
    if let Some(origin) = &to_add {
        write(core, &offer.environment, origin)?;
    }
    lock(&BOOK).taken(&offer);
    if to_add.is_some() {
        // The watcher would follow too; the portal's listeners should not wait for it.
        if let Err(error) = crate::commands::reload_config(core.host()) {
            tracing::warn!(%error, "the project config was not read again after the write");
        }
        core.notify(CoreToShell::Notification(Notification {
            level: NotificationLevel::Success,
            message_key: ADDED_KEY.to_owned(),
            args: names(&offer),
            settings_key: Some(ENVIRONMENTS_KEY.to_owned()),
        }));
    }
    if let Some(server) = super::app::current() {
        super::app::refresh_page(core.host(), &server);
    }
    Ok(json!({
        "added": to_add.is_some(),
        "environment": offer.environment,
        "origin": offer.origin,
    }))
}

/// The placeholders of the offer's texts.
fn names(offer: &Offer) -> std::collections::BTreeMap<String, String> {
    [
        ("origin".to_owned(), offer.origin.clone()),
        ("environment".to_owned(), offer.environment.clone()),
    ]
    .into()
}

/// Appends `origin` to the `assetOrigins` of the environment `id` in the file its list comes
/// from, a person's edit of `environments`.
fn write(core: &Core, id: &str, origin: &str) -> Result<(), CommandError> {
    let answer = edit_project_file(core, ENVIRONMENTS_KEY, |project, local| {
        let invalid = || refused(reason::INVALID);
        let file = target_file(project, local, id).ok_or_else(invalid)?;
        let document = match file {
            ConfigFile::Project => Some(project),
            ConfigFile::Local => local,
        }
        .ok_or_else(invalid)?;
        let changed = with_file_origin(document, id, origin).ok_or_else(invalid)?;
        Ok((file, changed))
    });
    match answer {
        CoreToShell::SettingRejected { issues, .. } => {
            Err(CommandError::Failed(issues.first().map_or_else(
                || reason::INVALID.to_owned(),
                |issue| issue.message_key.clone(),
            )))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "add_origin_tests.rs"]
mod tests;

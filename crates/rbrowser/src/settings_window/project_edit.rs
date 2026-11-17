//! A change of one project file computed from what both files hold now (RB-090-71, the portal's
//! `portal.add_asset_origin`): the settings window writes a whole key, which for a list such as
//! `environments` would copy the merged list into one file; an edit that touches one field of
//! one entry goes through here instead - under the same one-change lock, through the same checks
//! (`write::validate_project`: migration, the trust gate, `rb_config::check`), never over a file
//! that exists but cannot be read, and every page hears the key changed.

use rb_config::ConfigFile;
use rb_ipc::{CoreToShell, SettingIssue};
use serde_json::{Value, json};

use super::user_file::writable;
use super::write::{reason, refused, validate_project};
use crate::cdp::lock;
use crate::control::Core;

/// What an edit makes: the file to write and its new JSON.
pub type Edit = (ConfigFile, Value);

/// Changes one project file as a person's edit of `key`: `edit` gets `config.json` and
/// `config.local.json` (when it exists) as they are on disk and returns the file to write and
/// its whole new content. Answers `ack`, or `settingRejected` with the reasons; nothing is
/// written then.
pub fn edit_project_file(
    core: &Core,
    key: &str,
    edit: impl FnOnce(&Value, Option<&Value>) -> Result<Edit, Vec<SettingIssue>>,
) -> CoreToShell {
    let rejected = |issues: Vec<SettingIssue>| CoreToShell::SettingRejected {
        key: key.to_owned(),
        issues,
    };
    let _one_at_a_time = lock(&super::apply::ONE_CHANGE);
    let Some(root) = core.host().project_root() else {
        return rejected(refused(reason::NO_PROJECT));
    };
    let project_path = ConfigFile::Project.path(&root);
    let local_path = ConfigFile::Local.path(&root);
    if !writable(&project_path) || !writable(&local_path) {
        tracing::warn!("a config file that cannot be read is not written over");
        return rejected(refused(reason::NOT_SAVED));
    }
    let project = rb_config::read_json(&project_path).unwrap_or_else(|_| json!({}));
    let local = if local_path.is_file() {
        rb_config::read_json(&local_path).ok()
    } else {
        None
    };
    let (file, document) = match edit(&project, local.as_ref()) {
        Ok(edit) => edit,
        Err(issues) => return rejected(issues),
    };
    let checked = match file {
        ConfigFile::Project => validate_project(&document, local.as_ref(), key),
        ConfigFile::Local => validate_project(&project, Some(&document), key),
    };
    if let Err(issues) = checked {
        return rejected(issues);
    }
    if let Err(error) = rb_config::write_config_file(&root, file, &document) {
        tracing::warn!(%error, "a project file could not be written");
        return rejected(refused(reason::NOT_SAVED));
    }
    core.send(&CoreToShell::SettingsChanged {
        keys: vec![key.to_owned()],
        values: None,
    });
    CoreToShell::Ack
}

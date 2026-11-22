//! Puts the project config on disk in effect (RB-094-23): what the config watcher does after a
//! change, and what the app's own writes of the project files do at once - the settings window,
//! MCP's `config_set`. The watcher alone is not enough: a project on a WSL share
//! (`\\wsl.localhost`) sends no change notifications, so an environment created in the settings
//! reached the main menu, the palette, the toolbar menus and MCP only with the next start (the
//! bookmarks had the same gap, RB-080-16).
//!
//! The files are applied once: the watcher's event for a write the app already applied finds
//! the same bytes and builds nothing again. Every reload of the config goes through here
//! ([`reload`], `commands::reload_config`), so the bytes remembered are the ones in effect.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rb_config::ConfigFile;

use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;

/// What [`apply_from_disk`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reloaded {
    /// The files changed since they were last applied: the config was read and is in effect.
    Applied,
    /// The files are the ones applied last (or no project is open): nothing was done.
    Unchanged,
}

/// The bytes of both config files as last applied, by project root.
static APPLIED: Mutex<Applied> = Mutex::new(None);

type Applied = Option<HashMap<PathBuf, Fingerprint>>;

/// The contents of `config.json` and `config.local.json` (`None` for a file that is not there
/// or cannot be read).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Fingerprint {
    root: PathBuf,
    shared: Option<Vec<u8>>,
    local: Option<Vec<u8>>,
}

fn fingerprint(root: &Path) -> Fingerprint {
    Fingerprint {
        root: root.to_path_buf(),
        shared: std::fs::read(ConfigFile::Project.path(root)).ok(),
        local: std::fs::read(ConfigFile::Local.path(root)).ok(),
    }
}

/// Reads the open project's config again and applies it as the watcher does - the grid, then
/// every feature (`features::on_config_changed`) and the hot reload - unless the files are the
/// ones applied last. One read and grid build at a time; an error leaves the config in effect as it was and
/// is not remembered, so the next call tries again.
pub fn apply_from_disk(core: &Core) -> Result<Reloaded, String> {
    let host = core.host();
    let Some(root) = host.project_root() else {
        return Ok(Reloaded::Unchanged);
    };
    let mut applied = lock(&APPLIED);
    // Before the read: a write between the two is seen by the next call.
    let now = fingerprint(&root);
    if applied
        .as_ref()
        .and_then(|known| known.get(&root))
        .is_some_and(|last| *last == now)
    {
        return Ok(Reloaded::Unchanged);
    }
    load(host, root, now, &mut applied)?;
    // The features read the config in effect; none of them waits for a reload.
    drop(applied);
    crate::features::on_config_changed(core);
    crate::reload::on_config_changed(core);
    Ok(Reloaded::Applied)
}

/// Reads the open project's config again and puts it in effect in `host` - views whose cell
/// keeps its breakpoint stay - and remembers the files as applied (`commands::reload_config`:
/// `app.reload_config`, a saved preset, a view edit, the engine, the portal, the first run).
/// Without a project nothing is done.
///
/// # Errors
///
/// Why the config did not load or apply.
pub fn reload(host: &Host) -> Result<(), String> {
    let Some(root) = host.project_root() else {
        return Ok(());
    };
    let mut applied = lock(&APPLIED);
    // Before the read: a write between the two is seen by the next call.
    let now = fingerprint(&root);
    load(host, root, now, &mut applied)
}

/// Loads the config of `root` into `host` and remembers `now` as its files applied.
fn load(host: &Host, root: PathBuf, now: Fingerprint, applied: &mut Applied) -> Result<(), String> {
    let config = crate::project::load(&root).map_err(|error| error.to_string())?;
    host.open_config(config, Some(root.clone()))
        .map_err(|error| error.to_string())?;
    applied.get_or_insert_with(HashMap::new).insert(root, now);
    Ok(())
}

/// [`apply_from_disk`] after the app wrote a project file itself; a failure is logged - the
/// write stands, and the watcher (where it sees changes) tries again.
pub fn after_write(core: &Core) {
    if let Err(error) = apply_from_disk(core) {
        tracing::warn!(%error, "the written config was not applied");
    }
}

#[cfg(test)]
#[path = "project_reload_tests.rs"]
mod tests;

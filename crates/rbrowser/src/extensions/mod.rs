//! Chrome extensions in the Chromium views (RB-080-06): unpacked Manifest V3 folders the person
//! names in `engines.chromium.extensions.unpacked`, loaded into every Chromium view of the
//! instance at its start, and a trusted project's recommendations (`extensions` in its config)
//! offered but never installed by themselves.
//!
//! - [`manifest`]: Manifest V3, name, version, the id from `key`; `debugger` is refused.
//! - [`folder`]: a folder the person names, or one inside a project (no `..`, no link out).
//! - [`start`]: the start's folders, handed to Chromium as `--load-extension`
//!   (`cef_app::run`); changes wait for the next start.
//! - [`recommend`]: a trusted project's offers.
//! - [`app`] and [`defs`]: the notices, the person's commands and `extension_list`.
//! - The Chrome Web Store (RB-080-23): [`webstore`] (an id or store URL, the download from
//!   Google's update server, https to its hosts only), [`crx3`] (the container's proofs: the
//!   developer's key is the id, the Web Store's key the publisher), [`store`] (the unpacked
//!   versions under `%LOCALAPPDATA%\rBrowser\extensions`), [`install`] (the three in one),
//!   [`update_check`] (the store's version without a download), [`prompt`] (the question in the
//!   window before an MCP install, an update or an MCP removal) and [`web`] (the commands, the
//!   update check, the MCP tool `extension_install`); [`remove`] (RB-080-30) takes an installed
//!   one out again: the person's command `extensions.remove`, the settings' list and the MCP
//!   tool `extension_remove`, which asks in the window first.
//!
//! The CDP proxy hides every `chrome-extension:` target (`crate::devtools::hidden`).

pub mod app;
pub mod crx3;
pub mod defs;
pub mod folder;
pub mod install;
pub mod manifest;
pub mod prompt;
pub mod recommend;
pub mod remove;
pub mod start;
pub mod store;
pub mod update_check;
pub mod web;
pub mod webstore;

pub use defs::register;
use rb_ipc::{CoreToShell, ShellToCore};

use crate::control::Core;
use crate::ipc::Failure;

/// The core is up: the start's refused folders, a trusted project's offer, and the Web Store's
/// check for updates when it is switched on.
pub fn on_start(core: &Core) {
    app::on_start(core);
    if let Some(core) = core.arc() {
        web::on_start(&core);
    }
}

/// Answers `message` when it is this module's (`extensionDecision`, `listWebStoreExtensions`);
/// `None` otherwise.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    prompt::ipc(message).or_else(|| remove::ipc(core, message))
}

/// The project config or its trust changed: the offer again, when it changed.
pub fn on_config_changed(core: &Core) {
    app::on_config_changed(core);
}

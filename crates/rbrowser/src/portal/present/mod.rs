//! The portal as a presentation over the internet (RB-096-05): a customer opens a link in their
//! own browser and sees the page rBrowser shows - navigation, scrolling and clicks follow the
//! presenter; the customer drives nothing. rBrowser opens a Cloudflare Quick Tunnel (no account,
//! a random `https://<name>.trycloudflare.com`) to a listener of the portal on 127.0.0.1.
//!
//! | Part | Where |
//! | --- | --- |
//! | The viewer link: one-time, short-lived, failures per link and across links (pure) | [`link`] |
//! | The paths a viewer may load (pure) | [`paths`] |
//! | The headers that stay at the portal (pure) | [`headers`] |
//! | A viewer's WebSocket frames: control frames only | [`frames`] |
//! | cloudflared: arguments, address, the process | [`tunnel`] |
//! | Which cloudflared, its download | [`cloudflared`] |
//! | The listener's side: host, rules, pointer | [`public`] |
//! | The requests from the tunnel | [`serve`] |
//! | Link and viewers on the listener | [`viewers`] |
//! | Start, stop, a new link, the tunnel followed | [`app`] |
//! | The panel's and MCP's view; the status bar's words | [`panel`], [`notice`] |
//! | The MCP tools `portal_present_*` | [`mcp`] |
//!
//! **Security.** Every request reaches the listener from 127.0.0.1 through cloudflared, so
//! nothing is decided by address. Off by default (`portal.presentation.enabled`, protected from
//! MCP); the first start waits for the person's consent that the traffic runs through Cloudflare
//! (`portal.presentation.consented`), a missing cloudflared for the person's word to download it
//! (pinned SHA-256, Apache-2.0); only a person's `portal.present.start` starts it - no MCP tool,
//! the command's coverage is `omitted`; MCP may stop it and read its state, never the link. The
//! link's token (128 bits) is in its fragment, which no server log sees, works once and for
//! `portal.presentation.linkTtlSec`; five failures a minute spend the link, twenty across links
//! lock every attempt for the minute. A viewer gets an `HttpOnly`, `Secure`, `SameSite=Strict`
//! cookie; its socket takes reports only - nothing a viewer does reaches the grid. Only the paths
//! of `portal.presentation.allowPaths` without `denyPaths` are forwarded; any other answers 404
//! without reaching the dev server. No cookie, no Cloudflare header and no server detail passes,
//! and no header override or mock of the project applies (the portal never adds them). Token and
//! link are in the log mask; neither reaches a log, an MCP answer or the status bar. The stop
//! disconnects every viewer at once and ends cloudflared; so does the LAN portal's stop, a change
//! of the presentation's settings, a crash of cloudflared and rBrowser's end. Asset origins
//! (`environments[].assetOrigins`) are not served through the tunnel (the job's spike decides
//! between more tunnels and one listener with path routing).

pub mod app;
pub mod cloudflared;
pub mod frames;
pub mod headers;
pub mod link;
pub mod mcp;
pub mod notice;
pub mod panel;
pub mod paths;
pub mod public;
pub mod serve;
pub mod tunnel;
pub mod viewers;

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use serde_json::{Value, json};

use crate::control::Core;

/// `Leader S W`: start the presentation (`{ "consent"?: bool, "download"?: bool }`).
pub const START: &str = "portal.present.start";
/// `Leader S K`: a new viewer link (the one before is void).
pub const NEW_LINK: &str = "portal.present.new_link";
/// `Leader S E`: end the presentation.
pub const STOP: &str = "portal.present.stop";
/// Why `portal.present.start` has no MCP tool.
pub const START_OMITTED: &str = "human-only: shares the site on the internet";
/// Why `portal.present.new_link` has no MCP tool.
pub const NEW_LINK_OMITTED: &str =
    "protected: a viewer link is a person's to hand out; it never reaches MCP";

/// The command definitions.
///
/// # Errors
///
/// An id is malformed.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    [
        (START, 'W', McpCoverage::Omitted(START_OMITTED.to_owned())),
        (
            NEW_LINK,
            'K',
            McpCoverage::Omitted(NEW_LINK_OMITTED.to_owned()),
        ),
        (STOP, 'E', McpCoverage::Covered(mcp::STOP_TOOL.to_owned())),
    ]
    .into_iter()
    .map(|(id, key, mcp)| {
        Ok(CommandDef {
            id: CommandId::new(id)?,
            title_key: format!("commands.{id}"),
            group: Group::Sync,
            default_bindings: vec![Binding::Leader(Chord {
                keys: vec![Key::Char(Group::Sync.leader_key()), Key::Char(key)],
            })],
            mcp,
        })
    })
    .collect()
}

/// Registers the commands with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| {
            let handler = handler(def.id.as_str(), Arc::downgrade(core));
            registry.register(def, handler).err()
        })
        .collect()
}

fn handler(id: &str, core: Weak<Core>) -> Handler {
    let id = id.to_owned();
    Arc::new(move |args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        run(&core, &id, &args)
    })
}

/// Runs the command `id`.
///
/// # Errors
///
/// The presentation is off or not live, or the arguments are wrong.
pub fn run(core: &Arc<Core>, id: &str, args: &Value) -> Result<Value, CommandError> {
    match id {
        START => app::start(core, args),
        NEW_LINK => app::new_link(core),
        STOP => Ok(json!({ "stopped": app::stop(core) })),
        other => Err(CommandError::UnknownCommand(other.to_owned())),
    }
}

/// The Portal panel, where the consent, the link and its QR code are.
pub(super) fn open_panel(core: &Core) {
    core.ui()
        .open_settings_window(crate::shell::page_url(super::ROUTE));
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "log_tests.rs"]
mod log_tests;

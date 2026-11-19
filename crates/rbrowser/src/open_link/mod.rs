//! `app.open_link { url?, where? }` (RB-080-92): a link from rBrowser's own surfaces - a
//! finding's help page, a plugin panel's report or help link - opens in the system's default
//! browser or in the grid's active view, as `where` or the setting `ui.openLinksIn` says
//! (`ask` is the shell's: it asks, then sends `where`). Without `url` it opens the handbook
//! (`Leader , H`).
//!
//! Only a web address opens ([`address::checked`]: `http:`/`https:`, no credentials, the parsed
//! form), at most [`throttle::MAX_OPENS`] per [`throttle::WINDOW`]; the system browser through
//! `ShellExecuteW` without a command line ([`system`]). A refusal is a sentence in the status
//! bar and the answer `{ "opened": false, "reason": <key> }`; the log names the host only - an
//! address may carry a token. MCP has no tool for it: an agent opens no browser of the
//! person's (`navigate` loads a page in the views).

pub mod address;
pub mod system;
pub mod throttle;

use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_config::OpenLinksIn;
use rb_engine::{Target, ViewHost};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::cdp::lock;
use crate::control::Core;

/// The command.
pub const OPEN_LINK_COMMAND: &str = "app.open_link";
/// What opens without `url`: the handbook, the public wiki (RB-090-12).
pub const HANDBOOK_URL: &str = "https://github.com/degoya/rBrowser/wiki";
/// Why MCP has no tool for it.
pub const OPEN_LINK_OMITTED: &str =
    "protected: an agent opens no browser of the person's; navigate loads a page in the views";
/// The key after `Leader ,`: H for help.
const OPEN_KEY: char = 'H';
/// The notice after a link went to the browser; `{host}`.
pub const OPENED_KEY: &str = "links.notice.opened";
/// The notice after a link loaded in the active view; `{host}`.
pub const OPENED_VIEW_KEY: &str = "links.notice.opened_view";
/// The refusal of a link over the throttle.
pub const BUSY_KEY: &str = "links.error.busy";
/// The refusal without an active view.
pub const NO_VIEW_KEY: &str = "links.error.no_view";
/// The browser did not take the link.
pub const FAILED_KEY: &str = "links.error.failed";

static THROTTLE: Mutex<throttle::Throttle> = Mutex::new(throttle::Throttle::new());

/// Where a link opens.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Where {
    /// The system's default browser.
    System,
    /// The grid's active view.
    ActiveView,
}

impl Where {
    fn name(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::ActiveView => "activeView",
        }
    }
}

/// The command's arguments.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Args {
    url: Option<String>,
    #[serde(rename = "where")]
    target: Option<Where>,
}

/// Where a link opens: as asked, else as the setting says; `ask` is decided in the shell, so a
/// link that comes without an answer goes to the browser.
pub fn target_of(asked: Option<Where>, setting: OpenLinksIn) -> Where {
    asked.unwrap_or(match setting {
        OpenLinksIn::ActiveView => Where::ActiveView,
        OpenLinksIn::System | OpenLinksIn::Ask => Where::System,
    })
}

/// Runs `app.open_link` with `args`.
///
/// # Errors
///
/// Arguments of the wrong shape, or a view that refused the navigation.
pub fn open(core: &Core, args: Value) -> Result<Value, CommandError> {
    let args: Args = if args.is_null() {
        Args::default()
    } else {
        serde_json::from_value(args)
            .map_err(|error| CommandError::InvalidArgs(error.to_string()))?
    };
    let target = target_of(args.target, core.user_settings().ui.open_links_in);
    let text = args.url.as_deref().unwrap_or(HANDBOOK_URL);
    let checked = match address::checked(text) {
        Ok(checked) => checked,
        Err(refusal) => return Ok(refused(core, refusal.key())),
    };
    let host = checked.host_str().unwrap_or_default().to_owned();
    // Only a link that opens counts against the budget; a refused one uses none of it.
    let view = match target {
        Where::System => None,
        Where::ActiveView => match core.host().active_view() {
            Some(view) => Some(view),
            None => return Ok(refused(core, NO_VIEW_KEY)),
        },
    };
    // The slot is taken in one step with the check, so two links at once cannot both pass.
    let at = Instant::now();
    if !lock(&THROTTLE).allow(at) {
        return Ok(refused(core, BUSY_KEY));
    }
    match view {
        None => to_browser(core, checked, host),
        Some(view) => {
            if let Err(error) =
                crate::nav_sync::navigate(core.host(), Target::View(view), checked.as_str())
            {
                lock(&THROTTLE).release(at);
                return Err(CommandError::Failed(error.to_string()));
            }
            notice(
                core,
                NotificationLevel::Success,
                OPENED_VIEW_KEY,
                Some(&host),
            );
        }
    }
    Ok(json!({ "opened": true, "where": target.name() }))
}

/// Takes one open of the budget the system browser has now (at most [`throttle::MAX_OPENS`] per
/// [`throttle::WINDOW`]) for another opener - the sitemap gallery (RB-090-83); `false` over it.
pub fn take_slot() -> bool {
    lock(&THROTTLE).allow(Instant::now())
}

/// Hands `url` to the browser on a thread of its own (the shell call may take a moment); the
/// status bar says how it went.
fn to_browser(core: &Core, url: url::Url, host: String) {
    let core = core.arc().map(|core| Arc::downgrade(&core));
    let spawned = std::thread::Builder::new()
        .name("rb-open-link".to_owned())
        .spawn(move || {
            let outcome = system::open(&url);
            let Some(core) = core.as_ref().and_then(Weak::upgrade) else {
                return;
            };
            match outcome {
                Ok(()) => notice(&core, NotificationLevel::Success, OPENED_KEY, Some(&host)),
                Err(error) => {
                    tracing::warn!(%error, host, "a link did not open in the browser");
                    notice(&core, NotificationLevel::Error, FAILED_KEY, None);
                }
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "the thread that opens a link did not start");
    }
}

/// The refusal `key` in the status bar and as the answer.
fn refused(core: &Core, key: &str) -> Value {
    tracing::info!(reason = key, "a link was not opened");
    notice(core, NotificationLevel::Error, key, None);
    json!({ "opened": false, "reason": key })
}

fn notice(core: &Core, level: NotificationLevel, key: &str, host: Option<&str>) {
    core.notify(CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: host
            .map(|host| [("host".to_owned(), host.to_owned())].into())
            .unwrap_or_default(),
        settings_key: None,
    }));
}

/// The definition; title key `commands.app.open_link`.
///
/// # Errors
///
/// An id the registry would refuse (a test holds it valid).
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    Ok(vec![CommandDef {
        id: CommandId::new(OPEN_LINK_COMMAND)?,
        title_key: format!("commands.{OPEN_LINK_COMMAND}"),
        group: Group::Settings,
        default_bindings: vec![Binding::Leader(Chord {
            keys: vec![Key::Char(Group::Settings.leader_key()), Key::Char(OPEN_KEY)],
        })],
        mcp: McpCoverage::Omitted(OPEN_LINK_OMITTED.to_owned()),
    }])
}

/// Registers the command with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| registry.register(def, handler(Arc::downgrade(core))).err())
        .collect()
}

fn handler(core: Weak<Core>) -> Handler {
    Arc::new(move |args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        open(&core, args)
    })
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

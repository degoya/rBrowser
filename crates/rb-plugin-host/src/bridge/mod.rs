//! The plugin runtime's protocol (RB-060-12, `docs/plugins.md`, "Page scripts and panels"): how a
//! plugin's page scripts talk to its component, and how the component's answers reach its page
//! scripts and panels. Pure: the application does the CDP and the shell work.
//!
//! - **Page scripts** run in an isolated world of their own, `rbplugin-<id>` ([`world_name`]),
//!   with the global `rbPlugin` the prologue ([`PageBridge::source`]) defines. They reach the
//!   host through one CDP binding whose name is random per start; a message is JSON of at most
//!   [`MAX_MESSAGE_BYTES`], sent as `s<id>:<json>` (a request the component answers),
//!   `e:<json>` (an event for the plugin's panels) or `l:<json>` (a line for rBrowser's log and
//!   console, [`PageLog`]; the world's `console` goes there too) - [`parse_page_payload`].
//! - **The component** gets a request through its `feature` export: `call-tool` with the tool
//!   [`PAGE_MESSAGE_TOOL`] and the arguments `{"view": <id>, "message": <json>}` - no WIT
//!   change, the contract stays `rbrowser:plugin@0.1.0`.
//! - **Every answer** of `run-command` and `call-tool` may carry posts: an object with the key
//!   [`POST_KEY`], a list of `{"target": "all" | <view id> | "panel" | "status" | "host", "message":
//!   <json>}`; the host takes them out ([`split_posts`]) and delivers them. The rest is the
//!   answer. A `status` post is a notice for the status bar (RB-080-64): the application shows
//!   the plugin's own text of `{"key", "params", "level"}`. A `host` post asks for an action the
//!   application offers that plugin (`{"action": ...}`, RB-080-74).

use rand::Rng as _;
use serde_json::{Value, json};

use crate::manifest::PluginManifest;

mod log;
mod source;

pub use log::{LogLevel, MAX_LOG_CHARS, MAX_LOG_VALUES, MAX_STACK_CHARS, PageLog};

/// The tool name a page script's request reaches `feature.call-tool` with. Not a valid tool
/// name of a manifest (`[a-z][a-z0-9_]*`), so it never collides with a declared one.
pub const PAGE_MESSAGE_TOOL: &str = "@page";
/// The tool name the host asks a plugin granted `network-rules` for its interception rules
/// with (RB-070-02); like [`PAGE_MESSAGE_TOOL`] no valid manifest tool name.
pub const NETWORK_RULES_TOOL: &str = "@rules";
/// The largest message of a page script, as JSON text in bytes, in either direction: data from
/// untrusted pages. A plugin panel's messages have their own limit, the setting
/// `plugins.panelMessageMaxBytes` (RB-090-44, the application's `plugins::panel_limit`).
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;
/// The most posts one answer may carry; the rest are dropped.
pub const MAX_POSTS: usize = 16;
/// The key of an answer object whose posts the host delivers.
pub const POST_KEY: &str = "$post";
/// The most requests of one plugin waiting for its component at once; more are refused with
/// `plugins.error.busy`.
pub const MAX_PENDING: usize = 32;
/// The code of a request refused because [`MAX_PENDING`] wait already.
pub const BUSY: &str = "plugins.error.busy";
/// The code of a request the host no longer answers: the page script was switched off.
pub const STOPPED: &str = "plugins.error.stopped";
/// The code of a request a page script gave up after [`PAGE_TIMEOUT_MILLISECONDS`] without an
/// answer (RB-080-66: not `plugins.error.timed_out`, which says the plugin was stopped).
pub const NO_ANSWER: &str = "plugins.error.no_answer";
/// How long a page script waits for an answer before its promise is rejected ([`NO_ANSWER`]).
pub const PAGE_TIMEOUT_MILLISECONDS: u32 = 30_000;

/// The isolated world of plugin `plugin_id`'s page scripts.
#[must_use]
pub fn world_name(plugin_id: &str) -> String {
    format!("rbplugin-{plugin_id}")
}

/// One page script of a plugin: when it runs and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageScriptSource {
    /// `document-start`, `document-end` or `document-idle`.
    pub run_at: String,
    /// The package member's path, for messages.
    pub path: String,
    pub text: String,
}

/// A plugin's page scripts with the names this start gave their binding and receiver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageBridge {
    pub plugin_id: String,
    pub world: String,
    /// The CDP binding the page scripts call (`Runtime.addBinding`); random per start.
    pub binding: String,
    /// The function in the world the host calls with replies and posts; random per start.
    pub receiver: String,
    pub scripts: Vec<PageScriptSource>,
}

impl PageBridge {
    /// The bridge of `manifest`'s page scripts, their texts read from `files` (package member
    /// to bytes); `None` without a page script. A script that is missing or no UTF-8 is left
    /// out with a warning.
    #[must_use]
    pub fn new(manifest: &PluginManifest, file: impl Fn(&str) -> Option<Vec<u8>>) -> Option<Self> {
        let scripts: Vec<PageScriptSource> = manifest
            .page_scripts
            .iter()
            .filter_map(|script| {
                let text = file(&script.path).and_then(|bytes| String::from_utf8(bytes).ok());
                if text.is_none() {
                    tracing::warn!(
                        plugin_id = manifest.id.as_str(),
                        path = script.path.as_str(),
                        "a page script is missing or no UTF-8; left out"
                    );
                }
                Some(PageScriptSource {
                    run_at: script.run_at.clone(),
                    path: script.path.clone(),
                    text: text?,
                })
            })
            .collect();
        if scripts.is_empty() {
            return None;
        }
        let token = random_token();
        Some(Self {
            plugin_id: manifest.id.clone(),
            world: world_name(&manifest.id),
            binding: format!("__rbp_{token}"),
            receiver: format!("__rbr_{token}"),
            scripts,
        })
    }

    /// The source injected into every document of `view`: the prologue defining `rbPlugin`,
    /// then each script in the order of the manifest.
    #[must_use]
    pub fn source(&self, view: u32) -> String {
        source::page_source(self, view)
    }
}

fn random_token() -> String {
    let mut bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut bytes);
    rb_util::hex(&bytes)
}

/// What a page script sent through its binding.
#[derive(Clone, Debug, PartialEq)]
pub enum PageInput {
    /// A request the component answers; `id` names the answer.
    Request { id: u32, message: Value },
    /// An event for the plugin's panels.
    Emit { message: Value },
    /// A line for rBrowser's log and console (RB-080-66).
    Log(PageLog),
}

/// Why a page script's payload was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageRefusal {
    /// Over [`MAX_MESSAGE_BYTES`]; a request's id when it had one, so the page gets an answer.
    TooLarge { id: Option<u32> },
    /// Not the binding's format, or no JSON.
    Malformed { id: Option<u32> },
}

/// Reads a binding payload: `s<id>:<json>`, `e:<json>` or `l:<json>`. The size is checked
/// before the JSON is parsed.
///
/// # Errors
///
/// [`PageRefusal`]: too large, or malformed.
pub fn parse_page_payload(payload: &str) -> Result<PageInput, PageRefusal> {
    let (head, json) = payload
        .split_once(':')
        .ok_or(PageRefusal::Malformed { id: None })?;
    let id = match head.strip_prefix('s') {
        Some(digits) if !digits.is_empty() && digits.len() <= 10 => Some(
            digits
                .parse::<u32>()
                .map_err(|_| PageRefusal::Malformed { id: None })?,
        ),
        None if head == "e" || head == "l" => None,
        _ => return Err(PageRefusal::Malformed { id: None }),
    };
    if json.len() > MAX_MESSAGE_BYTES {
        return Err(PageRefusal::TooLarge { id });
    }
    let message: Value = serde_json::from_str(json).map_err(|_| PageRefusal::Malformed { id })?;
    Ok(match id {
        Some(id) => PageInput::Request { id, message },
        None if head == "l" => {
            PageInput::Log(PageLog::from_value(message).ok_or(PageRefusal::Malformed { id: None })?)
        }
        None => PageInput::Emit { message },
    })
}

/// The arguments of the component's [`PAGE_MESSAGE_TOOL`] call.
#[must_use]
pub fn page_request_args(view: u32, message: &Value) -> String {
    json!({ "view": view, "message": message }).to_string()
}

/// The expression that hands `message` to the receiver `receiver`. JSON is a JavaScript
/// expression, so nothing in the message can break out of the call.
#[must_use]
pub fn receiver_call(receiver: &str, message: &Value) -> String {
    format!(
        "(()=>{{const r=globalThis[{}];if(typeof r===\"function\")r({});}})()",
        Value::String(receiver.to_owned()),
        message
    )
}

/// The answer to request `id`: the component's value, or a failure's code.
#[must_use]
pub fn reply_message(id: u32, result: Result<&Value, &str>) -> Value {
    match result {
        Ok(value) => json!({ "reply": id, "ok": true, "value": value }),
        Err(code) => json!({ "reply": id, "ok": false, "error": code }),
    }
}

/// A post for the page scripts' `rbPlugin.on` listeners.
#[must_use]
pub fn post_message(message: &Value) -> Value {
    json!({ "post": message })
}

/// The host switched the page scripts off: `rbPlugin.onStop` listeners run, pending requests
/// are rejected.
#[must_use]
pub fn stop_message() -> Value {
    json!({ "stop": true })
}

/// Where a post goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostTarget {
    /// The plugin's page scripts in every view.
    All,
    /// Its page scripts in one view.
    View(u32),
    /// Its open panels, as an `event`.
    Panel,
    /// The status bar: `{"key": "plugin.<id>.<name>", "params": {...}, "level": "info" |
    /// "success"}`, the plugin's own text, checked by the application (RB-080-64).
    Status,
    /// An action the application offers this plugin: `{"action": "<name>", ...}`, only from the
    /// application's list per plugin (RB-080-74: the image overlay's file dialog for a design).
    Host,
}

/// One post of an answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Post {
    pub target: PostTarget,
    pub message: Value,
}

/// Splits `answer` into the answer proper and its posts: an object's [`POST_KEY`] list is
/// taken out. A malformed entry, a message over [`MAX_MESSAGE_BYTES`] and every entry past
/// [`MAX_POSTS`] are dropped; their count is the third value.
#[must_use]
pub fn split_posts(answer: Value) -> (Value, Vec<Post>, usize) {
    let Value::Object(mut object) = answer else {
        return (answer, Vec::new(), 0);
    };
    let Some(posts) = object.remove(POST_KEY) else {
        return (Value::Object(object), Vec::new(), 0);
    };
    let entries = match posts {
        Value::Array(entries) => entries,
        _ => return (Value::Object(object), Vec::new(), 1),
    };
    let total = entries.len();
    let posts: Vec<Post> = entries
        .into_iter()
        .take(MAX_POSTS)
        .filter_map(post_of)
        .collect();
    let dropped = total - posts.len();
    (Value::Object(object), posts, dropped)
}

fn post_of(entry: Value) -> Option<Post> {
    let Value::Object(mut entry) = entry else {
        return None;
    };
    let target = match entry.get("target")? {
        Value::String(text) if text == "all" => PostTarget::All,
        Value::String(text) if text == "panel" => PostTarget::Panel,
        Value::String(text) if text == "status" => PostTarget::Status,
        Value::String(text) if text == "host" => PostTarget::Host,
        Value::Number(number) => PostTarget::View(u32::try_from(number.as_u64()?).ok()?),
        _ => return None,
    };
    let message = entry.remove("message").unwrap_or(Value::Null);
    (message.to_string().len() <= MAX_MESSAGE_BYTES).then_some(Post { target, message })
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;

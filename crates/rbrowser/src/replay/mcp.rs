//! MCP's `click`, `type`, `hover` and `press_key` (RB-030-01 registers them): the arguments,
//! the answer and the functions behind them. They build the events the sync agent would have
//! captured and replay them with [`CdpReplay::replay_into`] - the same dispatch as the sync, as
//! real input. The input goes exactly to the views of `target` (the active view when absent):
//! the echo marker keeps the active view's agent from capturing it, so the sync does not carry
//! it further, and leader-only submit does not apply (a submit button clicked in every view
//! submits in every view). Typed text never appears in a log, an error or the answer. Each view
//! has `mcp.navigationTimeoutMs` for all of its input; a view out of time answers `timeout`, and
//! part of the input may have reached it.

use std::sync::Arc;
use std::time::Duration;

use futures_util::future::join_all;
use rb_commands::CommandError;
use rb_engine::{Target, ViewId};
use rb_sync::{MOD_ALT, MOD_CTRL, MOD_META, MOD_SHIFT, ReplayOutcome, SyncEvent};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::CdpReplay;
use super::keys::named_key;
use crate::host::Host;

/// `click`'s name.
pub const CLICK_TOOL: &str = "click";
/// `click`'s description.
pub const CLICK_DESCRIPTION: &str = "Click an element (CSS `selector`) as real input in the \
     views of `target` (\"all\" or a view id; the active view when absent): `button` left \
     (default), middle or right, `clickCount` 1 to 3, `modifiers` (Alt, Control, Meta, Shift). \
     Touch views get a tap. Answers per view `done`, `notFound`, `notVisible` or a failure.";
/// `type`'s name.
pub const TYPE_TOOL: &str = "type";
/// `type`'s description.
pub const TYPE_DESCRIPTION: &str = "Type `text` as real input in the views of `target`: into \
     the element of `selector` after a click on it, else into the focused element. Works with \
     every keyboard layout. Answers per view; the text never appears in the answer.";
/// `hover`'s name.
pub const HOVER_TOOL: &str = "hover";
/// `hover`'s description.
pub const HOVER_DESCRIPTION: &str = "Move the mouse onto an element (CSS `selector`) in the \
     views of `target`, so `:hover` styles and hover menus open. Answers per view.";
/// `press_key`'s name.
pub const PRESS_KEY_TOOL: &str = "press_key";
/// `press_key`'s description.
pub const PRESS_KEY_DESCRIPTION: &str = "Press a key in the views of `target`: `key` is a key \
     name (Enter, Tab, Escape, Backspace, Delete, ArrowUp/Down/Left/Right, Home, End, PageUp, \
     PageDown, F1-F24, \" \") or one letter or digit, `modifiers` Alt, Control, Meta, Shift. \
     Enter runs the default action (a form submit). Text goes through `type`. Answers per view.";

/// What a view out of time answers: the input is not undone.
pub const TIMEOUT_MESSAGE: &str = "the input did not finish within mcp.navigationTimeoutMs; \
     part of it may have been sent";

/// The longest selector taken.
const MAX_SELECTOR: usize = 2048;
/// The longest text `type` takes, in characters.
const MAX_TEXT: usize = 100_000;

/// A modifier key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Modifier {
    /// Alt.
    Alt,
    /// Control.
    Control,
    /// Meta (Windows key).
    Meta,
    /// Shift.
    Shift,
}

/// A mouse button `click` takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClickButton {
    /// Primary.
    #[default]
    Left,
    /// Wheel.
    Middle,
    /// Secondary.
    Right,
}

fn one() -> u32 {
    1
}

/// `click`'s arguments.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClickArgs {
    /// `"all"` or a view id; the active view when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// The element, a CSS selector.
    pub selector: String,
    /// The button.
    #[serde(default)]
    pub button: ClickButton,
    /// 1 to 3.
    #[serde(default = "one")]
    pub click_count: u32,
    /// Held during the click.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<Modifier>,
}

/// `hover`'s arguments.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HoverArgs {
    /// `"all"` or a view id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// The element, a CSS selector.
    pub selector: String,
}

/// `type`'s arguments. `Debug` masks the text.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TypeArgs {
    /// `"all"` or a view id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// The field, clicked first; the focused element when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// What to type.
    pub text: String,
}

impl std::fmt::Debug for TypeArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypeArgs")
            .field("target", &self.target)
            .field("selector", &self.selector)
            .field("text", &"<masked>")
            .finish()
    }
}

/// `press_key`'s arguments.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PressKeyArgs {
    /// `"all"` or a view id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// The key's name.
    pub key: String,
    /// Held during the key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<Modifier>,
}

/// How the input went in one view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputStatus {
    /// Sent.
    Done,
    /// The element is missing in the view.
    NotFound,
    /// The element is not rendered in the view.
    NotVisible,
    /// The view did not answer in time.
    Timeout,
    /// The view's engine cannot do it.
    Unsupported,
    /// It failed (`message`).
    Failed,
}

/// One view's result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewInput {
    /// The view.
    pub view: ViewId,
    /// How it went.
    pub status: InputStatus,
    /// Why it failed (CDP's reason; never the input).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl ViewInput {
    fn new(view: ViewId, outcome: ReplayOutcome) -> Self {
        let (status, message) = match outcome {
            ReplayOutcome::Done => (InputStatus::Done, None),
            ReplayOutcome::NotFound => (InputStatus::NotFound, None),
            ReplayOutcome::NotVisible => (InputStatus::NotVisible, None),
            ReplayOutcome::Timeout => (InputStatus::Timeout, Some(TIMEOUT_MESSAGE.to_owned())),
            ReplayOutcome::Unsupported => (InputStatus::Unsupported, None),
            ReplayOutcome::Failed(message) => (InputStatus::Failed, Some(message)),
        };
        Self {
            view,
            status,
            message,
        }
    }
}

/// The answer of all four tools.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputAnswer {
    /// Per view, in grid order.
    pub views: Vec<ViewInput>,
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidArgs(message.into())
}

/// The arguments in `args`. An unknown or missing field is named; any other error says no
/// more, since serde's message would quote the value (typed text).
fn parse<T: DeserializeOwned>(args: &Value) -> Result<T, CommandError> {
    serde_json::from_value(args.clone()).map_err(|error| {
        let message = error.to_string();
        if message.starts_with("unknown field") || message.starts_with("missing field") {
            invalid(message)
        } else {
            invalid("an argument has the wrong type or value")
        }
    })
}

fn bits(modifiers: &[Modifier]) -> u8 {
    modifiers.iter().fold(0, |bits, modifier| {
        bits | match modifier {
            Modifier::Alt => MOD_ALT,
            Modifier::Control => MOD_CTRL,
            Modifier::Meta => MOD_META,
            Modifier::Shift => MOD_SHIFT,
        }
    })
}

/// The element target of `selector`, at its centre.
fn element(selector: &str) -> Result<Value, CommandError> {
    let selector = selector.trim();
    if selector.is_empty() || selector.len() > MAX_SELECTOR {
        return Err(invalid(format!(
            "`selector` must have 1 to {MAX_SELECTOR} bytes"
        )));
    }
    Ok(json!({ "element": { "kind": "css", "path": selector }, "offset": { "x": 0.5, "y": 0.5 } }))
}

fn event(fields: Value) -> Result<SyncEvent, CommandError> {
    serde_json::from_value(fields).map_err(|_| invalid("the input does not form an event"))
}

/// `click`'s events: one click per count, as the engine counts a double click.
pub fn click_events(args: &ClickArgs) -> Result<Vec<SyncEvent>, CommandError> {
    if !(1..=3).contains(&args.click_count) {
        return Err(invalid("`clickCount` must be 1 to 3"));
    }
    let target = element(&args.selector)?;
    (1..=args.click_count)
        .map(|detail| {
            event(
                json!({ "type": "click", "button": args.button, "detail": detail,
                          "modifiers": bits(&args.modifiers), "target": target }),
            )
        })
        .collect()
}

/// `hover`'s event.
pub fn hover_events(args: &HoverArgs) -> Result<Vec<SyncEvent>, CommandError> {
    Ok(vec![event(
        json!({ "type": "hover", "target": element(&args.selector)? }),
    )?])
}

/// `type`'s events: a click on the field, then the text into it (or into the focused element).
pub fn type_events(args: &TypeArgs) -> Result<Vec<SyncEvent>, CommandError> {
    if args.text.is_empty() || args.text.chars().count() > MAX_TEXT {
        return Err(invalid(format!(
            "`text` must have 1 to {MAX_TEXT} characters"
        )));
    }
    let mut events = Vec::new();
    let field = match &args.selector {
        Some(selector) => {
            let target = element(selector)?;
            events.push(event(
                json!({ "type": "click", "button": "left", "detail": 1,
                                       "modifiers": 0, "target": target }),
            )?);
            target
        }
        None => element(":focus")?,
    };
    events.push(event(
        json!({ "type": "input", "inputType": "insertText", "data": args.text,
                               "target": field }),
    )?);
    Ok(events)
}

/// `press_key`'s events: the key down and up.
pub fn press_key_events(args: &PressKeyArgs) -> Result<Vec<SyncEvent>, CommandError> {
    let key = named_key(&args.key)
        .ok_or_else(|| invalid("`key` is not a key name, a letter or a digit"))?;
    let fields = |kind: &str| {
        json!({ "type": kind, "key": key.key, "code": key.code, "keyCode": key.key_code,
                "location": 0, "repeat": false, "modifiers": bits(&args.modifiers) })
    };
    Ok(vec![event(fields("keydown"))?, event(fields("keyup"))?])
}

/// Replays `events` in order into every view of `target`, the views side by side, each within
/// `timeout` (`mcp.navigationTimeoutMs`); a view stops at its first event that is not done.
async fn run(
    host: &Arc<Host>,
    target: Option<Target>,
    events: Vec<SyncEvent>,
    timeout: Duration,
) -> Result<InputAnswer, CommandError> {
    let views = host
        .target_views(target)
        .map_err(|error| invalid(error.to_string()))?;
    let replay = CdpReplay::for_host(host);
    let (replay, events) = (&replay, &events);
    let runs = views.into_iter().map(move |view| async move {
        let all = async {
            for event in events {
                let outcome = replay.replay_into(view, event, true, false).await;
                if outcome != ReplayOutcome::Done {
                    return outcome;
                }
            }
            ReplayOutcome::Done
        };
        let outcome = tokio::time::timeout(timeout, all)
            .await
            .unwrap_or(ReplayOutcome::Timeout);
        ViewInput::new(view, outcome)
    });
    Ok(InputAnswer {
        views: join_all(runs).await,
    })
}

/// Runs `click` with `args` (its JSON), each view within `timeout`.
pub async fn click(
    host: &Arc<Host>,
    args: &Value,
    timeout: Duration,
) -> Result<InputAnswer, CommandError> {
    let args: ClickArgs = parse(args)?;
    run(host, args.target, click_events(&args)?, timeout).await
}

/// Runs `hover` with `args`.
pub async fn hover(
    host: &Arc<Host>,
    args: &Value,
    timeout: Duration,
) -> Result<InputAnswer, CommandError> {
    let args: HoverArgs = parse(args)?;
    run(host, args.target, hover_events(&args)?, timeout).await
}

/// Runs `type` with `args`.
pub async fn type_text(
    host: &Arc<Host>,
    args: &Value,
    timeout: Duration,
) -> Result<InputAnswer, CommandError> {
    let args: TypeArgs = parse(args)?;
    run(host, args.target, type_events(&args)?, timeout).await
}

/// Runs `press_key` with `args`.
pub async fn press_key(
    host: &Arc<Host>,
    args: &Value,
    timeout: Duration,
) -> Result<InputAnswer, CommandError> {
    let args: PressKeyArgs = parse(args)?;
    run(host, args.target, press_key_events(&args)?, timeout).await
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;

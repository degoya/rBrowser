//! Check steps from the page menu (RB-090-60): "Check: element visible", "Check: text visible"
//! and "Wait for element" - `recorder.check { kind, view?, x?, y?, dip? }` (`Leader F K`, a
//! visible check on the element the next click picks). The element is picked and named in
//! [`super::pick_locator`] (as a recorded click names it), and the step runs in the views of the
//! recording whose page has that element (`onlyIn`). A recording must run.

use rb_commands::CommandError;
use rb_engine::ViewId;
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_sync::ElementTarget;
use rb_sync::recorder::{Action, Assertion, Locator};
use serde::Deserialize;
use serde_json::{Value, json};

#[cfg(test)]
pub(crate) use super::pick_locator::described;
use super::pick_locator::{pick_element, point_of};
use crate::control::Core;
use crate::devtools::inspect::Point;

/// `Leader F K`: add a check step for the element under the pointer (or the one clicked).
pub const CHECK: &str = "recorder.check";
/// The longest text a text check keeps, in characters.
pub const MAX_TEXT: usize = 500;

/// The notice while a check waits for the click that picks its element: the capture's pick
/// hint (`capture.notice.pick`).
#[must_use]
pub fn pick_notice() -> CoreToShell {
    CoreToShell::Notification(Notification {
        level: NotificationLevel::Info,
        message_key: crate::capture::PICK_KEY.to_owned(),
        args: std::collections::BTreeMap::new(),
        settings_key: None,
    })
}

/// What a check step checks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckKind {
    /// The element is visible.
    #[default]
    Visible,
    /// The element shows its text as it does now.
    Text,
    /// Wait until the element is there.
    WaitFor,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    #[serde(default)]
    kind: CheckKind,
    view: Option<u32>,
    x: Option<f64>,
    y: Option<f64>,
    dip: Option<bool>,
}

/// `recorder.check`'s arguments, read.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckArgs {
    pub kind: CheckKind,
    /// The view; the active one when absent.
    pub view: Option<ViewId>,
    /// The point and whether it is in DIP of the view's widget; a pick without.
    pub point: Option<(Point, bool)>,
}

/// Reads `recorder.check`'s arguments: `x` and `y` together or neither (a pick).
///
/// # Errors
///
/// An unknown field or kind, a lone or negative coordinate.
pub fn parse(args: &Value) -> Result<CheckArgs, CommandError> {
    let invalid = CommandError::InvalidArgs;
    let raw: Raw = if args.is_null() {
        Raw::default()
    } else {
        serde_json::from_value(args.clone()).map_err(|error| invalid(error.to_string()))?
    };
    let point = point_of(raw.x, raw.y, raw.dip)?;
    Ok(CheckArgs {
        kind: raw.kind,
        view: raw.view.map(ViewId),
        point,
    })
}

/// The step for `kind` on `target` showing `text`.
///
/// # Errors
///
/// A text check on an element without text.
pub fn action_of(kind: CheckKind, target: &ElementTarget, text: &str) -> Result<Action, String> {
    let target = Locator::from_target(target, None);
    Ok(match kind {
        CheckKind::Visible => Action::Assert {
            assertion: Assertion::Visible { target },
        },
        CheckKind::Text => {
            let text: String = text.chars().take(MAX_TEXT).collect();
            if text.trim().is_empty() {
                return Err("the element shows no text to check".to_owned());
            }
            Action::Assert {
                assertion: Assertion::Text { target, text },
            }
        }
        CheckKind::WaitFor => Action::WaitFor { target },
    })
}

/// `recorder.check`. Runs on a command thread of the core's runtime, never on the UI thread: the
/// resolution blocks on that runtime (a pick waits for a click).
pub fn run(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let args = parse(args)?;
    let failed = CommandError::Failed;
    let recorder = super::recorder_of(core);
    if recorder.outcome().name.is_none() {
        return Err(failed(
            "no recording is running: start one, then add checks".to_owned(),
        ));
    }
    let picked = pick_element(core, args.view, args.point)?;
    let action =
        action_of(args.kind, &picked.target, &picked.text).map_err(CommandError::InvalidArgs)?;
    let outcome = recorder
        .add_check(picked.source, action, &picked.views)
        .map_err(|error| failed(error.to_string()))?;
    Ok(json!({ "steps": outcome.steps, "views": picked.views }))
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;

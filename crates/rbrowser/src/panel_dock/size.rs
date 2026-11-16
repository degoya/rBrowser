//! The dock's size from a command or a drag (RB-080-57): a size given, a step from the size
//! shown (palette and MCP; the header folds the dock instead, RB-080-61), and the bar's drag - each within [`MIN_DOCK`] and
//! [`MAX_DOCK`], a step and a drag also within what the window can show.

use rb_commands::CommandError;
use serde_json::Value;

use super::PanelSide;
use crate::devtools::layout::{MAX_DOCK, MIN_DOCK};

/// How much one step changes the size, in DIP.
pub const STEP: u32 = 40;

/// The size a dock command asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wanted {
    /// The side's kept size.
    Kept,
    /// This size, kept for the side.
    Size(u32),
    /// One [`STEP`] larger (`1`) or smaller (`-1`) than shown.
    Step(i8),
}

/// What `args` asks for: `size` or `step`, at most one of them.
///
/// # Errors
///
/// [`CommandError::InvalidArgs`] for a bad value or both.
pub fn wanted(args: &Value) -> Result<Wanted, CommandError> {
    let size = size_arg(args)?;
    let step = match args.get("step") {
        None | Some(Value::Null) => None,
        Some(step) => Some(
            step.as_i64()
                .filter(|step| *step == 1 || *step == -1)
                .and_then(|step| i8::try_from(step).ok())
                .ok_or_else(|| CommandError::InvalidArgs("step is 1 or -1".to_owned()))?,
        ),
    };
    match (size, step) {
        (Some(_), Some(_)) => Err(CommandError::InvalidArgs(
            "give size or step, not both".to_owned(),
        )),
        (Some(size), None) => Ok(Wanted::Size(size)),
        (None, Some(step)) => Ok(Wanted::Step(step)),
        (None, None) => Ok(Wanted::Kept),
    }
}

/// The `size` a dock command was given, checked: absent, or [`MIN_DOCK`] to [`MAX_DOCK`] DIP.
///
/// # Errors
///
/// [`CommandError::InvalidArgs`] for anything else.
pub fn size_arg(args: &Value) -> Result<Option<u32>, CommandError> {
    match args.get("size") {
        None | Some(Value::Null) => Ok(None),
        Some(size) => size
            .as_u64()
            .and_then(|size| u32::try_from(size).ok())
            .filter(|size| (MIN_DOCK..=MAX_DOCK).contains(size))
            .map(Some)
            .ok_or_else(|| {
                CommandError::InvalidArgs(format!(
                    "size is a whole number of DIP from {MIN_DOCK} to {MAX_DOCK}"
                ))
            }),
    }
}

/// One step from `shown` in `direction`, within [`MIN_DOCK`] and `ceiling`.
#[must_use]
pub fn stepped(shown: u32, direction: i8, ceiling: u32) -> u32 {
    let size = if direction < 0 {
        shown.saturating_sub(STEP)
    } else {
        shown.saturating_add(STEP)
    };
    size.clamp(MIN_DOCK, ceiling.clamp(MIN_DOCK, MAX_DOCK))
}

/// The dock's size after a drag of the bar by `delta` DIP from `start`: towards the views it
/// grows (right for the left dock, left or up for the others), within [`MIN_DOCK`] and
/// [`MAX_DOCK`].
#[must_use]
pub fn dragged(side: PanelSide, start: u32, delta: i32) -> u32 {
    let size = match side {
        PanelSide::Left => i64::from(start) + i64::from(delta),
        PanelSide::Right | PanelSide::Bottom => i64::from(start) - i64::from(delta),
    };
    u32::try_from(size.clamp(i64::from(MIN_DOCK), i64::from(MAX_DOCK))).unwrap_or(MIN_DOCK)
}

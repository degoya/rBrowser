//! Form sync (RB-020-05): the `change` of a select (also `multiple`), radio, checkbox, range,
//! colour, date or text field and a contenteditable's text reach the other views as the
//! control's state, set there by the sync agent; focus sync (`sync.focus`, off by default); the
//! hint and the context menu entry of "do not sync the next click" (`sync.skip_next`).
//!
//! | Part | Where |
//! | --- | --- |
//! | Event to action (apply, focus, skipped file input) | [`plan`] |
//! | The evaluation in the agent's world, its answer | [`script`] |
//! | The replay into one view (called by `crate::replay`'s `CdpReplay`) | [`replay`] |
//! | `sync.toggle_focus`, the context menu entry | [`commands`] |
//! | The skip hint in the label strip | [`hint`] |
//! | Capture and apply in the page | `agent/src/forms/` |
//!
//! Password values travel to the other views - they need them - but are never logged:
//! `rb_sync::ChangeData`'s `Debug` masks every value, no error quotes the expression or the
//! agent's answer, and MCP and the recorder see `rb_sync::redact`'s mask. The rules are
//! `docs/sync-protocol.md` ("Form sync").

mod apply;
pub mod commands;
pub mod hint;
pub mod plan;
pub mod script;

pub use apply::replay;
pub use commands::{SKIP_NEXT_MENU_KEY, TOGGLE_FOCUS, run_command, skip_next_click_in};
pub use hint::SkipHint;
pub use plan::{FormAction, plan};

#[cfg(test)]
#[path = "log_tests.rs"]
mod log_tests;

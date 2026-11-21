//! What a recording captures (RB-050-01, RB-090-81): why the active view's agent would feed
//! it nothing, the hover capture of `automation.recorder.recordHover` and the step kinds whose
//! sync switch is off. Re-exported by [`super::glue`].

use rb_config::RecordHover;
use rb_sync::SyncSwitches;
use rb_sync::recorder::HoverCapture;

/// Why the active view's agent captures nothing - so a recording would stay empty: no active
/// view, sync off, or the active view excluded. The recorder hears only what that agent
/// captures, and the agent captures only while sync routes its events.
pub fn uncaptured(
    switches: SyncSwitches,
    excluded: bool,
    has_leader: bool,
) -> Option<&'static str> {
    if !has_leader {
        Some("no view is active to record from")
    } else if !switches.enabled {
        Some(
            "sync is off: the recorder records what the active view's sync agent captures - turn sync on first",
        )
    } else if excluded {
        Some(
            "the active view is excluded from sync: the recorder records what its sync agent captures - include it first",
        )
    } else {
        None
    }
}

/// The recorder's hover capture for `automation.recorder.recordHover` (RB-090-81). A function,
/// not a `From`: neither type is this crate's, and neither crate depends on the other.
pub fn hover_capture(choice: RecordHover) -> HoverCapture {
    match choice {
        RecordHover::Menu => HoverCapture::Menu,
        RecordHover::All => HoverCapture::All,
        RecordHover::Off => HoverCapture::Off,
    }
}

/// The kinds of steps whose capture is off by their sync switch: clicks, input, keys and form
/// controls always, hover and scroll when the recorder records them.
pub fn missing_kinds(
    switches: SyncSwitches,
    record_hover: bool,
    record_scroll: bool,
) -> Vec<&'static str> {
    [
        ("click", switches.click),
        ("input", switches.input),
        ("keyboard", switches.keyboard),
        ("forms", switches.forms),
        ("hover", switches.hover || !record_hover),
        ("scroll", switches.scroll || !record_scroll),
    ]
    .into_iter()
    .filter(|(_, on)| !on)
    .map(|(kind, _)| kind)
    .collect()
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;

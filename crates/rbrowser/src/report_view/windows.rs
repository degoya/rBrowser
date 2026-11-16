//! The report windows' bookkeeping, portable (security review L3 and L4 of RB-091-02): which
//! browsers are report windows - the report route answers only their main frames
//! (`cef_app/report_scheme.rs`), as the video route answers only the shell's - and where a report
//! goes: the one report window for [`Placement::Single`], a new window for
//! [`Placement::PerReport`] up to [`MAX_WINDOWS`], past that the oldest window.

use std::collections::BTreeSet;
use std::sync::Mutex;

use super::ask::Placement;

/// The most report windows open at once.
pub const MAX_WINDOWS: usize = 8;

/// The browser ids of the open report windows.
static BROWSERS: Mutex<BTreeSet<i32>> = Mutex::new(BTreeSet::new());

/// Browser `id` is a report window's (its view created it).
pub fn register_browser(id: i32) {
    rb_util::lock(&BROWSERS).insert(id);
}

/// Browser `id` closed.
pub fn forget_browser(id: i32) {
    rb_util::lock(&BROWSERS).remove(&id);
}

/// Whether a report request from browser `browser` (its main frame when `main`) is answered:
/// only a report window's main frame - never a grid view, a popup, the shell or a frame.
#[must_use]
pub fn requester_allowed(browser: Option<i32>, main: bool) -> bool {
    main && browser.is_some_and(|id| rb_util::lock(&BROWSERS).contains(&id))
}

/// An open report window: its slot id (oldest first) and whether it is the one report window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Open {
    pub id: u64,
    pub single: bool,
}

/// Where a report goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Into the open window with this slot id.
    Reuse(u64),
    /// Into a new window.
    New,
}

/// Where a report with `placement` goes, given the windows `open` (oldest first).
#[must_use]
pub fn choose(open: &[Open], placement: Placement) -> Target {
    if placement == Placement::Single
        && let Some(single) = open.iter().find(|window| window.single)
    {
        return Target::Reuse(single.id);
    }
    match open.first() {
        Some(oldest) if open.len() >= MAX_WINDOWS => Target::Reuse(oldest.id),
        _ => Target::New,
    }
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;

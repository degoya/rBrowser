//! The keyboard's way between the window's CEF views (RB-090-23, WCAG 2.1.1): Tab past the last
//! control of one view moves the focus into the next one, Shift+Tab past the first into the one
//! before - the toolbar, the active view, the status bar, and round again. CEF asks the view's
//! client in `on_take_focus` (`cef_app/take_focus.rs`); unanswered, the focus stayed at the
//! view's end and the status bar was out of the keyboard's reach. A stop that is not there (no
//! view yet, the status bar hidden) is skipped.

/// A view of the window that takes part in the Tab order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusStop {
    /// The shell's toolbar on top.
    Toolbar,
    /// The active grid view.
    ActiveView,
    /// The status bar at the bottom (RB-080-15).
    StatusBar,
}

/// The Tab order, top to bottom.
const ORDER: [FocusStop; 3] = [
    FocusStop::Toolbar,
    FocusStop::ActiveView,
    FocusStop::StatusBar,
];

/// The stop after `from` (`forward`, Tab) or before it (Shift+Tab), skipping the stops `present`
/// says are not there; `None` when no other stop is.
pub fn next_stop(
    from: FocusStop,
    forward: bool,
    present: impl Fn(FocusStop) -> bool,
) -> Option<FocusStop> {
    let len = ORDER.len();
    let at = ORDER.iter().position(|stop| *stop == from)?;
    (1..len)
        .map(|step| {
            if forward {
                (at + step) % len
            } else {
                (at + len - step) % len
            }
        })
        .filter_map(|index| ORDER.get(index).copied())
        .find(|stop| present(*stop))
}

#[cfg(test)]
#[path = "focus_cycle_tests.rs"]
mod tests;

//! The docked panels folded to a strip and opened again (RB-080-61): `panels.dock.toggle`, the
//! header's collapse button and `panels.dockState` (`rail` folded, RB-080-93; RB-080-61's
//! `panels.dockCollapsed` before). Folded, the dock keeps its side and its
//! size and shows only a strip - [`STRIP_HEIGHT`] for the header below the views,
//! [`STRIP_WIDTH`] for the icon column beside them - so the views get the room; the bar between
//! views and panels goes while it is folded.
//!
//! **Only the person unfolds it**: the toggle, the expand button, a panel chosen in the strip,
//! a command that asks for a panel - `plugins.panels`, `panels.findings`, `panels.recorder`,
//! `panels.flows`, `capture.video.panel`, a plugin's command of the Panels group (RB-090-82),
//! a check a person ran, whose findings it shows (`checks.run_all`, a plugin's command of the
//! Checks group, RB-099-25) - only ever a person's (RB-090-71, `show_unfolded`).
//! A dock command (`panels.dock.<side>`, the Dock menu) keeps the fold unless its `collapsed`
//! says otherwise; a panel opened as a side effect (another plugin command's `$open`, findings a
//! plugin reports on its own) and an MCP tool's check run keep it too - the page marks the
//! entry in the strip. Undocking ends the
//! fold; the window never folds, closing it brings the rail back (`rail`). The setting is the
//! one source the toggle and the page read.

use rb_commands::CommandError;
use rb_config::{PanelCloseTo, PanelDockState};
use serde_json::{Value, json};

use super::{PanelSide, store};
use crate::cdp::lock;
use crate::control::Core;
use crate::page_window::PageWindow;

/// The width of the folded dock beside the views, in DIP: the panels' icon column.
/// `ui/src/panels/dock.ts` (`STRIP`) lays the column out for it.
pub const STRIP_WIDTH: u32 = 44;
/// The height of the folded dock below the views, in DIP: the panels' header.
pub const STRIP_HEIGHT: u32 = 36;

/// The folded dock's size at `side`, in DIP at the text size in effect (RB-090-26).
#[must_use]
pub fn strip(side: PanelSide) -> u32 {
    crate::text_scale::dip(if side.horizontal() {
        STRIP_WIDTH
    } else {
        STRIP_HEIGHT
    })
}

/// The `collapsed` a command was given, checked: absent, or a boolean.
///
/// # Errors
///
/// [`CommandError::InvalidArgs`] for anything else.
pub fn arg(args: &Value) -> Result<Option<bool>, CommandError> {
    match args.get("collapsed") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(collapsed)) => Ok(Some(*collapsed)),
        Some(_) => Err(CommandError::InvalidArgs(
            "collapsed is true or false".to_owned(),
        )),
    }
}

/// `panels.dock.toggle` (the toolbar's switch, the rail's expand button, `Leader P Z`): the
/// panels opened at their size, or folded to their icon rail again (RB-080-93). `args.collapsed`
/// sets the state, without it the state flips; hidden panels open. Panels in their own window
/// open it, and fold by closing it - the rail comes back docked. Kept in `panels.dockState`; at
/// the panel `args` names.
///
/// # Errors
///
/// A bad `collapsed`.
pub fn toggle(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let wanted = arg(args)?;
    let mut user = core.user_settings();
    let side = PanelSide::of(user.panels.dock);
    // One source, the setting the page shows too (review point 10); panels not shown count as
    // folded: the toggle opens them.
    let shown = match side {
        None => lock(&super::state(core)).window_open,
        Some(_) => super::shown(core).is_some(),
    };
    let open = shown && user.panels.dock_state == PanelDockState::Open;
    let collapsed = wanted.unwrap_or(open);
    let url = super::url_for(core, args);
    if collapsed {
        lock(&super::state(core)).window_open = false;
        core.ui().hide_page_window(PageWindow::Panels);
        let answer = super::rail::close_to(core, PanelCloseTo::Rail, &url);
        super::select(core, args);
        return Ok(answer);
    }
    user.panels.dock_state = PanelDockState::Open;
    let Some(side) = side else {
        store::keep(core, user);
        super::open_window(core, url);
        super::select(core, args);
        return Ok(json!({ "position": "floating", "collapsed": false }));
    };
    let size = side.size_in(&user.panels.dock_size);
    store::keep(core, user);
    core.ui().hide_page_window(PageWindow::Panels);
    super::put(core, side, size, url, false);
    super::select(core, args);
    Ok(json!({ "position": side.key(), "size": size, "collapsed": false }))
}

#[cfg(test)]
#[path = "collapse_tests.rs"]
mod tests;

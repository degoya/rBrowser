//! The docked panels as an icon rail from the start (RB-080-93): the window starts with the
//! panels docked (right by default) and folded to their rail - the icon column beside the views,
//! the header below them - so the views have their room from the first layout and never jump.
//! Close (the X, `panels.close`, closing the panels' own window) folds them back to the rail
//! unless `panels.closeTo` says `hidden`; hiding them is a choice (that setting, the Dock
//! menu's "Hide the panels", `panels.close {to: "hidden"}`, `panels.dockState`).
//!
//! | Setting | What it says |
//! | --- | --- |
//! | `panels.dockState` | open, rail or hidden now; the one source the toggle and the pages read |
//! | `panels.dockAtStart` | rail (the default), hidden, or last - as they were left |
//! | `panels.closeTo` | rail (the default) or hidden |
//!
//! **Their own window** stays a choice (the Dock menu's "Own window", `panels.undock`); closing
//! it brings the rail back at the side the panels were docked at before (else right), and
//! `panels.dock` names that side again. A window never opens by itself at the start: panels left
//! in their window start at the rail.

use rb_commands::CommandError;
use rb_config::{PanelCloseTo, PanelDock, PanelDockAtStart, PanelDockState, PanelSettings};
use serde_json::{Value, json};

use super::{DockedPanels, PanelSide, state, store};
use crate::cdp::lock;
use crate::control::Core;
use crate::page_window::PageWindow;
use crate::plugins::commands::panels_url;

/// The side the rail goes to when nothing else says: right of the views.
pub const DEFAULT_SIDE: PanelSide = PanelSide::Right;

/// `panels.dockState` for folded (`true`, the rail) or open.
#[must_use]
pub fn state_of(collapsed: bool) -> PanelDockState {
    if collapsed {
        PanelDockState::Rail
    } else {
        PanelDockState::Open
    }
}

/// The panels settings as the window starts with them (`panels.dockAtStart`): the rail, hidden,
/// or as they were left; panels left in their own window start docked at the rail on the
/// right (the window does not open by itself; hidden ones stay hidden).
#[must_use]
pub fn at_start(panels: &PanelSettings) -> PanelSettings {
    let mut panels = *panels;
    panels.dock_state = match panels.dock_at_start {
        PanelDockAtStart::Rail => PanelDockState::Rail,
        PanelDockAtStart::Hidden => PanelDockState::Hidden,
        PanelDockAtStart::Last => panels.dock_state,
    };
    if panels.dock == PanelDock::Floating && panels.dock_state != PanelDockState::Hidden {
        panels.dock = DEFAULT_SIDE.setting();
        panels.dock_state = PanelDockState::Rail;
    }
    panels
}

/// The dock `panels` asks the window for, at the panels page `url`: none while hidden or in
/// their own window, else the side at its size, folded unless open.
#[must_use]
pub fn docked(panels: &PanelSettings, url: String) -> Option<DockedPanels> {
    let side = PanelSide::of(panels.dock)?;
    (panels.dock_state != PanelDockState::Hidden).then(|| DockedPanels {
        side,
        size: side.size_in(&panels.dock_size),
        url,
        collapsed: panels.dock_state != PanelDockState::Open,
    })
}

/// The window's first dock, before the first layout (CEF side, from the launch's settings): the
/// same [`at_start`] the core keeps in [`on_start`], so the views are laid out beside it once.
#[must_use]
pub fn first_dock(panels: &PanelSettings) -> Option<DockedPanels> {
    docked(&at_start(panels), panels_url(&Value::Null))
}

/// The core is up: the start rule becomes the settings in effect (written when it changed
/// them) and the dock the window shows, which [`first_dock`] made already.
pub fn on_start(core: &Core) {
    let mut user = core.user_settings();
    user.panels = at_start(&user.panels);
    let panels = user.panels;
    store::keep(core, user);
    if let Some(dock) = docked(&panels, panels_url(&Value::Null)) {
        super::put(core, dock.side, dock.size, dock.url, dock.collapsed);
    }
}

/// Where the rail stands for `dock`: its side, else the side before the panels' own window,
/// else [`DEFAULT_SIDE`].
fn rail_side(core: &Core, dock: PanelDock) -> PanelSide {
    PanelSide::of(dock)
        .or(lock(&state(core)).last_side)
        .unwrap_or(DEFAULT_SIDE)
}

/// What a Close was asked for: `args.to` (`rail` or `hidden`), else `panels.closeTo`.
///
/// # Errors
///
/// [`CommandError::InvalidArgs`] for another `to`.
pub fn close_target(core: &Core, args: &Value) -> Result<PanelCloseTo, CommandError> {
    match args.get("to") {
        None | Some(Value::Null) => Ok(core.user_settings().panels.close_to),
        Some(Value::String(to)) if to == "rail" => Ok(PanelCloseTo::Rail),
        Some(Value::String(to)) if to == "hidden" => Ok(PanelCloseTo::Hidden),
        Some(_) => Err(CommandError::InvalidArgs(
            "to is \"rail\" or \"hidden\"".to_owned(),
        )),
    }
}

/// `panels.close`: the panels go to `args.to` or `panels.closeTo` - folded to the icon rail
/// (their own window closes and the rail comes back docked) or hidden (no rail, no window).
///
/// # Errors
///
/// A bad `to`; nothing changes then.
pub fn close(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let target = close_target(core, args)?;
    lock(&state(core)).window_open = false;
    core.ui().hide_page_window(PageWindow::Panels);
    Ok(close_to(core, target, &super::url_for(core, args)))
}

/// Folds the panels to the rail or hides them; the window is gone already.
pub(super) fn close_to(core: &Core, target: PanelCloseTo, url: &str) -> Value {
    let mut user = core.user_settings();
    match target {
        PanelCloseTo::Hidden => {
            user.panels.dock_state = PanelDockState::Hidden;
            store::keep(core, user);
            super::remove(core);
            json!({ "position": "closed", "state": "hidden" })
        }
        PanelCloseTo::Rail => {
            let side = rail_side(core, user.panels.dock);
            user.panels.dock = side.setting();
            user.panels.dock_state = PanelDockState::Rail;
            let size = side.size_in(&user.panels.dock_size);
            store::keep(core, user);
            super::put(core, side, size, url.to_owned(), true);
            json!({ "position": side.key(), "size": size, "collapsed": true })
        }
    }
}

/// The panels' own window was closed by hand (CEF side): Close as `panels.closeTo` says - the
/// rail docked again, or hidden. A window closed already changes nothing.
pub fn on_window_closed(core: &Core) {
    {
        let state = state(core);
        let mut state = lock(&state);
        if !state.window_open {
            return;
        }
        state.window_open = false;
    }
    let target = core.user_settings().panels.close_to;
    close_to(core, target, &super::url_for(core, &Value::Null));
}

/// The user settings changed (the settings window, `settings_set`): the panels follow
/// `panels.dock`, `panels.dockSize` and `panels.dockState` - another side, size or fold, the
/// rail that comes back, hidden panels gone, or their own window, which takes the dock's panel.
pub fn on_settings_changed(core: &Core) {
    let (shown, window_open) = {
        let state = state(core);
        let state = lock(&state);
        (state.shown.clone(), state.window_open)
    };
    let panels = core.user_settings().panels;
    match (PanelSide::of(panels.dock), panels.dock_state) {
        (_, PanelDockState::Hidden) => {
            super::remove(core);
            if window_open {
                lock(&state(core)).window_open = false;
                core.ui().hide_page_window(PageWindow::Panels);
            }
        }
        (Some(side), dock_state) => {
            let size = side.size_in(&panels.dock_size);
            let collapsed = dock_state != PanelDockState::Open;
            let url = match &shown {
                Some(shown) => shown.url.clone(),
                None => super::url_for(core, &Value::Null),
            };
            let same = shown.as_ref().is_some_and(|shown| {
                (shown.side, shown.size, shown.collapsed) == (side, size, collapsed)
            });
            if !same {
                super::put(core, side, size, url, collapsed);
            }
        }
        (None, PanelDockState::Open) => {
            if let Some(shown) = &shown {
                lock(&state(core)).last_side = Some(shown.side);
            }
            if !window_open {
                let url = super::url_for(core, &Value::Null);
                super::remove(core);
                super::open_window(core, url);
            }
        }
        // `floating` chosen while the panels stood docked: the person wants their own window -
        // it opens and the choice stays (wave c20 review). Only `rail` chosen while the window
        // is open (or nothing shows) folds them back to the docked rail.
        (None, PanelDockState::Rail) => match &shown {
            Some(shown) => {
                lock(&state(core)).last_side = Some(shown.side);
                let url = super::url_for(core, &Value::Null);
                let mut user = core.user_settings();
                user.panels.dock_state = PanelDockState::Open;
                store::keep(core, user);
                super::remove(core);
                super::open_window(core, url);
            }
            None => {
                lock(&state(core)).window_open = false;
                core.ui().hide_page_window(PageWindow::Panels);
                close_to(
                    core,
                    PanelCloseTo::Rail,
                    &super::url_for(core, &Value::Null),
                );
            }
        },
    }
}

#[cfg(test)]
#[path = "rail_tests.rs"]
mod tests;

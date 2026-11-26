//! The plugin panels docked in the main window (RB-080-57): the panels page (`#/panels`) that
//! the floating window shows (`crate::page_window::PageWindow::Panels`) can sit left, right or
//! below the views instead, in a browser view of its own (`cef_app/panel_dock.rs`). The views
//! give up the room as they do for docked DevTools - the dock and its bar are boxes beside the
//! content area, never over a view - and the area starts below the toolbar, however many rows
//! it has.
//!
//! | Part | Where |
//! | --- | --- |
//! | Where the panels go (`panels.dock`), the size per side (`panels.dockSize`) | [`show`], [`PanelSide`] |
//! | The commands `panels.dock.left|right|bottom`, `panels.undock`, `panels.close`, `panels.dock.toggle` | [`defs`] |
//! | Folded to a strip and opened again (`panels.dockState`, RB-080-61) | [`collapse`] |
//! | The icon rail from the start, Close to the rail or hidden (RB-080-93), a changed setting | [`rail`] |
//! | The core's own panels in the page: Findings (RB-080-69) | [`core_panels`] |
//! | The bar between views and panels (the DevTools splitter's page) | [`on_title`] |
//! | The settings file | [`store`] |
//!
//! **One dock.** The panels page holds every panel in its own list, so docking moves that page:
//! one dock at a time, its tabs the page's list - never a second dock. With DevTools docked as
//! well, both stand: on another side each takes its own room; on the same side the panels sit
//! outside, farther from the views.
//!
//! **Same rights.** The docked page is the floating window's page with the window's handlers
//! (`PagesClient`): the same navigation guard, no popups, the same query check by browser id,
//! the same sandboxed panel frames. Nothing new reaches the core.

pub mod collapse;
pub mod core_panels;
pub mod defs;
pub mod rail;
pub mod size;
pub mod store;

use std::sync::{Arc, Mutex, OnceLock, Weak};

use rb_commands::CommandError;
use rb_config::{PanelDock, PanelDockSize, PanelDockState};
use rb_ipc::CoreToShell;
use serde_json::{Value, json};

use crate::cdp::lock;
use crate::control::Core;
use crate::devtools::layout::{MAX_DOCK, MIN_DOCK};
use crate::devtools::splitter::{Drag, parse_title};
use crate::host::Host;
use crate::page_window::PageWindow;
use crate::plugins::commands::panels_url;

pub use defs::register;
pub use rail::{close, on_settings_changed, on_start, on_window_closed};
pub use size::{dragged, size_arg};

/// A side of the views the panels dock at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PanelSide {
    /// Left of the views.
    Left,
    /// Right of the views.
    Right,
    /// Below the views.
    Bottom,
}

impl PanelSide {
    /// Every side.
    pub const ALL: [Self; 3] = [Self::Left, Self::Right, Self::Bottom];

    /// The side of the setting `dock`; `None` for floating.
    #[must_use]
    pub fn of(dock: PanelDock) -> Option<Self> {
        match dock {
            PanelDock::Floating => None,
            PanelDock::Left => Some(Self::Left),
            PanelDock::Right => Some(Self::Right),
            PanelDock::Bottom => Some(Self::Bottom),
        }
    }

    /// The setting that names it.
    #[must_use]
    pub fn setting(self) -> PanelDock {
        match self {
            Self::Left => PanelDock::Left,
            Self::Right => PanelDock::Right,
            Self::Bottom => PanelDock::Bottom,
        }
    }

    /// Its name in the settings and in answers (`left`, `right`, `bottom`).
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Bottom => "bottom",
        }
    }

    /// Beside the views (a width) rather than below them (a height).
    #[must_use]
    pub fn horizontal(self) -> bool {
        self != Self::Bottom
    }

    /// The size `sizes` keeps for it, as stored.
    #[must_use]
    pub fn stored_in(self, sizes: &PanelDockSize) -> u32 {
        match self {
            Self::Left => sizes.left,
            Self::Right => sizes.right,
            Self::Bottom => sizes.bottom,
        }
    }

    /// The size `sizes` keeps for it, held between [`MIN_DOCK`] and [`MAX_DOCK`].
    #[must_use]
    pub fn size_in(self, sizes: &PanelDockSize) -> u32 {
        self.stored_in(sizes).clamp(MIN_DOCK, MAX_DOCK)
    }

    /// Keeps `size` for it in `sizes`.
    pub fn set_in(self, sizes: &mut PanelDockSize, size: u32) {
        match self {
            Self::Left => sizes.left = size,
            Self::Right => sizes.right = size,
            Self::Bottom => sizes.bottom = size,
        }
    }
}

/// The docked panels the window shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DockedPanels {
    /// Where.
    pub side: PanelSide,
    /// The width (left, right) or height (bottom) in DIP it is set to; the window shows at most
    /// what leaves the views a quarter of the area (`crate::devtools::layout::shared_extents`).
    pub size: u32,
    /// The panels page a new dock loads (`#/panels[/<plugin>.<panel>]`); a dock shown already
    /// keeps its page and hears the `select` event instead.
    pub url: String,
    /// Folded to its strip ([`collapse::strip`]) - `size` is what it opens to again.
    pub collapsed: bool,
}

/// What the window laid the dock out at (CEF side, after every layout of the dock).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Room {
    /// The side it was laid out at.
    pub side: PanelSide,
    /// Its size on screen, in DIP.
    pub shown: u32,
    /// The most it can have there now: what leaves the views their quarter, beside docked
    /// DevTools.
    pub ceiling: u32,
}

/// The dock's state in one window.
#[derive(Default)]
struct DockState {
    /// The dock shown, if any.
    shown: Option<DockedPanels>,
    /// The size when the bar's drag began.
    drag_start: Option<u32>,
    /// The dock's last layout.
    room: Option<Room>,
    /// The panel a panels page shows now (`#/panels/<plugin>.<panel>`), when one said.
    selected: Option<String>,
    /// The floating window was opened and has not been closed or hidden since.
    window_open: bool,
    /// The side the panels were docked at before they went into their own window: where the
    /// icon rail comes back when the window closes (RB-080-93).
    last_side: Option<PanelSide>,
}

impl DockState {
    /// The size of `side` on screen: the last layout's, else `stored`.
    fn shown_size(&self, side: PanelSide, stored: u32) -> u32 {
        self.room
            .filter(|room| room.side == side)
            .map_or(stored, |room| room.shown)
    }

    /// The most `side` can have: the last layout's ceiling, else [`MAX_DOCK`].
    fn ceiling(&self, side: PanelSide) -> u32 {
        self.room
            .filter(|room| room.side == side)
            .map_or(MAX_DOCK, |room| room.ceiling.clamp(MIN_DOCK, MAX_DOCK))
    }
}

type Registry = Mutex<Vec<(Weak<Host>, Arc<Mutex<DockState>>)>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(Registry::default)
}

/// The dock state of `core`'s window, made at its first use.
fn state(core: &Core) -> Arc<Mutex<DockState>> {
    let host = core.host();
    let mut entries = lock(registry());
    entries.retain(|(weak, _)| weak.strong_count() > 0);
    if let Some((_, state)) = entries
        .iter()
        .find(|(weak, _)| std::ptr::eq(weak.as_ptr(), Arc::as_ptr(host)))
    {
        return state.clone();
    }
    let state = Arc::new(Mutex::new(DockState::default()));
    entries.push((Arc::downgrade(host), state.clone()));
    state
}

/// `plugins.panels` (and a plugin command's `$open`): the panels page at `url`, where
/// `panels.dock` says - the floating window, or docked.
///
/// A folded dock stays folded and nothing is written (review point 2): this is a command's or a
/// check's open, never the person's choice - the page marks the panel in the rail, the person
/// unfolds. Hidden docked panels come back as the rail, so the mark shows (RB-080-93); the
/// floating window opens and `panels.dockState` says so.
pub fn show(core: &Core, url: String) {
    let mut user = core.user_settings();
    let panels = user.panels;
    match PanelSide::of(panels.dock) {
        None => {
            user.panels.dock_state = PanelDockState::Open;
            store::keep(core, user);
            open_window(core, url);
        }
        Some(side) => {
            if panels.dock_state == PanelDockState::Hidden {
                user.panels.dock_state = PanelDockState::Rail;
                store::keep(core, user);
            }
            let size = side.size_in(&panels.dock_size);
            let folded = panels.dock_state != PanelDockState::Open;
            put(core, side, size, url, folded);
        }
    }
}

/// The panels page at `url` because a person asked for a panel - a command from a menu, the
/// palette or a shortcut that no MCP tool runs (RB-090-71), or a check they ran (RB-099-25,
/// [`crate::mcp::origin`] tells a tool's run apart): unlike [`show`], a docked dock folded
/// to its rail or hidden opens at its size, and `panels.dockState` says so. The floating window
/// opens as [`show`] opens it.
pub fn show_unfolded(core: &Core, url: String) {
    let mut user = core.user_settings();
    let Some(side) = PanelSide::of(user.panels.dock) else {
        show(core, url);
        return;
    };
    let size = side.size_in(&user.panels.dock_size);
    user.panels.dock_state = PanelDockState::Open;
    store::keep(core, user);
    put(core, side, size, url, false);
}

/// Opens (or shows) the floating window at `url`.
fn open_window(core: &Core, url: String) {
    lock(&state(core)).window_open = true;
    core.ui().open_page_window(PageWindow::Panels, url);
}

/// Shows the dock at `side` and `size`, folded or open; a dock shown already keeps its page. The
/// floating window goes (the window closes it as the dock is made).
fn put(core: &Core, side: PanelSide, size: u32, url: String, collapsed: bool) {
    let state = state(core);
    let mut state = lock(&state);
    let url = match &state.shown {
        Some(shown) if url == panels_url(&Value::Null) => shown.url.clone(),
        _ => url,
    };
    // Until the window lays it out again, the dock is taken to show what it is set to.
    let ceiling = state.ceiling(side);
    if let Some(room) = state
        .room
        .as_mut()
        .filter(|room| room.side == side && !collapsed)
    {
        room.shown = size.min(ceiling);
    }
    state.window_open = false;
    let docked = DockedPanels {
        side,
        size,
        url,
        collapsed,
    };
    state.shown = Some(docked.clone());
    drop(state);
    core.ui().dock_panels(Some(docked));
}

/// The dock shown, if any.
fn shown(core: &Core) -> Option<DockedPanels> {
    lock(&state(core)).shown.clone()
}

/// The page of `args` (`{plugin, panel}`) when it names a panel, else the panel a panels page
/// shows now, else the dock's, else the panels page.
fn url_for(core: &Core, args: &Value) -> String {
    let named = panels_url(args);
    if named != panels_url(&Value::Null) {
        return named;
    }
    let state = state(core);
    let state = lock(&state);
    state
        .selected
        .clone()
        .or_else(|| state.shown.as_ref().map(|shown| shown.url.clone()))
        .unwrap_or(named)
}

/// Selects the panel `args` names in an open panels page.
fn select(core: &Core, args: &Value) {
    if panels_url(args) == panels_url(&Value::Null) {
        return;
    }
    let (Some(plugin), Some(panel)) = (args["plugin"].as_str(), args["panel"].as_str()) else {
        return;
    };
    core.send(&CoreToShell::PluginPanel {
        message: crate::plugins::open::select_message(plugin, panel),
    });
}

/// A panels page shows panel `panel` of plugin `plugin` now (its `selected` request): the page
/// the panels move with when they dock, undock or follow the setting.
///
/// # Errors
///
/// An id that is not well formed; nothing is kept then.
pub fn selected(core: &Core, plugin: &str, panel: &str) -> Result<(), String> {
    let url = panels_url(&json!({ "plugin": plugin, "panel": panel }));
    if url == panels_url(&Value::Null) {
        return Err("a panel is named by a plugin id and a panel id".to_owned());
    }
    lock(&state(core)).selected = Some(url);
    Ok(())
}

/// `panels.dock.<side>`: the panels docked at `side` - at `args.size` (kept for the side), one
/// `args.step` larger or smaller than shown, or the side's kept size - the floating window
/// gone, `panels.dock` set to the side; folded as before (hidden panels open), or as
/// `args.collapsed` says; at the panel `args` names.
///
/// # Errors
///
/// A bad size, step or collapsed; nothing changes then.
pub fn dock(core: &Core, side: PanelSide, args: &Value) -> Result<Value, CommandError> {
    let wanted = size::wanted(args)?;
    // Another side keeps the fold (review point 6); `collapsed` sets it.
    let collapsed = collapse::arg(args)?
        .unwrap_or(core.user_settings().panels.dock_state == PanelDockState::Rail);
    let url = url_for(core, args);
    let mut user = core.user_settings();
    user.panels.dock = side.setting();
    user.panels.dock_state = rail::state_of(collapsed);
    let stored = side.size_in(&user.panels.dock_size);
    let size = match wanted {
        size::Wanted::Kept => None,
        size::Wanted::Size(size) => Some(size),
        size::Wanted::Step(direction) => {
            let state = state(core);
            let state = lock(&state);
            let base = state.shown_size(side, stored);
            Some(size::stepped(base, direction, state.ceiling(side)))
        }
    };
    if let Some(size) = size {
        side.set_in(&mut user.panels.dock_size, size);
    }
    let size = side.size_in(&user.panels.dock_size);
    store::keep(core, user);
    core.ui().hide_page_window(PageWindow::Panels);
    put(core, side, size, url, collapsed);
    select(core, args);
    Ok(json!({ "position": side.key(), "size": size, "collapsed": collapsed }))
}

/// `panels.undock`: the dock goes, the floating window opens at the panel shown (or the one
/// `args` names), `panels.dock` is `floating`.
///
/// # Errors
///
/// None so far; a command's signature.
pub fn undock(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let url = url_for(core, args);
    let mut user = core.user_settings();
    if let Some(side) = PanelSide::of(user.panels.dock) {
        lock(&state(core)).last_side = Some(side);
    }
    user.panels.dock = PanelDock::Floating;
    // The window never folds; docked again later, the panels start open (review point 3).
    user.panels.dock_state = PanelDockState::Open;
    store::keep(core, user);
    remove(core);
    open_window(core, url);
    select(core, args);
    Ok(json!({ "position": "floating" }))
}

/// Removes the dock, if one is shown.
fn remove(core: &Core) {
    let state = state(core);
    let mut state = lock(&state);
    state.drag_start = None;
    state.room = None;
    if state.shown.take().is_some() {
        drop(state);
        core.ui().dock_panels(None);
    }
}

/// The window laid the dock out (CEF side, UI thread): where a drag or a step starts. The
/// folded strip is no size of the dock.
pub fn on_room(core: &Core, room: Room) {
    let state = state(core);
    let mut state = lock(&state);
    if !state.shown.as_ref().is_some_and(|shown| shown.collapsed) {
        state.room = Some(room);
    }
}

/// The bar between the views and the dock changed its title (CEF side, UI thread): a move lays
/// the dock out live, the end of a drag becomes the side's `panels.dockSize`. A drag starts
/// from the size shown and keeps no more than the window can show. Nothing happens while no
/// dock is shown.
pub fn on_title(core: &Core, title: &str) {
    let Some(drag) = parse_title(title) else {
        return;
    };
    let state = state(core);
    let mut inner = lock(&state);
    // A folded dock has no bar; a title still on its way changes nothing.
    let Some(shown) = inner.shown.clone().filter(|shown| !shown.collapsed) else {
        inner.drag_start = None;
        return;
    };
    let side = shown.side;
    let stored = side.size_in(&core.user_settings().panels.dock_size);
    let shown_size = inner.shown_size(side, stored);
    let start = *inner.drag_start.get_or_insert(shown_size);
    let ceiling = inner.ceiling(side);
    let (delta, end) = match drag {
        Drag::Move(delta) => (delta, false),
        Drag::End(delta) => (delta, true),
    };
    if end {
        inner.drag_start = None;
    }
    drop(inner);
    let size = size::dragged(side, start, delta).min(ceiling);
    if end {
        let mut user = core.user_settings();
        side.set_in(&mut user.panels.dock_size, size);
        store::keep(core, user);
    }
    put(core, side, size, shown.url, false);
}

#[cfg(test)]
#[path = "mod_tests.rs"]
pub(crate) mod tests;

//! The page windows beside the main one (batch-2 review of RB-060-01 and RB-060-11): the
//! plugins' panels (RB-060-12) in a top-level window of its own - the findings with them since
//! RB-080-69 (`crate::panel_dock::core_panels`), no window of their own any more - the
//! command palette in a frameless popup anchored below the toolbar (ADR 0001: dropdowns as native
//! CEF popups) and the message history in one standing on the status bar (RB-080-15). All load a
//! route of the shell's pages (`settings.html#/<route>`), may query the core like the shell and
//! never cover or resize the grid's layout: the shell view keeps its 44 px.
//!
//! The CEF side is `cef_app/page_windows.rs`; this module holds what is portable: which window
//! loads which route and where the palette goes.

use crate::shell::page_url;

/// The route of the command palette (RB-060-11).
pub const ROUTE_PALETTE: &str = "palette";
/// The route of the plugin panels window (RB-060-12).
pub const ROUTE_PANELS: &str = "panels";
/// The route of the status bar's message history (RB-080-15).
pub const ROUTE_STATUS_HISTORY: &str = "status-history";
/// The message history's size in DIP when the window has room for it.
pub const HISTORY_SIZE: (i32, i32) = (560, 360);
/// The plugin panels window's first size in DIP: a side bar's height, room for a panel.
pub const PANELS_SIZE: (i32, i32) = (720, 760);
/// The palette's size in DIP when the window has room for it.
pub const PALETTE_SIZE: (i32, i32) = (640, 420);
/// The palette's smallest size in DIP.
pub const MIN_SIZE: (i32, i32) = (240, 160);
/// The space the palette keeps to the window's edges in DIP.
pub const MARGIN: i32 = 16;

/// A page window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PageWindow {
    /// The command palette (`app.command_palette`).
    Palette,
    /// The plugins' panels (`plugins.panels`, RB-060-12).
    Panels,
    /// The status bar's message history (`status.history`, RB-080-15).
    History,
}

impl PageWindow {
    /// Every page window.
    pub const ALL: [Self; 3] = [Self::Palette, Self::Panels, Self::History];

    /// The route it loads.
    #[must_use]
    pub fn route(self) -> &'static str {
        match self {
            Self::Palette => ROUTE_PALETTE,
            Self::Panels => ROUTE_PANELS,
            Self::History => ROUTE_STATUS_HISTORY,
        }
    }

    /// The URL it loads.
    #[must_use]
    pub fn url(self) -> String {
        page_url(self.route())
    }

    /// The i18n key of its title.
    #[must_use]
    pub fn title_key(self) -> &'static str {
        match self {
            Self::Palette => "palette.window.title",
            Self::Panels => "panels.window.title",
            Self::History => "status.window.title",
        }
    }

    /// Whether the window can be resized, maximised and minimised: every page window but the
    /// palette popup, which keeps its place under the toolbar.
    #[must_use]
    pub fn resizable(self) -> bool {
        !self.is_popup()
    }

    /// Whether it is a frameless popup of the main window that hides when it loses the focus.
    #[must_use]
    pub fn is_popup(self) -> bool {
        matches!(self, Self::Palette | Self::History)
    }
}

/// A rectangle in screen DIP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenRect {
    /// Left.
    pub x: i32,
    /// Top.
    pub y: i32,
    /// Width.
    pub width: i32,
    /// Height.
    pub height: i32,
}

/// Where the palette goes for the main window's client area `client`: right below the toolbar
/// (`shell_height`), centred, [`PALETTE_SIZE`] or less so it stays inside the window, never
/// smaller than [`MIN_SIZE`].
#[must_use]
pub fn palette_bounds(client: ScreenRect, shell_height: u32) -> ScreenRect {
    let shell = i32::try_from(shell_height).unwrap_or(i32::MAX);
    let width = PALETTE_SIZE
        .0
        .min(client.width - 2 * MARGIN)
        .max(MIN_SIZE.0);
    let height = PALETTE_SIZE
        .1
        .min(client.height - shell - MARGIN)
        .max(MIN_SIZE.1);
    ScreenRect {
        x: client.x + (client.width - width) / 2,
        y: client.y.saturating_add(shell),
        width,
        height,
    }
}

/// Where the message history goes for the main window's client area `client`: standing on the
/// status bar (`bar_height`, 0 while it is hidden) at the right edge, where the bar's history
/// button is; [`HISTORY_SIZE`] or less so it never covers the toolbar (`shell_height`), never
/// smaller than [`MIN_SIZE`].
#[must_use]
pub fn history_bounds(client: ScreenRect, shell_height: u32, bar_height: u32) -> ScreenRect {
    let shell = i32::try_from(shell_height).unwrap_or(i32::MAX);
    let bar = i32::try_from(bar_height).unwrap_or(i32::MAX);
    let width = HISTORY_SIZE
        .0
        .min(client.width - 2 * MARGIN)
        .max(MIN_SIZE.0);
    let height = HISTORY_SIZE
        .1
        .min(client.height - shell - bar - MARGIN)
        .max(MIN_SIZE.1);
    let bottom = client.y.saturating_add(client.height).saturating_sub(bar);
    ScreenRect {
        x: client.x + client.width - width - MARGIN,
        y: bottom - height,
        width,
        height,
    }
}

/// Which window (or docked page) of each kind is the current one (RB-080-57). A window closes
/// asynchronously: [`Generations::close`] lets go of it at once, so an open right after makes a
/// new one, and the old one's late end ([`Generations::destroyed`]) spares its successor. A
/// browser made for a generation no longer current is an orphan and gets no rights.
#[derive(Debug)]
pub struct Generations<K> {
    next: u64,
    live: std::collections::HashMap<K, u64>,
}

impl<K> Default for Generations<K> {
    fn default() -> Self {
        Self {
            next: 0,
            live: std::collections::HashMap::new(),
        }
    }
}

impl<K: Copy + Eq + std::hash::Hash> Generations<K> {
    /// A new window of `kind` is made: its generation, the current one.
    pub fn open(&mut self, kind: K) -> u64 {
        self.next += 1;
        self.live.insert(kind, self.next);
        self.next
    }

    /// The window of `kind` starts to close: none is current; whether one was.
    pub fn close(&mut self, kind: K) -> bool {
        self.live.remove(&kind).is_some()
    }

    /// Whether `generation` is the current window of `kind`.
    #[must_use]
    pub fn is_current(&self, kind: K, generation: u64) -> bool {
        self.live.get(&kind) == Some(&generation)
    }

    /// The window of `generation` is gone: whether it was the current one (then none is).
    pub fn destroyed(&mut self, kind: K, generation: u64) -> bool {
        let current = self.is_current(kind, generation);
        if current {
            self.live.remove(&kind);
        }
        current
    }
}

#[cfg(test)]
#[path = "page_window_tests.rs"]
mod tests;

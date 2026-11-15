//! What the host asks of the window: the CEF side implements [`UiBackend`] by posting each
//! call to the CEF UI thread, so every method returns at once and never blocks.
//!
//! Wave 0.1-w1b added the window work of its jobs (contract 0.1-w1b): the DevTools dock
//! (RB-010-06), the settings window and the file dialog (RB-010-08), the environment frame
//! (RB-010-12) and bringing the window to the front (RB-010-11). The CEF side of each lives in
//! the job's own file under `cef_app/`.

use std::path::PathBuf;

use rb_engine::external::{NativeWindow, WindowRect};
use rb_engine::{CellRect, ViewId};
use rb_ipc::{MapRect, PathKind};

use crate::layout_plan::PlanNode;
pub use crate::page_window::{PageWindow, ScreenRect};
pub use crate::sessions::SessionBadge;
pub use crate::view_footer::CellFooter;

/// Where docked DevTools sit beside the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockSide {
    /// Below the grid.
    Bottom,
    /// Right of the grid.
    Right,
}

/// Docked DevTools: the hosted frontend for one view (RB-010-06).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DevtoolsDock {
    /// The inspected view.
    pub view: ViewId,
    /// Below or beside the grid.
    pub side: DockSide,
    /// The dock's height (bottom) or width (right) in DIP.
    pub size: u32,
    /// The frontend URL (`devtools://devtools/bundled/inspector.html?ws=...`, the internal
    /// one-time endpoint); never logged, it carries a token.
    pub frontend_url: String,
}

/// A native file or folder dialog (RB-010-08).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathPick {
    /// File or folder.
    pub kind: PathKind,
    /// The dialog's title, already translated.
    pub title: String,
    /// A Save dialog that proposes this file (RB-080-73); `None` opens an existing file or
    /// folder.
    pub save_as: Option<PathBuf>,
}

/// What a [`UiBackend::pick_path`] calls with the chosen path, `None` when cancelled.
pub type PathPicked = Box<dyn FnOnce(Option<PathBuf>) + Send>;

/// The short note in a cell's label strip after a breakpoint shortcut (RB-010-16).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellOverlay {
    /// The text (`Tablet - 768 x 1024 @2x`), already translated.
    pub text: String,
    /// How long it stays before the label returns, in ms.
    pub ms: u32,
}

/// What [`UiBackend::clear_session_cookies`] calls with: `true` once the cookies are gone.
pub type CookiesCleared = Box<dyn FnOnce(bool) + Send>;

/// An external view's window to show in its grid cell (RB-080-39, `cef_app/embed.rs`): the
/// window finds the view's window among `window`'s process's, gives it the main window as its
/// owner, takes its frame and keeps it over the cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbedRequest {
    /// The view whose cell the window covers.
    pub view: ViewId,
    /// How the engine names the window.
    pub window: NativeWindow,
    /// Where the engine put the window (the cascade), to tell it from the engine's other
    /// windows; `None` when the engine could not place it.
    pub expected: Option<WindowRect>,
}

/// The window's part of the views.
pub trait UiBackend: Send + Sync {
    /// Builds the grid: `grid.plan` arranges `grid.cells` (by `layout.cells` index). A view in
    /// `cells` that exists keeps its browser (re-parented, not reloaded); a new one is created
    /// loading its `initial_url`; an existing view not in `cells` is closed.
    fn apply_grid(&self, grid: UiGrid);
    /// Loads `url` in the view's main frame.
    fn navigate(&self, view: ViewId, url: &str);
    /// Reloads the view.
    fn reload(&self, view: ViewId, ignore_cache: bool);
    /// History back.
    fn go_back(&self, view: ViewId);
    /// History forward.
    fn go_forward(&self, view: ViewId);
    /// Gives the view keyboard focus.
    fn focus(&self, view: ViewId);
    /// Tells the browser whether the view can be seen; a hidden view is throttled.
    fn set_hidden(&self, view: ViewId, hidden: bool);
    /// Opens the native DevTools window of the view.
    fn show_devtools(&self, view: ViewId);
    /// Runs `script` in the shell view's main frame (core -> shell events).
    fn send_to_shell(&self, script: String);
    /// Closes the main window, which ends the app.
    fn close_window(&self);
    /// Shows docked DevTools as `dock` says, or removes the dock with `None`; the grid area
    /// shrinks or grows (RB-010-06).
    fn dock_devtools(&self, dock: Option<DevtoolsDock>);
    /// Closes the native DevTools window of the view (RB-010-06).
    fn close_devtools(&self, view: ViewId);
    /// Opens the settings window at `url` (the shell's settings page), or brings it to the
    /// front and loads `url` there (RB-010-08).
    fn open_settings_window(&self, url: String);
    /// Shows a native file or folder dialog; `done` gets the choice (RB-010-08).
    fn pick_path(&self, pick: PathPick, done: PathPicked);
    /// Frames the grid's views in `color` (`#rrggbb`, the environment's), or not at all
    /// (RB-010-12).
    fn set_frame_color(&self, color: Option<String>);
    /// Brings the main window to the front and focuses it (a second call handed over its URL,
    /// RB-010-11).
    fn activate_window(&self);
    /// Gives the keyboard focus to the shell view, so a bar in the toolbar strip (the session
    /// bar, RB-020-07) takes the typing. A window without a shell does nothing.
    fn focus_shell(&self) {}
    /// Opens `window` at `url`, or shows it, brings it to the front and focuses it when it is
    /// open (`crate::page_window`: the findings window, the palette's popup below the toolbar).
    /// A window that cannot does nothing.
    fn open_page_window(&self, window: PageWindow, url: String) {
        let _ = (window, url);
    }
    /// Shows the plugin panels docked in the main window as `dock` says, or removes the dock
    /// with `None`; the views make room as for docked DevTools (RB-080-57,
    /// `cef_app/panel_dock.rs`). A window that cannot does nothing.
    fn dock_panels(&self, dock: Option<crate::panel_dock::DockedPanels>) {
        let _ = dock;
    }
    /// Hides `window` and gives the main window the focus back (the palette ran or was left
    /// with Escape). A window that cannot does nothing.
    fn hide_page_window(&self, window: PageWindow) {
        let _ = window;
    }
    /// `view` is the active view now: its label strip is marked (`ui.highlightActiveView`,
    /// [`UiBackend::set_active_highlight`]). A window without label strips does nothing.
    fn mark_active(&self, view: Option<ViewId>) {
        let _ = view;
    }
    /// Whether the active view's label strip is marked at all (`ui.highlightActiveView`).
    fn set_active_highlight(&self, enabled: bool) {
        let _ = enabled;
    }
    /// rBrowser's own theme (`ui.theme`, RB-080-95): every window, its native parts and its
    /// title bar take it at once - never the pages in the views. A window without native parts
    /// does nothing.
    fn set_app_theme(&self, theme: rb_config::Theme) {
        let _ = theme;
    }
    /// Shows or hides the status bar at the bottom of the window; the grid takes its room
    /// (RB-080-15, `cef_app/status_bar.rs`). A window without one does nothing.
    fn set_status_bar_visible(&self, visible: bool) {
        let _ = visible;
    }
    /// The shell view's height changed (`crate::toolbar_rows::height`, RB-080-52): the window
    /// lays its box out again. A window without a shell does nothing.
    fn relayout_shell(&self) {}
    /// Runs `script` (a `statusMessage` event) in the status bar and the page windows, where its
    /// history is (RB-080-15); the toolbar and the cell labels never get it. A window without a
    /// bar does nothing.
    fn send_to_status_bar(&self, script: String) {
        let _ = script;
    }
    /// Writes `label` into the label strip of `view`; with `overlay`, its text first for
    /// `overlay.ms` (RB-010-16, `cef_app/cell_labels.rs`).
    fn set_cell_label(&self, view: ViewId, label: String, overlay: Option<CellOverlay>);
    /// Writes `footer` into the footer of `view` - in its cell or its own window - or hides the
    /// strip with `None` (RB-080-44, `cef_app/cell_footers.rs`). A window without one does nothing.
    fn set_cell_footer(&self, view: ViewId, footer: Option<CellFooter>) {
        let _ = (view, footer);
    }
    /// Opens the size quick pick of `view` under `label`, the strip the canvas board draws at
    /// that rectangle from the grid area's top-left corner (RB-080-22,
    /// `cef_app/canvas_board.rs`). A window without a board does nothing.
    fn show_size_pick(&self, view: ViewId, label: MapRect) {
        let _ = (view, label);
    }
    /// Opens the engine menu of `view` (RB-080-33, `cef_app/engine_badge.rs`): under the badge
    /// of its grid strip, or - with `label` - under the right end of the strip the canvas board
    /// draws at that rectangle. A window without label strips does nothing.
    fn show_engine_pick(&self, view: ViewId, label: Option<MapRect>) {
        let _ = (view, label);
    }
    /// Shows a native view or add menu in front of the views (RB-080-46, `layout::view_menu`).
    fn show_menu(&self, menu: crate::layout::view_menu::MenuRequest) {
        let _ = menu;
    }
    /// Whether every label strip ends in the engine badge (`ui.engineBadge`, RB-080-33).
    fn set_engine_badges(&self, shown: bool) {
        let _ = shown;
    }
    /// Sets the page zoom of `view`'s browser to Chromium's `level` (0 is 100 %, RB-090-20,
    /// `cef_app/page_zoom.rs`); the zoom badge of its label strip follows. A view without a
    /// browser does nothing.
    fn set_page_zoom(&self, view: ViewId, level: f64) {
        let _ = (view, level);
    }
    /// Moves the placed views of a layout mode to `placed` (scroll, pan, a new grid area;
    /// RB-080-04) without rebuilding them; `area` is the grid area they were computed for, which
    /// the window cuts them to (RB-080-55).
    fn place_views(&self, area: CellRect, placed: Vec<PlacedView>);
    /// Moves `window.view` out of its cell into a window of its own, its browser kept
    /// (RB-080-14, `cef_app/pop_out.rs`). The grid built next shows its cell as
    /// [`UiGrid::detached`] says. A window that cannot does nothing.
    fn pop_out(&self, window: PopOutWindow) {
        let _ = window;
    }
    /// Takes `view` out of its window and closes the window; the grid built next puts the view
    /// back into its cell. A view without a window does nothing.
    fn pop_in(&self, view: ViewId) {
        let _ = view;
    }
    /// Brings the window of `view` to the front and focuses it.
    fn raise_pop_out(&self, view: ViewId) {
        let _ = view;
    }
    /// Shows the external view's window in its cell (RB-080-39): the window answers with
    /// `HostEngines::embedded_bound` once it holds the window, or `embed_failed` with the
    /// reason. A window that cannot does nothing, so the view stays a window of its own.
    fn embed_external(&self, request: EmbedRequest) {
        let _ = request;
    }
    /// Gives the window of `view` back as a window of its own, at `rect` (the placement) or
    /// where it is. A window that holds no such window does nothing.
    fn release_external(&self, view: ViewId, rect: Option<WindowRect>) {
        let _ = (view, rect);
    }

    /// Deletes every cookie of `session`'s request context (RB-020-07,
    /// `sessions::cef::clear_cookies`) and calls `done`. A window that cannot answers `false`.
    fn clear_session_cookies(&self, session: String, done: CookiesCleared) {
        tracing::warn!(%session, "this window cannot clear a session's cookies");
        done(false);
    }
}

/// The grid the window builds.
#[derive(Clone, Debug, PartialEq)]
pub struct UiGrid {
    /// The nested boxes.
    pub plan: PlanNode,
    /// The gap in DIP.
    pub gap: u32,
    /// The label strip above each view in DIP.
    pub label_height: u32,
    /// One line of the footer under each view in DIP (`ui.viewFooter`, RB-080-44); 0 without
    /// footers. A footer of two lines (RB-080-53) is twice it: `CellFooter::height`.
    pub footer_height: u32,
    /// The cells, by `layout.cells` index.
    pub cells: Vec<UiCell>,
    /// A layout mode (RB-080-04): each cell's view is placed at its rectangle as an overlay
    /// instead of in the plan's boxes; `None` for grid and focus.
    pub placed: Option<Vec<PlacedView>>,
    /// The grid area `placed` was computed for; the window cuts the overlays to it (RB-080-55).
    pub area: Option<CellRect>,
    /// The views in a window of their own (RB-080-14): the grid leaves them there and shows a
    /// placeholder in their cell, or hides the cell.
    pub detached: Vec<DetachedCell>,
}

impl UiGrid {
    /// How `view`'s cell shows a view in a window of its own; `None` for a view in the grid.
    pub fn detached(&self, view: ViewId) -> Option<DetachedCell> {
        self.detached.iter().copied().find(|cell| cell.view == view)
    }
}

/// The cell of a view in a window of its own (RB-080-14).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DetachedCell {
    /// The view.
    pub view: ViewId,
    /// A placeholder in its cell (a click brings the window to the front); else the cell is
    /// hidden and the other views close the gap.
    pub placeholder: bool,
}

/// The window of a view popped out of the grid (RB-080-14): the breakpoint at its real size.
#[derive(Clone, Debug, PartialEq)]
pub struct PopOutWindow {
    /// The view; it keeps its id, browser, session and sync.
    pub view: ViewId,
    /// The view's label (the breakpoint's), for the title.
    pub label: String,
    /// The emulated viewport width in CSS px: the client width at zoom 1.
    pub width: u32,
    /// The emulated viewport height in CSS px.
    pub height: u32,
    /// The client area the breakpoint's window had last time (screen DIP), if kept.
    pub remembered: Option<ScreenRect>,
    /// `popOut.fitToScreen`.
    pub fit_to_screen: bool,
    /// The footer at the window's bottom (RB-080-44); `None` without footers.
    pub footer: Option<CellFooter>,
}

/// Where a layout mode puts one view (RB-080-04): its label strip and view together, in DIP of
/// the client area; wholly inside the grid area when `visible`, in the canvas partly - the window
/// cuts it to the area (RB-080-55).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedView {
    /// The view.
    pub view: ViewId,
    /// The label strip and the view below it.
    pub outer: CellRect,
    /// Shown; a hidden view keeps its browser.
    pub visible: bool,
}

/// One cell of the grid.
#[derive(Clone, Debug, PartialEq)]
pub struct UiCell {
    /// The view shown in the cell.
    pub view: ViewId,
    /// The text of the cell's label strip.
    pub label: String,
    /// The URL a new view starts with (the marker that identifies its CDP target).
    pub initial_url: String,
    /// The session (cookie and storage context) a new view is created in (RB-020-07).
    pub session: String,
    /// The session's badge in the label strip; `None` for `default`.
    pub badge: Option<SessionBadge>,
    /// The footer under the view (RB-080-44); `None` without footers.
    pub footer: Option<CellFooter>,
}

#[cfg(test)]
#[path = "ui_recording.rs"]
pub(crate) mod recording;

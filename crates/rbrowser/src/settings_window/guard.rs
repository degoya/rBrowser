//! Who may query the core (`cefQuery`, RB-010-07/RB-010-08): the shell view, the pages window,
//! the page windows (findings, palette, message history; `crate::page_window`), the status bar
//! (RB-080-15) and the canvas board (RB-080-04), each only from its main frame and only on a page
//! of `rbrowser://ui/`. A grid view, a popup, a sub-frame or a page that navigated away is
//! refused, whatever it sends. The board may send only what it needs ([`board_may_send`]): the
//! state, the theme, the text size, its drags and its menus' places. The CEF side (`cef_app/queries.rs`) asks [`request_allowed`]
//! with the browser ids it knows.

use rb_ipc::{ShellToCore, parse_request};

use crate::shell::is_shell_url;

/// The browser a query came from, as CEF reports it.
#[derive(Clone, Copy, Debug)]
pub struct QueryOrigin<'a> {
    /// The browser's id; 0 when unknown.
    pub browser: i32,
    /// Whether the frame is the browser's main frame.
    pub main_frame: bool,
    /// The frame's URL.
    pub url: &'a str,
}

/// The browsers allowed to query: the shell view's, the pages window's and the page windows'
/// ids (`crate::page_window`), 0 while a browser does not exist.
#[derive(Clone, Copy, Debug, Default)]
pub struct KnownBrowsers {
    /// The shell view.
    pub shell: i32,
    /// The pages window (settings, trust, the analyser's wizard).
    pub pages: i32,
    /// The command palette's popup (RB-060-11).
    pub palette: i32,
    /// The plugin panels window (RB-060-12); its panels' frames are sub-frames, never allowed.
    pub panels: i32,
    /// The same panels page docked in the main window (RB-080-57), with the window's rights.
    pub panels_dock: i32,
    /// The status bar's message history (RB-080-15).
    pub history: i32,
    /// The status bar at the bottom of the main window (RB-080-15).
    pub status_bar: i32,
    /// The canvas board behind the canvas's views (RB-080-04); narrowed by [`board_may_send`].
    pub board: i32,
}

/// Whether `origin` may query the core: a known browser (never id 0), its main frame, a page of
/// the shell's origin.
pub fn query_allowed(origin: QueryOrigin<'_>, known: KnownBrowsers) -> bool {
    let ids = [
        known.shell,
        known.pages,
        known.palette,
        known.panels,
        known.panels_dock,
        known.history,
        known.status_bar,
        known.board,
    ];
    let known_browser = origin.browser != 0 && ids.contains(&origin.browser);
    known_browser && origin.main_frame && is_shell_url(origin.url)
}

/// Whether `origin` may send `request`: [`query_allowed`], and from the canvas board only what
/// [`board_may_send`] lets through.
pub fn request_allowed(origin: QueryOrigin<'_>, known: KnownBrowsers, request: &str) -> bool {
    query_allowed(origin, known) && (origin.browser != known.board || board_may_send(request))
}

/// What the canvas board may send: the state, the settings without their schema (the theme),
/// the text size (RB-090-26), the active view (a click on a label), its size quick pick (a click on the active view's
/// label, RB-080-22), its engine menu, its drags, its wheel zoom, the canvas it painted (RB-090-55) and where a right-click asks for a menu
/// (RB-080-46: the core shows the menu and runs its rows, so the board runs no command at
/// all); nothing that runs a command, writes a setting or navigates.
pub fn board_may_send(request: &str) -> bool {
    matches!(
        parse_request(request),
        Ok(ShellToCore::GetState
            | ShellToCore::GetSettings {
                include_schema: false,
            }
            | ShellToCore::GetTextScale
            | ShellToCore::SetActiveView { .. }
            | ShellToCore::CanvasDrag { .. }
            | ShellToCore::CanvasPan { .. }
            | ShellToCore::CanvasStep { .. }
            | ShellToCore::CanvasSizePick { .. }
            | ShellToCore::CanvasEnginePick { .. }
            | ShellToCore::CanvasMenu { .. }
            | ShellToCore::CanvasZoom { .. }
            | ShellToCore::CanvasPainted { .. })
    )
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;

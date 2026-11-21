//! Who may query the core (`cefQuery`, RB-010-07/RB-010-08): the shell view, the pages window,
//! the page windows (findings, palette, message history; `crate::page_window`), the status bar
//! (RB-080-15) and the canvas board (RB-080-04), each only from its main frame and only on a page
//! of `rbrowser://ui/`. A grid view, a popup, a sub-frame or a page that navigated away is
//! refused, whatever it sends. The board may send only what it needs ([`board_may_send`]): the
//! state, the theme, the text size, its drags and its menus' places; the sitemap gallery window
//! its run and links ([`gallery_may_send`], RB-090-86). The CEF side (`cef_app/queries.rs`)
//! asks [`request_allowed`] with the browser ids it knows.

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
    /// The sitemap gallery window (RB-090-86); narrowed by [`gallery_may_send`].
    pub gallery: i32,
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
        known.gallery,
    ];
    let known_browser = origin.browser != 0 && ids.contains(&origin.browser);
    known_browser && origin.main_frame && is_shell_url(origin.url)
}

/// Whether `origin` may send `request`: [`query_allowed`], from the canvas board only what
/// [`board_may_send`] lets through, and the catalogue's requests from the pages window only.
pub fn request_allowed(origin: QueryOrigin<'_>, known: KnownBrowsers, request: &str) -> bool {
    query_allowed(origin, known)
        && (origin.browser != known.board || board_may_send(request))
        && (origin.browser != known.gallery || gallery_may_send(request))
        && (origin.browser == known.pages || !is_catalogue_request(request))
        // The setup wizard's messages write project configs: the pages window's only (RB-091-05).
        && (origin.browser == known.pages || !crate::first_run::is_setup_request(request))
        // Save or Cancel of a bug report's preview: the person's in the pages window (RB-093-04).
        && (origin.browser == known.pages
            || !crate::bug_report::preview::is_decision_request(request))
        // The MCP setup writes client files: the pages window's only (RB-091-09).
        && (origin.browser == known.pages || !crate::mcp::setup::is_request(request))
}

/// Whether `request` is one of the plugin catalogue's (`pluginPanel` with an `op` starting
/// `catalog`, RB-091-01, or the first-run wizard's `available`, `installChosen` and
/// `removeChosen`, RB-091-04): it downloads, installs or removes plugins, so only the pages
/// window - Settings > Plugins > Catalog and the wizard - may send it (security review L3), never
/// the panels window or the palette.
pub fn is_catalogue_request(request: &str) -> bool {
    match parse_request(request) {
        Ok(ShellToCore::PluginPanel { request }) => request
            .get("op")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|op| {
                op.starts_with("catalog")
                    || matches!(
                        op,
                        "available" | "installChosen" | "removeChosen" | "applyRecommended"
                    )
            }),
        _ => false,
    }
}

/// What the canvas board may send: the state, the settings without their schema (the theme),
/// the text size (RB-090-26), the active view (a click on a label), its size quick pick (a click on the active view's
/// label, RB-080-22), its engine menu, a label piece's click (RB-090-69, the core looks up what it
/// does), its drags, its wheel zoom, the canvas it painted (RB-090-55) and where a right-click asks for a menu
/// (RB-080-46: the core shows the menu and runs its rows, so the board runs no command at
/// all); nothing that writes a setting, and no command but `view.edit_address` - the address
/// field of a decoupled view's label (RB-092-03), which loads only an http, https or file
/// address, only in that view and only while its navigation is decoupled
/// ([`crate::cell_info::address`]); its arguments only in the field's shape
/// ([`edit_address_args`]).
pub fn board_may_send(request: &str) -> bool {
    let request = parse_request(request);
    if let Ok(ShellToCore::RunCommand { id, args }) = &request {
        return (id == crate::cell_info::address::EDIT_ADDRESS && edit_address_args(args))
            || (id == PAGE_ZOOM_RESET && page_zoom_reset_args(args));
    }
    matches!(
        request,
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
            | ShellToCore::CanvasPiece { .. }
            | ShellToCore::CanvasMenu { .. }
            | ShellToCore::CanvasZoom { .. }
            | ShellToCore::CanvasPainted { .. })
    )
}

/// The command the canvas zoom badge's click sends (RB-090-28).
const PAGE_ZOOM_RESET: &str = "view.page_zoom.reset";

/// The board's `view.page_zoom.reset` arguments: `{ target }`, one view id, nothing else.
fn page_zoom_reset_args(args: &serde_json::Value) -> bool {
    args.as_object().is_some_and(|args| {
        args.len() == 1
            && args
                .get("target")
                .and_then(serde_json::Value::as_u64)
                .is_some_and(|view| u32::try_from(view).is_ok())
    })
}

/// The board's `view.edit_address` arguments: an object with `view`, a view id (a number that
/// fits `u32`), and optionally `url`, the typed address as a string of at most
/// [`crate::open_link::address::MAX_URL_CHARS`] characters; no other key.
fn edit_address_args(args: &serde_json::Value) -> bool {
    use serde_json::Value;
    let Some(args) = args.as_object() else {
        return false;
    };
    let view = args
        .get("view")
        .and_then(Value::as_u64)
        .is_some_and(|view| u32::try_from(view).is_ok());
    let url = match args.get("url") {
        None => true,
        Some(Value::String(url)) => url.chars().count() <= crate::open_link::address::MAX_URL_CHARS,
        Some(_) => false,
    };
    view && url && args.keys().all(|key| key == "view" || key == "url")
}

/// What the sitemap gallery window may send (RB-090-86): the state, the settings without their
/// schema (the theme), the text size, its run, and `app.open_link` for a page's address (http
/// and https only, within the command's budget); nothing that writes, picks a file or runs any
/// other command.
pub fn gallery_may_send(request: &str) -> bool {
    match parse_request(request) {
        Ok(
            ShellToCore::GetState
            | ShellToCore::GetSettings {
                include_schema: false,
            }
            | ShellToCore::GetTextScale
            | ShellToCore::GetSitemapGallery(_),
        ) => true,
        Ok(ShellToCore::RunCommand { id, .. }) => id == crate::open_link::OPEN_LINK_COMMAND,
        _ => false,
    }
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;

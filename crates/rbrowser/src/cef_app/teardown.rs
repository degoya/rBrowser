//! What the UI thread holds of CEF, let go when the main window is destroyed (RB-080-91).
//!
//! The owner's crash dumps of builds `f68e917` and before (4 of them, `diag8`, `diag9`) are one
//! crash: an access violation in `libcef.dll` at the end of the process, from the thread-local
//! map of the layout modes' overlays (`placed.rs`, `OVERLAYS`). The CRT ran that thread's
//! destructors after `CefShutdown` (`__acrt_freeptd` -> `thread_local::destructors` -> drop of
//! the map -> `cef_base_ref_counted_t::release` into a CEF that was gone). It only crashed after
//! a session in a layout mode - the grid leaves the map empty - which is why 3 of 28 ends of the
//! owner's log crashed. Every UI-thread holder of a CEF object is therefore emptied here, while
//! CEF still runs; the repository lint `cef_release` holds each `thread_local!` of `cef_app` to
//! this list or to its reason for staying.
//!
//! The order of the end (`super::run`): the window closes, the host attaches no view again
//! (`Host::begin_shutdown`), the window is destroyed and this runs, the message loop ends, the
//! watchers, MCP, the CDP proxy and the portal stop, the external engines and the sidecar end,
//! and `CefShutdown` comes last. UI thread.

use super::views::UI;

/// Empties every UI-thread holder of a CEF object.
pub fn release() {
    // The layout modes' overlays: the crash of the dumps.
    super::placed::clear();
    super::cell_footers::release();
    super::cell_labels::release();
    super::engine_badge::clear();
    super::page_zoom::release();
    super::cell_info::release();
    super::canvas_board::clear();
    super::status_bar::release();
    super::frame::release();
    super::devtools::release();
    super::panel_dock::release();
    super::report_window::release();
    // The sitemap's render views and its gallery window (RB-090-83).
    super::render_views::release();
    super::gallery_window::release();
    // The windows that follow `ui.theme` (RB-080-95).
    super::theme::clear();
    UI.with(|ui| *ui.borrow_mut() = Default::default());
}

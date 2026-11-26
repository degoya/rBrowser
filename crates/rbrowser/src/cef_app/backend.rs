//! The window side of the host ([`UiBackend`]): every call is posted to the CEF UI thread.

use std::sync::Mutex;
use std::time::Instant;

use cef::*;
use rb_engine::{CellRect, ViewHost as _, ViewId};

use super::task::on_ui;
use super::views::UI;
use rb_engine::external::WindowRect;

use super::{
    activate, canvas_board, cell_labels, devtools, dialogs, embed, engine_badge, frame, grid,
    page_windows, placed, pop_out, settings_window,
};
use crate::host::ui::place_tickets::PlaceTickets;
use crate::host::ui::{
    CellOverlay, CookiesCleared, DevtoolsDock, EmbedRequest, PageWindow, PathPick, PathPicked,
    PlacedView, PopOutWindow, UiBackend, UiGrid,
};
use crate::shell::SHELL_URL;

/// The CEF window behind the host.
pub struct CefUi;

/// The placements posted to the UI thread and not run yet: only the newest of a view moves it
/// (RB-094-18).
static PLACE_TICKETS: Mutex<PlaceTickets> = Mutex::new(PlaceTickets::new());

/// Runs `action` with `view`'s browser on the UI thread.
fn with_browser(view: ViewId, action: impl FnOnce(&Browser) + Send + 'static) {
    on_ui(move || {
        let browser = UI.with(|ui| ui.borrow().views.get(&view).and_then(BrowserView::browser));
        match browser {
            Some(browser) => action(&browser),
            None => tracing::debug!(%view, "no browser for the view (yet)"),
        }
    });
}

impl UiBackend for CefUi {
    // A loop of these is told by `crate::host::rebuild_watch` (RB-099-07).
    fn apply_grid(&self, grid: UiGrid) {
        on_ui(move || grid::apply(&grid));
    }

    // Latest wins: a placement overtaken by a newer one of its views before the UI thread ran
    // it moves those views no more (RB-094-18); the wait is logged for the pace of a gesture.
    fn place_views(&self, area: CellRect, placed: Vec<PlacedView>) {
        let ticket = rb_util::lock(&PLACE_TICKETS).post(&placed);
        let posted = Instant::now();
        on_ui(move || {
            let (fresh, skipped) = rb_util::lock(&PLACE_TICKETS).take(ticket, placed);
            tracing::debug!(
                target: crate::layout::canvas_live::PACE,
                ticket,
                waited_us = u64::try_from(posted.elapsed().as_micros()).unwrap_or(u64::MAX),
                placed = fresh.len(),
                skipped,
                "the window placed views"
            );
            if !fresh.is_empty() {
                placed::place(area, &fresh);
            }
        });
    }

    // The board's page answers each canvas it painted once it exists (RB-090-55).
    fn board_acks(&self) -> bool {
        canvas_board::browser_id() != 0
    }

    // A view in a window of its own (RB-080-14).

    fn pop_out(&self, window: PopOutWindow) {
        on_ui(move || pop_out::open(window));
    }

    fn pop_in(&self, view: ViewId) {
        on_ui(move || pop_out::close(view));
    }

    fn raise_pop_out(&self, view: ViewId) {
        on_ui(move || pop_out::raise(view));
    }

    // An external view's window in its cell (RB-080-39).

    fn embed_external(&self, request: EmbedRequest) {
        on_ui(move || embed::embed(request));
    }

    fn release_external(&self, view: ViewId, rect: Option<WindowRect>) {
        on_ui(move || embed::release(view, rect));
    }

    fn navigate(&self, view: ViewId, url: &str) {
        let url = url.to_owned();
        with_browser(view, move |browser| {
            if let Some(frame) = browser.main_frame() {
                frame.load_url(Some(&CefString::from(url.as_str())));
            }
        });
    }

    fn reload(&self, view: ViewId, ignore_cache: bool) {
        with_browser(view, move |browser| {
            if ignore_cache {
                browser.reload_ignore_cache();
            } else {
                browser.reload();
            }
        });
    }

    fn go_back(&self, view: ViewId) {
        with_browser(view, |browser| browser.go_back());
    }

    fn go_forward(&self, view: ViewId) {
        with_browser(view, |browser| browser.go_forward());
    }

    fn focus(&self, view: ViewId) {
        with_browser(view, move |browser| {
            // A focus queued for a view another one took the lead from since would take the
            // focus - and the lead - back from the view the person works in.
            if super::views::host().is_some_and(|host| host.active_view() != Some(view)) {
                return;
            }
            if let Some(host) = browser.host() {
                host.set_focus(1);
            }
        });
    }

    fn set_hidden(&self, view: ViewId, hidden: bool) {
        with_browser(view, move |browser| {
            if let Some(host) = browser.host() {
                host.was_hidden(i32::from(hidden));
            }
        });
    }

    fn show_devtools(&self, view: ViewId) {
        // RB-010-06: DevTools' keys, the window's remembered bounds, its close.
        with_browser(view, move |browser| devtools::show_window(view, browser));
    }

    fn send_to_shell(&self, script: String) {
        on_ui(move || {
            let frame = UI.with(|ui| {
                ui.borrow()
                    .shell
                    .as_ref()
                    .and_then(BrowserView::browser)
                    .and_then(|browser| browser.main_frame())
            });
            if let Some(frame) = frame {
                frame.execute_java_script(
                    Some(&CefString::from(script.as_str())),
                    Some(&CefString::from(SHELL_URL)),
                    0,
                );
            }
            // The settings window, the page windows, the status bar and the canvas board are
            // shell pages too (RB-010-08, RB-080-15, RB-080-04).
            settings_window::send(&script);
            page_windows::send(&script);
            super::panel_dock::send(&script);
            super::status_bar::send(&script);
            canvas_board::send(&script);
        });
    }

    fn close_window(&self) {
        on_ui(|| {
            if let Some(window) = UI.with(|ui| ui.borrow().window.clone()) {
                window.close();
            }
        });
    }

    // The window work of wave 0.1-w1b: each job's own file under `cef_app/`.

    fn dock_devtools(&self, dock: Option<DevtoolsDock>) {
        on_ui(move || devtools::dock(dock));
    }

    fn close_devtools(&self, view: ViewId) {
        on_ui(move || devtools::close(view));
    }

    fn open_settings_window(&self, url: String) {
        on_ui(move || settings_window::open(&url));
    }

    fn close_settings_window(&self) {
        on_ui(settings_window::close);
    }

    fn pick_path(&self, pick: PathPick, done: PathPicked) {
        on_ui(move || dialogs::pick_path(pick, done));
    }

    fn set_frame_color(&self, color: Option<String>) {
        on_ui(move || frame::set_color(color));
    }

    fn activate_window(&self) {
        on_ui(activate::activate);
    }

    fn focus_shell(&self) {
        on_ui(|| {
            let browser = UI.with(|ui| ui.borrow().shell.as_ref().and_then(BrowserView::browser));
            if let Some(host) = browser.and_then(|browser| browser.host()) {
                host.set_focus(1);
            }
        });
    }

    fn open_page_window(&self, window: PageWindow, url: String) {
        on_ui(move || page_windows::open(window, &url));
    }

    fn open_report(&self, report: crate::report_view::ReportOpen) {
        on_ui(move || super::report_window::open(report));
    }

    fn open_render_view(
        &self,
        view: crate::sitemap::render_views::RenderView,
        made: crate::sitemap::render_views::RenderViewMade,
    ) {
        on_ui(move || super::render_views::open(view, made));
    }

    fn resize_render_view(
        &self,
        marker: String,
        width: u32,
        height: u32,
        painted: crate::sitemap::render_views::RenderViewMade,
    ) {
        on_ui(move || super::render_views::resize(&marker, width, height, painted));
    }

    fn close_render_view(&self, marker: String) {
        on_ui(move || super::render_views::close(&marker));
    }

    fn open_gallery(&self, gallery: crate::sitemap::gallery::GalleryOpen) {
        on_ui(move || super::gallery_window::open(gallery));
    }

    fn hide_page_window(&self, window: PageWindow) {
        on_ui(move || page_windows::hide(window));
    }

    fn dock_panels(&self, dock: Option<crate::panel_dock::DockedPanels>) {
        on_ui(move || super::panel_dock::dock(dock));
    }

    fn mark_active(&self, view: Option<ViewId>) {
        on_ui(move || super::active_label::mark(view));
    }

    fn set_active_highlight(&self, enabled: bool) {
        on_ui(move || super::active_label::set_enabled(enabled));
    }

    fn set_app_theme(&self, theme: rb_config::Theme) {
        on_ui(move || super::theme::set(theme));
    }

    fn set_window_title(&self, title: String) {
        on_ui(move || super::window::set_title(&title));
    }

    fn set_status_bar_visible(&self, visible: bool) {
        on_ui(move || super::status_bar::set_visible(visible));
    }

    fn relayout_shell(&self) {
        on_ui(super::window::relayout_shell);
    }

    fn send_to_status_bar(&self, script: String) {
        on_ui(move || {
            super::status_bar::send(&script);
            page_windows::send_to(crate::page_window::PageWindow::History, &script);
        });
    }

    fn set_cell_label(&self, view: ViewId, label: String, overlay: Option<CellOverlay>) {
        on_ui(move || cell_labels::set_label(view, label, overlay));
    }

    fn set_cell_footer(&self, view: ViewId, footer: Option<crate::host::ui::CellFooter>) {
        on_ui(move || super::cell_footers::set(view, footer));
    }

    fn show_size_pick(&self, view: ViewId, label: rb_ipc::MapRect) {
        on_ui(move || canvas_board::show_size_pick(view, label));
    }

    fn show_engine_pick(&self, view: ViewId, label: Option<rb_ipc::MapRect>) {
        on_ui(move || match label {
            Some(label) => canvas_board::show_engine_pick(view, label),
            None => engine_badge::show_pick(view),
        });
    }

    fn show_menu(&self, menu: crate::layout::view_menu::MenuRequest) {
        on_ui(move || super::view_menus::show(menu));
    }

    fn show_native_menu(
        &self,
        menu: crate::layout::native_menu::NativeMenu,
        done: crate::layout::native_menu::NativeMenuDone,
    ) {
        on_ui(move || super::native_menu::show(menu, done));
    }

    fn set_page_zoom(&self, view: ViewId, level: f64) {
        on_ui(move || super::page_zoom::set_level(view, level));
    }

    fn set_engine_badges(&self, shown: bool) {
        // Sent at the start and with every change of the user settings: the mouse hook's
        // switches follow them here too, so the hook itself never reads the settings.
        super::board_pan::refresh();
        on_ui(move || engine_badge::set_shown(shown));
    }

    fn set_cell_info(&self, infos: Vec<crate::cell_info::CellInfo>) {
        on_ui(move || super::cell_info::set(infos));
    }

    fn edit_address(&self, view: ViewId, url: String, name: String) {
        on_ui(move || super::address_field::open(view, &url, &name));
    }

    fn clear_session_cookies(&self, session: String, done: CookiesCleared) {
        on_ui(move || crate::sessions::cef::clear_cookies(&session, done));
    }
}

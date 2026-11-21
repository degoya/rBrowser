//! A [`UiBackend`] that records the calls, for the tests.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;
use crate::cdp::lock;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum UiCall {
    Grid(UiGrid),
    Navigate(ViewId, String),
    Reload(ViewId, bool),
    Back(ViewId),
    Forward(ViewId),
    Focus(ViewId),
    Hidden(ViewId, bool),
    Devtools(ViewId),
    Shell(String),
    Close,
    Dock(Option<DevtoolsDock>),
    CloseDevtools(ViewId),
    SettingsWindow(String),
    CloseSettingsWindow,
    PickPath(PathPick),
    FrameColor(Option<String>),
    Activate,
    CellLabel(ViewId, String, Option<CellOverlay>),
    CellFooter(ViewId, Option<CellFooter>),
    SizePick(ViewId, MapRect),
    EnginePick(ViewId, Option<MapRect>),
    Menu(crate::layout::view_menu::MenuRequest),
    NativeMenu(crate::layout::native_menu::NativeMenu),
    EngineBadges(bool),
    CellInfo(Vec<crate::cell_info::CellInfo>),
    EditAddress(ViewId, String, String),
    ClearCookies(String),
    FocusShell,
    PageWindow(PageWindow, String),
    HidePageWindow(PageWindow),
    Place(Vec<PlacedView>),
    StatusBar(bool),
    StatusEvent(String),
    MarkActive(Option<ViewId>),
    ActiveHighlight(bool),
    PopOut(PopOutWindow),
    PopIn(ViewId),
    RaisePopOut(ViewId),
    Embed(EmbedRequest),
    Release(ViewId, Option<WindowRect>),
    PanelDock(Option<crate::panel_dock::DockedPanels>),
    AppTheme(rb_config::Theme),
    WindowTitle(String),
    PageZoom(ViewId, f64),
}

#[derive(Default)]
pub(crate) struct RecordingUi {
    pub(crate) calls: Mutex<Vec<UiCall>>,
    /// What the path dialog answers; `None`: cancelled.
    pub(crate) picked: Mutex<Option<std::path::PathBuf>>,
    /// The row a native menu answers with (RB-090-41); `None`: closed without a choice.
    pub(crate) menu_choice: Mutex<Option<String>>,
    /// Whether the window's canvas board answers what it painted (RB-090-55); off by default.
    pub(crate) board_acks: AtomicBool,
}

impl RecordingUi {
    pub(crate) fn calls(&self) -> Vec<UiCall> {
        lock(&self.calls).clone()
    }

    pub(crate) fn last_grid(&self) -> Option<UiGrid> {
        lock(&self.calls).iter().rev().find_map(|call| match call {
            UiCall::Grid(grid) => Some(grid.clone()),
            _ => None,
        })
    }

    fn push(&self, call: UiCall) {
        lock(&self.calls).push(call);
    }
}

impl UiBackend for RecordingUi {
    fn apply_grid(&self, grid: UiGrid) {
        self.push(UiCall::Grid(grid));
    }
    fn navigate(&self, view: ViewId, url: &str) {
        self.push(UiCall::Navigate(view, url.to_owned()));
    }
    fn reload(&self, view: ViewId, ignore_cache: bool) {
        self.push(UiCall::Reload(view, ignore_cache));
    }
    fn go_back(&self, view: ViewId) {
        self.push(UiCall::Back(view));
    }
    fn go_forward(&self, view: ViewId) {
        self.push(UiCall::Forward(view));
    }
    fn focus(&self, view: ViewId) {
        self.push(UiCall::Focus(view));
    }
    fn set_hidden(&self, view: ViewId, hidden: bool) {
        self.push(UiCall::Hidden(view, hidden));
    }
    fn show_devtools(&self, view: ViewId) {
        self.push(UiCall::Devtools(view));
    }
    fn send_to_shell(&self, script: String) {
        self.push(UiCall::Shell(script));
    }
    fn close_window(&self) {
        self.push(UiCall::Close);
    }
    fn dock_devtools(&self, dock: Option<DevtoolsDock>) {
        self.push(UiCall::Dock(dock));
    }
    fn close_devtools(&self, view: ViewId) {
        self.push(UiCall::CloseDevtools(view));
    }
    fn open_settings_window(&self, url: String) {
        self.push(UiCall::SettingsWindow(url));
    }
    fn close_settings_window(&self) {
        self.push(UiCall::CloseSettingsWindow);
    }
    /// Records the dialog and answers "cancelled".
    fn pick_path(&self, pick: PathPick, done: PathPicked) {
        self.push(UiCall::PickPath(pick));
        done(lock(&self.picked).clone());
    }
    fn set_frame_color(&self, color: Option<String>) {
        self.push(UiCall::FrameColor(color));
    }
    fn activate_window(&self) {
        self.push(UiCall::Activate);
    }
    fn set_cell_label(&self, view: ViewId, label: String, overlay: Option<CellOverlay>) {
        self.push(UiCall::CellLabel(view, label, overlay));
    }
    fn set_cell_footer(&self, view: ViewId, footer: Option<CellFooter>) {
        self.push(UiCall::CellFooter(view, footer));
    }
    fn show_size_pick(&self, view: ViewId, label: MapRect) {
        self.push(UiCall::SizePick(view, label));
    }
    fn show_engine_pick(&self, view: ViewId, label: Option<MapRect>) {
        self.push(UiCall::EnginePick(view, label));
    }
    fn show_menu(&self, menu: crate::layout::view_menu::MenuRequest) {
        self.push(UiCall::Menu(menu));
    }
    fn show_native_menu(
        &self,
        menu: crate::layout::native_menu::NativeMenu,
        done: crate::layout::native_menu::NativeMenuDone,
    ) {
        self.push(UiCall::NativeMenu(menu));
        done(lock(&self.menu_choice).clone());
    }
    fn set_engine_badges(&self, shown: bool) {
        self.push(UiCall::EngineBadges(shown));
    }
    fn set_cell_info(&self, infos: Vec<crate::cell_info::CellInfo>) {
        self.push(UiCall::CellInfo(infos));
    }
    fn edit_address(&self, view: ViewId, url: String, name: String) {
        self.push(UiCall::EditAddress(view, url, name));
    }
    fn set_page_zoom(&self, view: ViewId, level: f64) {
        self.push(UiCall::PageZoom(view, level));
    }
    fn place_views(&self, _area: CellRect, placed: Vec<PlacedView>) {
        self.push(UiCall::Place(placed));
    }
    fn board_acks(&self) -> bool {
        self.board_acks.load(Ordering::Relaxed)
    }
    fn pop_out(&self, window: PopOutWindow) {
        self.push(UiCall::PopOut(window));
    }
    fn pop_in(&self, view: ViewId) {
        self.push(UiCall::PopIn(view));
    }
    fn raise_pop_out(&self, view: ViewId) {
        self.push(UiCall::RaisePopOut(view));
    }
    fn embed_external(&self, request: EmbedRequest) {
        self.push(UiCall::Embed(request));
    }
    fn release_external(&self, view: ViewId, rect: Option<WindowRect>) {
        self.push(UiCall::Release(view, rect));
    }

    /// Records the session and answers "cleared".
    fn focus_shell(&self) {
        self.push(UiCall::FocusShell);
    }

    fn open_page_window(&self, window: PageWindow, url: String) {
        self.push(UiCall::PageWindow(window, url));
    }

    fn hide_page_window(&self, window: PageWindow) {
        self.push(UiCall::HidePageWindow(window));
    }
    fn dock_panels(&self, dock: Option<crate::panel_dock::DockedPanels>) {
        self.push(UiCall::PanelDock(dock));
    }

    fn set_status_bar_visible(&self, visible: bool) {
        self.push(UiCall::StatusBar(visible));
    }

    fn mark_active(&self, view: Option<ViewId>) {
        self.push(UiCall::MarkActive(view));
    }

    fn set_active_highlight(&self, enabled: bool) {
        self.push(UiCall::ActiveHighlight(enabled));
    }

    fn set_app_theme(&self, theme: rb_config::Theme) {
        self.push(UiCall::AppTheme(theme));
    }

    fn set_window_title(&self, title: String) {
        self.push(UiCall::WindowTitle(title));
    }

    fn send_to_status_bar(&self, script: String) {
        self.push(UiCall::StatusEvent(script));
    }

    fn clear_session_cookies(&self, session: String, done: CookiesCleared) {
        self.push(UiCall::ClearCookies(session));
        done(true);
    }
}

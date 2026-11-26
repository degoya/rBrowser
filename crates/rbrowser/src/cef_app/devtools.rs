//! The CEF side of DevTools (RB-010-06): the content area that holds the grid and the dock,
//! the dock (the hosted frontend in an Alloy view of its own below or beside the grid), and
//! the native DevTools window with its remembered bounds. UI thread.
//!
//! Both DevTools browsers get [`DevtoolsKeys`], so `shortcuts.disableInDevtools` applies to
//! them. The dock never leaves `devtools://` (or its marker), opens no popups and never
//! reaches the core's `cefQuery`. Its URL carries the endpoint's token: never logged.

use std::cell::RefCell;
use std::os::raw::c_int;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use cef::*;
use rb_engine::ViewId;

use super::keys::DevtoolsKeys;
use super::views::{UI, core, sized_panel};
use crate::devtools::WindowBounds;
use crate::devtools::hidden::DOCK_MARKER_URL;
use crate::devtools::layout::{DockWant, shared_extents};
use crate::host::ui::{DevtoolsDock, DockSide};

/// The dock's wanted size and side, read by its delegate (and the splitter's) at every layout.
#[derive(Default)]
pub(super) struct Extent {
    size: AtomicU32,
    pub(super) right: AtomicBool,
}

impl Extent {
    fn set(&self, side: DockSide, size: u32) {
        self.size.store(size, Ordering::Relaxed);
        self.right.store(side == DockSide::Right, Ordering::Relaxed);
    }
}

/// Whether DevTools are docked, for the docked plugin panels' size (`panel_dock.rs`,
/// RB-080-57): both docks size against one area.
static DOCKED: AtomicBool = AtomicBool::new(false);
/// The docked DevTools' side and size, as [`DOCKED`] says.
static WANT_RIGHT: AtomicBool = AtomicBool::new(false);
static WANT_SIZE: AtomicU32 = AtomicU32::new(0);

/// What docked DevTools want, `None` while they are not docked.
pub(super) fn want() -> Option<DockWant> {
    DOCKED.load(Ordering::Relaxed).then(|| DockWant {
        horizontal: WANT_RIGHT.load(Ordering::Relaxed),
        wanted: WANT_SIZE.load(Ordering::Relaxed),
        fixed: false,
    })
}

fn set_want(dock: Option<&DevtoolsDock>) {
    if let Some(dock) = dock {
        WANT_RIGHT.store(dock.side == DockSide::Right, Ordering::Relaxed);
        WANT_SIZE.store(dock.size, Ordering::Relaxed);
    }
    DOCKED.store(dock.is_some(), Ordering::Relaxed);
}

/// The content area below the shell: the grid and, when docked, the splitter and DevTools.
#[derive(Default)]
struct DockArea {
    content: Option<Panel>,
    grid: Option<View>,
    /// The bar between the grid and the dock that resizes it (`splitter.rs`).
    splitter: Option<BrowserView>,
    dock: Option<BrowserView>,
    shown: Option<DevtoolsDock>,
    extent: Arc<Extent>,
}

thread_local! {
    static AREA: RefCell<DockArea> = RefCell::new(DockArea::default());
}

/// The panel the window puts below the shell, holding `grid` (and the dock later).
pub fn content(grid: View) -> Option<View> {
    let panel = sized_panel(0, 0, false)?;
    AREA.with(|area| {
        let mut area = area.borrow_mut();
        area.content = Some(panel.clone());
        area.grid = Some(grid);
        arrange(&area);
    });
    Some(View::from(&panel))
}

/// Lays the content area out: the grid (flex 1), then the dock (its own size) below or right.
fn arrange(area: &DockArea) {
    let Some(content) = &area.content else {
        return;
    };
    content.remove_all_child_views();
    crate::host::rebuild_watch::note("the DevTools dock was laid out anew".to_owned());
    let right = area
        .shown
        .as_ref()
        .is_some_and(|shown| shown.side == DockSide::Right);
    let layout = content.set_to_box_layout(Some(&BoxLayoutSettings {
        horizontal: i32::from(right),
        cross_axis_alignment: AxisAlignment::STRETCH,
        ..Default::default()
    }));
    if let Some(grid) = &area.grid {
        let mut grid = grid.clone();
        content.add_child_view(Some(&mut grid));
        if let Some(layout) = &layout {
            layout.set_flex_for_view(Some(&mut grid), 1);
        }
    }
    if let (Some(splitter), Some(_)) = (&area.splitter, &area.dock) {
        let mut splitter = View::from(splitter);
        content.add_child_view(Some(&mut splitter));
        if let Some(layout) = &layout {
            layout.set_flex_for_view(Some(&mut splitter), 0);
        }
    }
    if let Some(dock) = &area.dock {
        let mut dock = View::from(dock);
        content.add_child_view(Some(&mut dock));
        if let Some(layout) = &layout {
            layout.set_flex_for_view(Some(&mut dock), 0);
        }
    }
    content.layout();
}

/// Shows `dock` (the hosted frontend in a view of its own beside the grid) or removes it.
pub fn dock(dock: Option<DevtoolsDock>) {
    set_want(dock.as_ref());
    AREA.with(|area| {
        let mut area = area.borrow_mut();
        let Some(dock) = dock else {
            // Out of the content area first, then released: CEF destroys a detached browser
            // view's browser with its last reference. `close_browser` on the attached dock
            // closed the main window and quit the app (DoClose's default, RB-010-12).
            let closed = [area.dock.take(), area.splitter.take()];
            area.shown = None;
            arrange(&area);
            for view in closed.into_iter().flatten() {
                super::grid::release(view);
            }
            return;
        };
        area.extent.set(dock.side, dock.size);
        let Some(view) = area.dock.clone() else {
            area.dock = dock_view(&dock.frontend_url, area.extent.clone());
            if area.dock.is_none() {
                tracing::error!("the DevTools dock could not be created");
            }
            area.splitter = super::splitter::view(area.extent.clone());
            if area.splitter.is_none() {
                tracing::warn!(
                    "the DevTools splitter could not be created; the dock keeps its size"
                );
            }
            area.shown = Some(dock);
            arrange(&area);
            return;
        };
        let previous = area.shown.replace(dock.clone());
        let url_changed = previous
            .as_ref()
            .is_none_or(|shown| shown.frontend_url != dock.frontend_url);
        if url_changed && let Some(frame) = view.browser().and_then(|browser| browser.main_frame())
        {
            frame.load_url(Some(&CefString::from(dock.frontend_url.as_str())));
        }
        if previous.is_none_or(|shown| shown.side != dock.side) {
            arrange(&area);
        } else {
            if let Some(splitter) = &area.splitter {
                View::from(splitter).invalidate_layout();
            }
            View::from(&view).invalidate_layout();
            if let Some(content) = &area.content {
                content.layout();
            }
        }
    });
}

/// The dock's browser view, loading `url`.
fn dock_view(url: &str, extent: Arc<Extent>) -> Option<BrowserView> {
    let mut client = DockClient::new();
    let mut delegate = DockDelegate::new(extent);
    browser_view_create(
        Some(&mut client),
        Some(&CefString::from(url)),
        Some(&BrowserSettings::default()),
        None,
        None,
        Some(&mut delegate),
    )
}

/// Closes the native DevTools window of `view`.
pub fn close(view: ViewId) {
    let browser = UI.with(|ui| ui.borrow().views.get(&view).and_then(BrowserView::browser));
    if let Some(host) = browser.and_then(|browser| browser.host()) {
        host.close_dev_tools();
    }
}

/// Opens the native DevTools window of `view` (inspecting `browser`) at its last bounds.
pub fn show_window(view: ViewId, browser: &Browser) {
    let Some(host) = browser.host() else {
        return;
    };
    let mut info = popup_info();
    if let Some(bounds) = core().and_then(|core| crate::devtools::window_bounds(core)) {
        info.bounds = Rect {
            x: bounds.x,
            y: bounds.y,
            width: i32::try_from(bounds.width).unwrap_or(i32::MAX),
            height: i32::try_from(bounds.height).unwrap_or(i32::MAX),
        };
    }
    let mut client = WindowClient::new(view);
    host.show_dev_tools(
        Some(&info),
        Some(&mut client),
        Some(&BrowserSettings::default()),
        None,
    );
}

/// A top-level window of its own for the DevTools window.
#[cfg(windows)]
fn popup_info() -> WindowInfo {
    WindowInfo::default().set_as_popup(sys::HWND::default(), "DevTools")
}

/// A top-level window of its own for the DevTools window: with no parent view CEF makes one
/// on macOS (RB-090-15).
#[cfg(not(windows))]
fn popup_info() -> WindowInfo {
    WindowInfo {
        window_name: CefString::from("DevTools"),
        ..Default::default()
    }
}

/// The outer bounds of `browser`'s top-level window: not remembered on macOS yet (RB-090-15),
/// so the DevTools window opens where CEF puts it.
#[cfg(not(windows))]
fn window_bounds(_browser: &Browser) -> Option<WindowBounds> {
    None
}

/// The outer bounds of `browser`'s top-level window, in screen pixels.
#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "GetAncestor and GetWindowRect have no safe wrapper"
)]
fn window_bounds(browser: &Browser) -> Option<WindowBounds> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GA_ROOT, GetAncestor, GetWindowRect};
    let handle = browser.host()?.window_handle().0.cast::<std::ffi::c_void>();
    if handle.is_null() {
        return None;
    }
    // SAFETY: a window handle CEF gave for a live browser (this runs before it closes);
    // GetAncestor only reads it.
    let root = unsafe { GetAncestor(handle, GA_ROOT) };
    let root = if root.is_null() { handle } else { root };
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: `root` is a window handle as above, `rect` a valid out pointer.
    let ok = unsafe { GetWindowRect(root, &mut rect) };
    if ok == 0 {
        return None;
    }
    Some(WindowBounds {
        x: rect.left,
        y: rect.top,
        width: u32::try_from(rect.right - rect.left).ok()?,
        height: u32::try_from(rect.bottom - rect.top).ok()?,
    })
}

wrap_browser_view_delegate! {
    // The dock: Alloy style (ADR 0001, Decision 2), as large as `extent` wants and the content
    // area allows.
    struct DockDelegate {
        extent: Arc<Extent>,
    }

    impl ViewDelegate {
        fn preferred_size(&self, view: Option<&mut View>) -> Size {
            let right = self.extent.right.load(Ordering::Relaxed);
            let wanted = self.extent.size.load(Ordering::Relaxed);
            // The whole area: the box of the docked plugin panels around this content area
            // (RB-080-57), so the views keep their quarter with both docks.
            let area = view.and_then(|view| view.parent_view()).map(|parent| {
                parent.parent_view().unwrap_or(parent).bounds()
            });
            let (width, height) = area.map_or((0, 0), |bounds| {
                (
                    u32::try_from(bounds.width).unwrap_or(0),
                    u32::try_from(bounds.height).unwrap_or(0),
                )
            });
            let own = Some(DockWant {
                horizontal: right,
                wanted,
                fixed: false,
            });
            let extent = shared_extents(width, height, super::panel_dock::want(), own).1;
            let extent = i32::try_from(extent).unwrap_or(0);
            if right {
                super::views::strip(extent, 0)
            } else {
                super::views::strip(0, extent)
            }
        }
    }

    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

wrap_client! {
    // The dock's handlers: DevTools' keys, its load, the guard on what it loads, no popups.
    struct DockClient {}

    impl Client {
        fn keyboard_handler(&self) -> Option<KeyboardHandler> {
            Some(DevtoolsKeys::new())
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(DockLoad::new())
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(DockRequests::new())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(NoPopups::new())
        }
    }
}

wrap_load_handler! {
    struct DockLoad {}

    impl LoadHandler {
        fn on_load_end(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _http_status_code: c_int,
        ) {
            let (Some(frame), Some(core)) = (frame, core()) else {
                return;
            };
            if frame.is_main() != 0 {
                let url = CefString::from(&frame.url()).to_string();
                crate::devtools::on_dock_loaded(core, &url);
            }
        }
    }
}

/// Whether the dock's main frame may load `url`: its marker or a DevTools page.
fn dock_may_load(url: &str) -> bool {
    url == DOCK_MARKER_URL
        || url
            .get(.."devtools://".len())
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("devtools://"))
}

wrap_request_handler! {
    struct DockRequests {}

    impl RequestHandler {
        fn on_before_browse(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _user_gesture: c_int,
            _is_redirect: c_int,
        ) -> c_int {
            let main = frame.as_ref().is_some_and(|frame| frame.is_main() != 0);
            let url = request
                .map(|request| CefString::from(&request.url()).to_string())
                .unwrap_or_default();
            if main && !dock_may_load(&url) {
                tracing::warn!("the DevTools dock refused to leave DevTools");
                return 1;
            }
            0
        }
    }
}

wrap_life_span_handler! {
    pub(super) struct NoPopups {}

    impl LifeSpanHandler {
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: c_int,
            _target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: c_int,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut c_int>,
        ) -> c_int {
            1
        }

        // The dock and its splitter sit in the main window: CEF's default (false) "will send
        // the standard close notification to the browser's top-level parent window"
        // (`cef_life_span_handler.h`, DoClose) and would quit the app. `dock(None)` releases
        // their detached views instead.
        fn do_close(&self, _browser: Option<&mut Browser>) -> c_int {
            1
        }
    }
}

wrap_client! {
    // The native DevTools window of one view: DevTools' keys and its close.
    struct WindowClient {
        view: ViewId,
    }

    impl Client {
        fn keyboard_handler(&self) -> Option<KeyboardHandler> {
            Some(DevtoolsKeys::new())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(WindowLifeSpan::new(self.view))
        }
    }
}

wrap_life_span_handler! {
    struct WindowLifeSpan {
        view: ViewId,
    }

    impl LifeSpanHandler {
        fn on_before_close(&self, browser: Option<&mut Browser>) {
            let bounds = browser.and_then(|browser| window_bounds(browser));
            if let Some(core) = core() {
                crate::devtools::on_window_closed(core, self.view, bounds);
            }
        }
    }
}

/// Lets the content area's panels and the dock's views go (the main window was destroyed,
/// RB-080-91).
pub fn release() {
    AREA.with(|area| *area.borrow_mut() = DockArea::default());
}

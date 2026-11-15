//! The window of a view popped out of the grid (RB-080-14, `crate::pop_out`): the view's one
//! browser view moves out of its cell into a top-level window of its own and back - CEF Views
//! re-parents a view between widgets without reloading it, as the overlays of the layout modes
//! do - so the view keeps its browser, session, CDP target and sync agent. The window's client
//! area is the breakpoint at zoom 1 (`crate::pop_out::bounds::place`), its title names the size
//! and what the screen allowed. Closing it by hand gives the view back to its cell and keeps the
//! bounds; before any window goes, the browser view is taken out of it, so it never closes with
//! the window. With `ui.viewFooter` the window ends in the view's footer (RB-080-44,
//! `cell_footers.rs`) and is that much taller, so the page keeps its real size. UI thread.

use std::cell::RefCell;
use std::collections::HashMap;
use std::os::raw::c_int;

use cef::*;
use rb_engine::ViewId;

use super::cell_footers;
use super::grid::detach;
use super::views::{UI, core, strip};
use crate::host::ui::PopOutWindow;
use crate::pop_out::bounds::{
    Insets, PlaceInput, Placement, ScreenRect, frame_insets, percent, place, taller, title_key,
};
use crate::view_footer::CellFooter;

/// The i18n key of the placeholder in the cell of a popped-out view.
const PLACEHOLDER_KEY: &str = "popout.placeholder";
/// A work area when no display answers (never seen on a desktop), DIP.
const FALLBACK_AREA: ScreenRect = ScreenRect {
    x: 0,
    y: 0,
    width: 1280,
    height: 800,
};

thread_local! {
    /// The window of each popped-out view.
    static WINDOWS: RefCell<HashMap<ViewId, Window>> = RefCell::new(HashMap::new());
}

fn window_of(view: ViewId) -> Option<Window> {
    WINDOWS.with(|windows| windows.borrow().get(&view).cloned())
}

fn browser_of(view: ViewId) -> Option<BrowserView> {
    UI.with(|ui| ui.borrow().views.get(&view).cloned())
}

/// Moves `request.view` out of its cell into a window of its own; a view out already comes to
/// the front.
pub fn open(request: PopOutWindow) {
    let view = request.view;
    if window_of(view).is_some() {
        raise(view);
        return;
    }
    let Some(browser) = browser_of(view) else {
        tracing::warn!(%view, "no browser view to pop out");
        return;
    };
    detach(&View::from(&browser));
    let mut delegate = PopOutDelegate::new(request);
    match window_create_top_level(Some(&mut delegate)) {
        Some(window) => {
            WINDOWS.with(|windows| windows.borrow_mut().insert(view, window));
        }
        None => tracing::error!(%view, "the pop-out window could not be created"),
    }
}

/// Takes `view` out of its window and closes the window (`view.pop_in`); the grid built next
/// puts the view back into its cell.
pub fn close(view: ViewId) {
    let Some(window) = take(view) else {
        return;
    };
    release(view);
    window.close();
}

/// Closes the window of `view` after taking its browser view `browser` out; for the grid, which
/// holds the window's objects (`UI`) while it closes a dropped view's browser.
pub fn close_without(view: ViewId, browser: &BrowserView) {
    let Some(window) = take(view) else {
        return;
    };
    detach(&View::from(browser));
    window.close();
}

fn take(view: ViewId) -> Option<Window> {
    cell_footers::forget_window(view);
    WINDOWS.with(|windows| windows.borrow_mut().remove(&view))
}

/// Brings the window of `view` to the front (the placeholder's click, `view.pop_out` again).
pub fn raise(view: ViewId) {
    let Some(window) = window_of(view) else {
        return;
    };
    if window.is_minimized() != 0 {
        window.restore();
    }
    window.show();
    window.activate();
    if let Some(browser) = browser_of(view) {
        browser.request_focus();
    }
}

/// The app ends: every popped-out view goes back into the main window, so its browser closes
/// with it, and its window closes.
pub fn close_all() {
    let windows: Vec<(ViewId, Window)> =
        WINDOWS.with(|windows| windows.borrow_mut().drain().collect());
    let grid = UI.with(|ui| ui.borrow().grid.clone());
    for (view, window) in windows {
        release(view);
        if let (Some(grid), Some(browser)) = (&grid, browser_of(view)) {
            grid.add_child_view(Some(&mut View::from(&browser)));
        }
        window.close();
    }
}

/// Takes the browser view of `view` out of whatever holds it (its window).
fn release(view: ViewId) {
    if let Some(browser) = browser_of(view) {
        detach(&View::from(&browser));
    }
}

/// The window of `view` is closing by hand (the close button, Alt+F4): unless the app or
/// `view.pop_in` closes it, its bounds are kept and the view goes back into its cell.
fn closing(view: ViewId, window: &Window) {
    let ours = WINDOWS.with(|windows| {
        let mut windows = windows.borrow_mut();
        let same = windows
            .get(&view)
            .is_some_and(|stored| View::from(stored).is_same(Some(&mut View::from(window))) != 0);
        same && windows.remove(&view).is_some()
    });
    if !ours {
        return;
    }
    // The bounds kept are the page's: without the footer, whatever the setting is next time.
    let footer = i32::try_from(cell_footers::window_footer(view)).unwrap_or(0);
    let client = (window.is_minimized() == 0)
        .then(|| taller(screen_rect(&window.client_area_bounds_in_screen()), -footer));
    cell_footers::forget_window(view);
    release(view);
    if let Some(core) = core() {
        let state = crate::pop_out::store::default_path();
        // It writes a file: off the UI thread.
        core.runtime().spawn_blocking(move || {
            crate::pop_out::window_closed(core, view, client, state.as_deref());
        });
    }
}

/// Puts `window` where `request` wants it: first at the client size, to measure the frame the
/// system draws, then with the frame around the client area.
fn arrange(window: &Window, request: &PopOutWindow) {
    // The footer below the page: the window is that much taller (RB-080-44), by each of its
    // lines (RB-080-53), the remembered bounds - the page's - too.
    let footer = request.footer.as_ref().map_or(0, CellFooter::height);
    let remembered = request
        .remembered
        .map(|page| taller(page, i32::try_from(footer).unwrap_or(0)));
    let mut input = PlaceInput {
        viewport: (request.width, request.height.saturating_add(footer)),
        work_area: work_area(remembered),
        insets: Insets::default(),
        remembered,
        fit_to_screen: request.fit_to_screen,
    };
    let first = place(&input);
    window.set_bounds(Some(&cef_rect(first.client)));
    input.insets = frame_insets(
        screen_rect(&window.bounds_in_screen()),
        screen_rect(&window.client_area_bounds_in_screen()),
    );
    let placed = place(&input);
    window.set_bounds(Some(&cef_rect(placed.outer)));
    let title = title(request, &placed);
    window.set_title(Some(&CefString::from(title.as_str())));
}

/// The work area of the screen the window goes to: the remembered bounds' screen, else the
/// main window's, else the primary one.
fn work_area(remembered: Option<ScreenRect>) -> ScreenRect {
    let display = remembered
        .and_then(|rect| display_get_matching_bounds(Some(&cef_rect(rect)), 0))
        .or_else(|| UI.with(|ui| ui.borrow().window.as_ref().and_then(|main| main.display())))
        .or_else(display_get_primary);
    display.map_or(FALLBACK_AREA, |display| screen_rect(&display.work_area()))
}

fn title(request: &PopOutWindow, placed: &Placement) -> String {
    let key = title_key(placed, request.fit_to_screen);
    let (width, height) = (request.width.to_string(), request.height.to_string());
    let percent = percent(placed.zoom).to_string();
    rb_i18n::t_with(
        key,
        &[
            ("label", request.label.as_str()),
            ("width", width.as_str()),
            ("height", height.as_str()),
            ("percent", percent.as_str()),
        ],
    )
}

fn cef_rect(rect: ScreenRect) -> Rect {
    Rect {
        x: rect.x,
        y: rect.y,
        // No side 0: CEF takes such a size for empty.
        width: rect.width.max(1),
        height: rect.height.max(1),
    }
}

fn screen_rect(rect: &Rect) -> ScreenRect {
    ScreenRect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

/// The placeholder in the cell of `view`: a click brings its window to the front.
pub fn placeholder(view: ViewId) -> Option<View> {
    let mut delegate = Placeholder::new(view);
    let text = rb_i18n::t(PLACEHOLDER_KEY);
    let button = label_button_create(Some(&mut delegate), Some(&CefString::from(text.as_str())))?;
    button.set_horizontal_alignment(HorizontalAlignment::CENTER);
    Some(View::from(&button))
}

wrap_button_delegate! {
    struct Placeholder {
        view: ViewId,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            // Stretched by the cell's box; never a side of 0.
            strip(0, 0)
        }
    }

    impl ButtonDelegate {
        fn on_button_pressed(&self, _button: Option<&mut Button>) {
            raise(self.view);
        }
    }
}

wrap_window_delegate! {
    struct PopOutDelegate {
        request: PopOutWindow,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            let side = |value: u32| i32::try_from(value).unwrap_or(i32::MAX);
            strip(side(self.request.width), side(self.request.height))
        }
    }

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            let Some(window) = window else {
                return;
            };
            // The view over its footer; a hidden footer leaves the view the whole window.
            let layout = window.set_to_box_layout(Some(&BoxLayoutSettings {
                horizontal: 0,
                cross_axis_alignment: AxisAlignment::STRETCH,
                ..Default::default()
            }));
            let browser = browser_of(self.request.view);
            if let Some(browser) = &browser {
                let mut browser = View::from(browser);
                window.add_child_view(Some(&mut browser));
                if let Some(layout) = &layout {
                    layout.set_flex_for_view(Some(&mut browser), 1);
                }
            }
            let footer = self.request.footer.as_ref();
            if let Some(mut footer) = cell_footers::window_strip(self.request.view, footer) {
                window.add_child_view(Some(&mut footer));
            }
            super::icon::apply(window);
            super::theme::adopt(window);
            arrange(window, &self.request);
            window.show();
            if let Some(browser) = browser {
                browser.request_focus();
            }
        }

        // Windows switched its theme and CEF reset the colours: rBrowser's again (RB-080-95).
        fn on_theme_colors_changed(
            &self,
            window: Option<&mut Window>,
            _chrome_theme: ::std::os::raw::c_int,
        ) {
            if let Some(window) = window {
                super::theme::colors_changed(window);
            }
        }

        fn can_close(&self, window: Option<&mut Window>) -> c_int {
            if let Some(window) = window {
                closing(self.request.view, window);
            }
            1
        }

        // The cef crate's defaults are 0 (CEF's own are true): a pop-out is a normal window.
        fn can_resize(&self, _window: Option<&mut Window>) -> c_int {
            1
        }

        fn can_maximize(&self, _window: Option<&mut Window>) -> c_int {
            1
        }

        fn can_minimize(&self, _window: Option<&mut Window>) -> c_int {
            1
        }

        fn window_runtime_style(&self) -> RuntimeStyle {
            // Like every view of the app (ADR 0001, Decision 2): an Alloy browser view.
            RuntimeStyle::ALLOY
        }
    }
}

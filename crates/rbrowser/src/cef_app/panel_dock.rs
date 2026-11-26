//! The CEF side of the docked plugin panels (RB-080-57, `crate::panel_dock`): a box around the
//! content area (the grid and docked DevTools, `devtools.rs`) that puts the panels page and its
//! bar left, right or below it - the content area keeps flex 1 and gives up the room, so nothing
//! covers a view, and the box sits below the toolbar like the content area did.
//!
//! The panels page is the floating window's (`page_windows.rs`) in a browser view of its own
//! with the same handlers (`PagesClient`: the message router, the navigation guard, no popups);
//! its browser id joins the query check as the window's does (`queries.rs`). The bar is the
//! DevTools splitter's page (`crate::devtools::splitter::page_url`) in a thin browser view whose
//! title goes to [`crate::panel_dock::on_title`]. A view taken out is released, never closed
//! with `close_browser` (DoClose would close the main window, see `devtools.rs`). UI thread.

use std::cell::RefCell;
use std::os::raw::c_int;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU32, Ordering};

use cef::*;

use super::devtools::NoPopups;
use super::settings_window::PagesClient;
use super::views::{core, sized_panel, strip as strip_size};
use crate::devtools::layout::{DockWant, MAX_DOCK, shared_extents};
use crate::devtools::splitter::{THICKNESS, may_load, page_url};
use crate::page_window::Generations;
use crate::panel_dock::collapse::strip;
use crate::panel_dock::{DockedPanels, PanelSide, Room};
use crate::shell::PAGES_URL;

/// The docked page's browser id; 0 while none is shown.
static DOCK_BROWSER: AtomicI32 = AtomicI32::new(0);
/// Whether the panels are docked, at which side (0 left, 1 right, 2 bottom) and size: read by
/// the delegates here and by docked DevTools (`devtools.rs`) at every layout, so both docks
/// size against one area ([`shared_extents`]). Atomics, not `AREA`: a layout runs while
/// `AREA` is borrowed.
static SHOWN: AtomicBool = AtomicBool::new(false);
static SIDE: AtomicU8 = AtomicU8::new(0);
static SIZE: AtomicU32 = AtomicU32::new(0);
/// Folded to its strip (RB-080-61): the dock's size is [`strip`], it has no bar.
static COLLAPSED: AtomicBool = AtomicBool::new(false);

fn set_want(dock: Option<&DockedPanels>) {
    if let Some(dock) = dock {
        SIZE.store(dock.size, Ordering::Relaxed);
        COLLAPSED.store(dock.collapsed, Ordering::Relaxed);
        let side = match dock.side {
            PanelSide::Left => 0,
            PanelSide::Right => 1,
            PanelSide::Bottom => 2,
        };
        SIDE.store(side, Ordering::Relaxed);
    }
    SHOWN.store(dock.is_some(), Ordering::Relaxed);
}

fn side() -> PanelSide {
    match SIDE.load(Ordering::Relaxed) {
        0 => PanelSide::Left,
        1 => PanelSide::Right,
        _ => PanelSide::Bottom,
    }
}

/// What the docked panels want, `None` while none are docked. Folded, the strip at its own size
/// ([`strip`], `fixed`): [`shared_extents`] counts it, so docked DevTools beside it leave the
/// views their quarter (review point 7).
pub(super) fn want() -> Option<DockWant> {
    let folded = COLLAPSED.load(Ordering::Relaxed);
    let side = side();
    SHOWN.load(Ordering::Relaxed).then(|| DockWant {
        horizontal: side.horizontal(),
        wanted: if folded {
            strip(side)
        } else {
            SIZE.load(Ordering::Relaxed)
        },
        fixed: folded,
    })
}

#[derive(Default)]
struct Area {
    outer: Option<Panel>,
    /// The content area: the grid and docked DevTools.
    inner: Option<View>,
    splitter: Option<BrowserView>,
    dock: Option<BrowserView>,
    shown: Option<DockedPanels>,
}

thread_local! {
    static AREA: RefCell<Area> = RefCell::new(Area::default());
    /// The docked page's generation: a browser made for a dock already removed is an orphan.
    static GENERATIONS: RefCell<Generations<()>> = RefCell::new(Generations::default());
}

/// The docked page's browser id, 0 while none is shown (any thread).
pub fn browser_id() -> i32 {
    DOCK_BROWSER.load(Ordering::Acquire)
}

/// The browser `browser` closed: if it was the docked page, it no longer queries.
pub fn forget(browser: i32) {
    let _ = DOCK_BROWSER.compare_exchange(browser, 0, Ordering::AcqRel, Ordering::Acquire);
}

/// The box the window puts below the shell, holding the content area `inner` and, from the
/// first layout, the dock the launch's settings start with (RB-080-93: the icon rail), so the
/// views are laid out beside it once and never jump when it comes.
pub fn content(inner: View) -> Option<View> {
    let panel = sized_panel(0, 0, false)?;
    let first = super::start::LAUNCH
        .get()
        .and_then(|launch| crate::panel_dock::rail::first_dock(&launch.setup.user.panels));
    AREA.with(|area| {
        let mut area = area.borrow_mut();
        area.outer = Some(panel.clone());
        area.inner = Some(inner);
        arrange(&area);
    });
    if first.is_some() {
        dock(first);
    }
    Some(View::from(&panel))
}

/// The main window is closing or gone: no dock is made any more (the panels' own window
/// closing with it brings the rail back in the core, RB-080-93).
fn window_closing() -> bool {
    super::views::UI.with(|ui| {
        ui.borrow()
            .window
            .as_ref()
            .is_none_or(|window| window.is_closed() != 0)
    })
}

/// Lays the box out: left `[dock, bar, content]`, right `[content, bar, dock]`, bottom the same
/// downwards; the content flex 1, dock and bar their own size.
fn arrange(area: &Area) {
    let Some(outer) = &area.outer else {
        return;
    };
    outer.remove_all_child_views();
    crate::host::rebuild_watch::note("the plugin panels' dock was laid out anew".to_owned());
    let side = area.shown.as_ref().map(|shown| shown.side);
    let folded = area.shown.as_ref().is_some_and(|shown| shown.collapsed);
    let horizontal = side.is_some_and(PanelSide::horizontal);
    let layout = outer.set_to_box_layout(Some(&BoxLayoutSettings {
        horizontal: i32::from(horizontal),
        cross_axis_alignment: AxisAlignment::STRETCH,
        ..Default::default()
    }));
    let docked = area.dock.as_ref().map(View::from);
    // A folded dock has no bar: there is nothing to drag (RB-080-61).
    let bar = area
        .splitter
        .as_ref()
        .filter(|_| docked.is_some() && !folded)
        .map(View::from);
    let mut children: Vec<(View, i32)> = Vec::new();
    if let Some(inner) = &area.inner {
        children.push((inner.clone(), 1));
    }
    if let Some(bar) = bar {
        children.push((bar, 0));
    }
    if let Some(docked) = docked {
        children.push((docked, 0));
    }
    if side == Some(PanelSide::Left) {
        children.reverse();
    }
    for (mut child, flex) in children {
        outer.add_child_view(Some(&mut child));
        if let Some(layout) = &layout {
            layout.set_flex_for_view(Some(&mut child), flex);
        }
    }
    outer.layout();
}

/// The width and height of `view`'s parent - the box around the views and both docks.
fn area_of(view: Option<&mut View>) -> (u32, u32) {
    view.and_then(|view| view.parent_view())
        .map(|parent| {
            let bounds = parent.bounds();
            (
                u32::try_from(bounds.width).unwrap_or(0),
                u32::try_from(bounds.height).unwrap_or(0),
            )
        })
        .unwrap_or((0, 0))
}

/// Shows `dock` or removes it.
pub fn dock(dock: Option<DockedPanels>) {
    AREA.with(|area| {
        let mut area = area.borrow_mut();
        set_want(dock.as_ref());
        let Some(dock) = dock else {
            let closed = [area.dock.take(), area.splitter.take()];
            area.shown = None;
            arrange(&area);
            // A browser still to come for this page is an orphan now (review point 6).
            GENERATIONS.with(|generations| generations.borrow_mut().close(()));
            DOCK_BROWSER.store(0, Ordering::Release);
            for view in closed.into_iter().flatten() {
                super::grid::release(view);
            }
            return;
        };
        if area.dock.is_none() {
            // The window has none before it is made; once it closes, none is made again.
            if area.outer.is_none() || (super::views::core().is_some() && window_closing()) {
                return;
            }
            // One panels page at a time: the floating window goes, so its panels stop working
            // in the background.
            super::page_windows::close(crate::page_window::PageWindow::Panels);
            let generation = GENERATIONS.with(|generations| generations.borrow_mut().open(()));
            area.dock = page_view(&dock.url, generation);
            if area.dock.is_none() {
                tracing::error!("the docked plugin panels could not be created");
            }
            area.splitter = bar_view();
            if area.splitter.is_none() {
                tracing::warn!(
                    "the plugin panels' bar could not be created; the dock keeps its size"
                );
            }
            area.shown = Some(dock);
            arrange(&area);
            return;
        }
        let previous = area.shown.replace(dock.clone());
        // Another side, or folded or opened: the bar comes or goes, the box is laid out anew.
        if previous.is_none_or(|shown| shown.side != dock.side || shown.collapsed != dock.collapsed)
        {
            arrange(&area);
        } else {
            for view in [&area.dock, &area.splitter].into_iter().flatten() {
                View::from(view).invalidate_layout();
            }
            if let Some(outer) = &area.outer {
                outer.layout();
            }
        }
    });
}

/// Runs `script` (a core -> shell event) in the docked page, when one is shown.
pub fn send(script: &str) {
    let frame = AREA.with(|area| {
        area.borrow()
            .dock
            .as_ref()
            .and_then(BrowserView::browser)
            .and_then(|browser| browser.main_frame())
    });
    if let Some(frame) = frame {
        frame.execute_java_script(
            Some(&CefString::from(script)),
            Some(&CefString::from(PAGES_URL)),
            0,
        );
    }
}

/// The docked page's browser view at `url`, with the floating window's handlers.
fn page_view(url: &str, generation: u64) -> Option<BrowserView> {
    let mut client = PagesClient::new();
    let mut delegate = DockedPage::new(generation);
    browser_view_create(
        Some(&mut client),
        Some(&CefString::from(url)),
        Some(&BrowserSettings::default()),
        None,
        None,
        Some(&mut delegate),
    )
}

/// The bar's browser view.
fn bar_view() -> Option<BrowserView> {
    let mut client = BarClient::new();
    let mut delegate = BarDelegate::new();
    browser_view_create(
        Some(&mut client),
        Some(&CefString::from(page_url().as_str())),
        Some(&BrowserSettings::default()),
        None,
        None,
        Some(&mut delegate),
    )
}

wrap_browser_view_delegate! {
    // The docked page: Alloy style (ADR 0001, Decision 2), as large as it wants and the box
    // allows - the views keep a quarter of the area beside docked DevTools too
    // ([`shared_extents`]). Its layout tells the core where a drag or a step starts.
    struct DockedPage {
        generation: u64,
    }

    impl ViewDelegate {
        fn preferred_size(&self, view: Option<&mut View>) -> Size {
            let side = side();
            let (width, height) = area_of(view);
            let extent = shared_extents(width, height, want(), super::devtools::want()).0;
            let extent = i32::try_from(extent).unwrap_or(0);
            if side.horizontal() {
                strip_size(extent, 0)
            } else {
                strip_size(0, extent)
            }
        }

        fn on_layout_changed(&self, view: Option<&mut View>, new_bounds: Option<&Rect>) {
            let (Some(view), Some(bounds), Some(core)) = (view, new_bounds, core()) else {
                return;
            };
            let current = GENERATIONS.with(|generations| generations.borrow().is_current((), self.generation));
            // The folded strip is no size of the dock (RB-080-61).
            if !current || !SHOWN.load(Ordering::Relaxed) || COLLAPSED.load(Ordering::Relaxed) {
                return;
            }
            let side = side();
            let horizontal = side.horizontal();
            let shown = if horizontal { bounds.width } else { bounds.height };
            let (width, height) = area_of(Some(view));
            let most = Some(DockWant { horizontal, wanted: MAX_DOCK, fixed: false });
            let ceiling = shared_extents(width, height, most, super::devtools::want()).0;
            crate::panel_dock::on_room(
                core,
                Room {
                    side,
                    shown: u32::try_from(shown).unwrap_or(0),
                    ceiling,
                },
            );
        }
    }

    impl BrowserViewDelegate {
        fn on_browser_created(
            &self,
            _browser_view: Option<&mut BrowserView>,
            browser: Option<&mut Browser>,
        ) {
            let Some(browser) = browser else {
                return;
            };
            let current = GENERATIONS.with(|generations| generations.borrow().is_current((), self.generation));
            if current {
                DOCK_BROWSER.store(browser.identifier(), Ordering::Release);
                return;
            }
            // The dock went before its browser came (review point 6): it gets no query rights
            // and is closed at once. Its view is out of every window, so closing it closes no
            // window.
            tracing::warn!("a docked plugin panels page outlived its dock; closed");
            if let Some(host) = browser.host() {
                host.close_browser(1);
            }
        }

        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

wrap_browser_view_delegate! {
    // The bar: thin across the dock's axis, stretched along it.
    struct BarDelegate {}

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            let thickness = i32::try_from(THICKNESS).unwrap_or(6);
            if side().horizontal() {
                strip_size(thickness, 0)
            } else {
                strip_size(0, thickness)
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
    // The bar's handlers: its title, the guard on what it loads, no popups.
    struct BarClient {}

    impl Client {
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(BarTitle::new())
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(BarRequests::new())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(NoPopups::new())
        }
    }
}

wrap_display_handler! {
    struct BarTitle {}

    impl DisplayHandler {
        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if let (Some(title), Some(core)) = (title, core()) {
                crate::panel_dock::on_title(core, &title.to_string());
            }
        }
    }
}

wrap_request_handler! {
    struct BarRequests {}

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
            if main && !may_load(&url) {
                tracing::warn!("the plugin panels' bar refused to leave its page");
                return 1;
            }
            0
        }
    }
}

/// Lets the dock's panels and views go (the main window was destroyed, RB-080-91).
pub fn release() {
    AREA.with(|area| *area.borrow_mut() = Area::default());
}

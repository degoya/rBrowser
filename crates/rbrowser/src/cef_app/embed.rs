//! An external view's window in its grid cell (RB-080-39): the owned overlay, stage 1 of the
//! job (ADR 0003). The core asks for a view's window by its engine's process
//! (`UiBackend::embed_external`); this module finds it among the top-level windows of that
//! process and its children (Firefox's launcher process starts the browser that owns the
//! windows; `rb_engine::external::embed::pick_window`), gives it the main window as its owner,
//! takes its frame (`embedded_style`) and from then on keeps it over the cell's view area on
//! every layout, move, resize, minimize, restore, mode switch and canvas pan - clipped to the
//! grid area with a window region, hidden while its cell is - until the core releases it
//! (`release_external`: the frame and the owner come back). The rectangles are the cell's
//! browser view's (the stand-in stays underneath: it shows the view while the window is away),
//! converted from DIP to pixels by CEF; every cell's size in pixels goes to the core, which
//! scales the page to it (RB-080-40, `engines::embed::cells`). A shell with a menu bar and a
//! toolbar above its page (Playwright's WebKit) loses the menu and is moved up by the toolbar,
//! which the region clips away. The Win32 calls are `embed_windows.rs`. UI thread.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use cef::*;
use rb_engine::ViewId;
use rb_engine::external::{
    CellPixels, EmbedPlace, WindowRect, WindowStyle, content_inset, embedded_style, pick_window,
    place_in_cell,
};

use super::embed_windows as win;
use super::task::on_ui_after;
use super::views::{UI, host};
use crate::engines::HostEngines;
use crate::engines::embed::process_family;
use crate::host::ui::EmbedRequest;

/// How long to wait between two looks for a window that is not there yet.
const RETRY_MS: u32 = 250;
/// How often to look before the embedding counts as failed (10 s in all).
const RETRIES: u32 = 40;

/// A window bound to its cell.
struct Bound {
    handle: usize,
    /// The styles to give back on release.
    normal: WindowStyle,
    /// The menu bar taken away, to give back on release; 0 without one.
    menu: usize,
    /// How far below the window's top its page starts (the shell's toolbar).
    inset: u32,
    /// Where it was put last; nothing is sent twice.
    last: Option<EmbedPlace>,
}

/// A request still looking for its window.
struct Pending {
    request: EmbedRequest,
    /// Stamps the retry chain: a newer request for the view ends the older chain.
    generation: u64,
}

#[derive(Default)]
struct State {
    /// The window of each embedded view.
    bound: HashMap<ViewId, Bound>,
    /// The views whose window is still looked for.
    pending: HashMap<ViewId, Pending>,
    /// The window of each released view, so a new view never takes it and a pop-in finds it.
    released: HashMap<ViewId, usize>,
    /// The last generation handed out.
    generation: u64,
}

impl State {
    /// Forgets every window that is gone (the view ended; Windows reuses handle values).
    fn prune(&mut self) {
        self.bound.retain(|_, bound| win::is_window(bound.handle));
        self.released.retain(|_, handle| win::is_window(*handle));
    }

    /// The windows no new binding may take.
    fn taken(&self) -> Vec<usize> {
        self.bound
            .values()
            .map(|bound| bound.handle)
            .chain(self.released.values().copied())
            .collect()
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Looks for `request.view`'s window and binds it to the cell; the core hears
/// `embedded_bound` or `embed_failed`. A window bound to the view before (its old window) is
/// given back first; an older look for the view ends.
pub fn embed(request: EmbedRequest) {
    let view = request.view;
    let (old, generation) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.generation += 1;
        let generation = state.generation;
        let old = state.bound.remove(&view);
        state.pending.insert(
            view,
            Pending {
                request,
                generation,
            },
        );
        (old, generation)
    });
    if let Some(old) = old {
        // Not recorded as released: the core closed it before it started the new window; one
        // still alive is a window of its own again.
        give_back(&old, None);
    }
    attempt(view, generation, 0);
}

fn attempt(view: ViewId, generation: u64, tries: u32) {
    let request = STATE.with(|state| {
        state
            .borrow()
            .pending
            .get(&view)
            .filter(|pending| pending.generation == generation)
            .map(|pending| pending.request.clone())
    });
    let Some(request) = request else {
        // Released meanwhile, or a newer request took over.
        return;
    };
    let Some(owner) = main_window_handle() else {
        fail(view, "the main window does not exist");
        return;
    };
    let picked = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.prune();
        // The window of this view released earlier (a pop-in): that one.
        if let Some(handle) = state.released.remove(&view) {
            return Some(handle);
        }
        let taken = state.taken();
        let family = crate::perf::process::links()
            .map(|links| process_family(request.window.process_id, &links))
            .unwrap_or_else(|| vec![request.window.process_id]);
        let candidates = win::top_level_windows_of(&family, request.window.window_class.as_deref());
        pick_window(&candidates, &taken, request.expected)
    });
    match picked {
        Some(handle) => bind(view, handle, owner),
        None if tries < RETRIES => {
            on_ui_after(RETRY_MS, move || attempt(view, generation, tries + 1));
        }
        None => fail(
            view,
            &format!(
                "no top-level window of process {} or its children was found",
                request.window.process_id
            ),
        ),
    }
}

fn bind(view: ViewId, handle: usize, owner: usize) {
    let normal = win::style(handle);
    let menu = win::remove_menu(handle);
    win::set_style(handle, embedded_style(normal));
    win::set_owner(handle, owner);
    win::frame_changed(handle);
    let inset = content_inset(&win::child_rects(handle));
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.pending.remove(&view);
        state.bound.insert(
            view,
            Bound {
                handle,
                normal,
                menu,
                inset,
                last: None,
            },
        );
    });
    tracing::info!(
        %view,
        handle = format_args!("{handle:#x}"),
        inset,
        menu = menu != 0,
        "external window bound to its cell"
    );
    place_view(view);
    if let Some(engines) = engines() {
        engines.embedded_bound(view);
    }
}

fn fail(view: ViewId, reason: &str) {
    STATE.with(|state| state.borrow_mut().pending.remove(&view));
    if let Some(engines) = engines() {
        engines.embed_failed(view, reason);
    }
}

/// Gives `view`'s window back as a window of its own: the frame and the owner it had, no
/// region, at `rect` when the core names one.
pub fn release(view: ViewId, rect: Option<WindowRect>) {
    let bound = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.pending.remove(&view);
        state.prune();
        state.bound.remove(&view)
    });
    let Some(bound) = bound else {
        return;
    };
    give_back(&bound, rect);
    STATE.with(|state| state.borrow_mut().released.insert(view, bound.handle));
    tracing::info!(%view, "external window released from its cell");
}

/// Restores `bound`'s window to what it was: no region, no owner, its own frame; shown at
/// `rect` or where it is. A window gone already is left alone.
fn give_back(bound: &Bound, rect: Option<WindowRect>) {
    let handle = bound.handle;
    if !win::is_window(handle) {
        return;
    }
    win::set_region(handle, None);
    win::set_owner(handle, 0);
    win::set_style(handle, bound.normal);
    win::set_menu(handle, bound.menu);
    win::frame_changed(handle);
    match rect {
        Some(rect) => win::place(handle, rect),
        None => win::show(handle),
    }
}

/// Puts every embedded window over its cell again (the window moved or was laid out).
pub fn place_all() {
    let views: Vec<ViewId> = STATE.with(|state| state.borrow().bound.keys().copied().collect());
    for view in views {
        place_view(view);
    }
}

/// Puts `view`'s window over its cell, when it is embedded and the place changed, after
/// telling the core the cell's size in pixels (RB-080-40, every view: the core knows which are
/// external). Nothing happens while the window's objects are borrowed (a layout inside
/// `grid::apply`): the `place_all` after it catches up.
pub fn place_view(view: ViewId) {
    let Some(cell) = cell_of(view) else {
        return;
    };
    if let (Cell::Shown { cell, .. }, Some(engines)) = (&cell, engines()) {
        engines.cell_sized(
            view,
            CellPixels {
                width: cell.width,
                height: cell.height,
            },
        );
    }
    let handle = STATE.with(|state| state.borrow().bound.get(&view).map(|bound| bound.handle));
    let Some(handle) = handle else {
        return;
    };
    if !win::is_window(handle) {
        // The window is gone (the view ended): nothing to place any more.
        STATE.with(|state| state.borrow_mut().bound.remove(&view));
        return;
    }
    // The toolbar's height, measured again every time: the shell may not have laid its
    // children out when the window was bound, and a DPI change resizes them.
    let children = win::child_rects(handle);
    let inset = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(bound) = state.bound.get_mut(&view) else {
            return 0;
        };
        if !children.is_empty() {
            bound.inset = content_inset(&children);
        }
        bound.inset
    });
    let place = match cell {
        Cell::Hidden => EmbedPlace::Hidden,
        Cell::Shown { cell, area } => place_in_cell(cell, area, true, inset),
    };
    let changed = STATE.with(|state| {
        let mut state = state.borrow_mut();
        match state.bound.get_mut(&view) {
            Some(bound) if bound.last != Some(place) => {
                bound.last = Some(place);
                true
            }
            _ => false,
        }
    });
    if !changed {
        return;
    }
    match place {
        EmbedPlace::Hidden => win::hide(handle),
        EmbedPlace::Shown { rect, region } => {
            win::set_region(handle, region);
            win::place(handle, rect);
        }
    }
}

/// A view's cell as the window has it now, in screen pixels.
enum Cell {
    /// Not drawn: the main window is minimized, the view is hidden (a hidden overlay of a
    /// layout mode, a view outside the canvas) or there is no window yet.
    Hidden,
    /// The cell's view area and the grid area.
    Shown { cell: WindowRect, area: WindowRect },
}

/// Where `view`'s cell is now: its view area inside the grid area, in screen pixels. `None`
/// while the window's objects are borrowed by the caller up the stack.
fn cell_of(view: ViewId) -> Option<Cell> {
    let (window, browser, grid) = UI.with(|ui| {
        let ui = ui.try_borrow().ok()?;
        Some((
            ui.window.clone(),
            ui.views.get(&view).cloned(),
            ui.grid.clone(),
        ))
    })?;
    let (Some(window), Some(browser), Some(grid)) = (window, browser, grid) else {
        return Some(Cell::Hidden);
    };
    if window.is_minimized() != 0 {
        return Some(Cell::Hidden);
    }
    let cell = View::from(&browser);
    if cell.is_drawn() == 0 {
        return Some(Cell::Hidden);
    }
    let cell = pixels(&cell.bounds_in_screen());
    let area = pixels(&View::from(&grid).bounds_in_screen());
    if cell.width == 0 || cell.height == 0 {
        return Some(Cell::Hidden);
    }
    Some(Cell::Shown { cell, area })
}

/// `rect` (screen DIP) in screen pixels, per monitor as Windows scales it.
fn pixels(rect: &Rect) -> WindowRect {
    let pixels = display_convert_screen_rect_to_pixels(Some(rect));
    WindowRect {
        x: pixels.x,
        y: pixels.y,
        width: u32::try_from(pixels.width).unwrap_or(0),
        height: u32::try_from(pixels.height).unwrap_or(0),
    }
}

fn main_window_handle() -> Option<usize> {
    UI.with(|ui| {
        ui.try_borrow()
            .ok()?
            .window
            .as_ref()
            .map(|window| super::handle::value(window.window_handle()))
    })
    .filter(|handle| *handle != 0)
}

fn engines() -> Option<Arc<HostEngines>> {
    host().and_then(|host| crate::engines::for_host(host))
}

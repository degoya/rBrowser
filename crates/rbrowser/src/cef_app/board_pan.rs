//! The middle mouse button pans the canvas, and Ctrl + wheel over a native strip zooms it
//! (RB-080-50, `crate::layout::{middle_pan, canvas_zoom}`). The views are native and the board
//! lies behind them, so no page can take a drag that starts over a view, and a footer strip
//! hears no wheel: the UI thread's mouse hook (`strip_clicks.rs`) hands these messages here.
//!
//! - A middle press over the canvas board's area - in canvas mode, with
//!   `layoutModes.middleButtonPan` - is taken from the page under the pointer, so Chromium's
//!   autoscroll never starts. Dragged past a few DIP it pans: every move shows the canvas there,
//!   the release keeps the camera, Escape puts it back. Let go where it went down it was a
//!   click, and the page gets it after all (the press and the release posted to its window).
//! - Ctrl + wheel over a footer strip on the canvas zooms the canvas around the pointer, as
//!   the board does for its own wheel: shown while it turns, kept when the wheel rests.
//!
//! [`on_mouse`] runs inside the hook, so it only reads: two switches kept as atomics
//! ([`refresh`]), the board's rectangle from CEF, the keys' state - no lock, no settings, no
//! work. What it decides runs as a task of the UI thread, at most one move waiting. Each task
//! of a drag carries the drag's number, so one that runs late never touches the next drag.

use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicBool, Ordering};

use cef::*;
use rb_ipc::DragPhase;
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CONTROL, VK_ESCAPE, VK_MBUTTON,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GetAncestor, IsWindow, PostMessageW, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_MOUSEMOVE, WM_MOUSEWHEEL,
};

use super::task::{on_ui, on_ui_after};
use super::views::core;
use crate::layout::canvas::Camera;
use crate::layout::canvas_zoom::{self, NOTCH, REST_MS};
use crate::layout::middle_pan::{self, MiddlePan, Step};

/// `MK_MBUTTON`: the middle button is down, in a mouse message's `wParam`.
const MK_MBUTTON: usize = 0x0010;
/// `WHEEL_DELTA`: one notch of the wheel as Windows counts it.
const WHEEL_DELTA: f64 = 120.0;

/// `layoutModes.middleButtonPan` and `layoutModes.wheelZoom`, for the hook.
static PAN_ON: AtomicBool = AtomicBool::new(false);
static WHEEL_ON: AtomicBool = AtomicBool::new(false);

/// A wheel turning over a native strip.
#[derive(Clone, Copy)]
struct Turn {
    /// Where it began, from the board's top-left corner.
    at: (i32, i32),
    /// The whole turn so far, in CSS px.
    total: i32,
    /// Counts the frames: the rest timer of an older frame does nothing.
    frame: u64,
}

thread_local! {
    /// The middle button's gesture.
    static PAN: RefCell<MiddlePan> = RefCell::new(MiddlePan::default());
    /// The number of the gesture under way.
    static DRAG: Cell<u64> = const { Cell::new(0) };
    /// The camera when the drag of that number began.
    static FROM: Cell<Option<(u64, Camera)>> = const { Cell::new(None) };
    /// The newest move not yet shown (drag, dx, dy); a task of the UI thread waits for it.
    static LATEST: Cell<Option<(u64, i32, i32)>> = const { Cell::new(None) };
    /// The window and the screen pixel of the press, for the click a press may turn out to be.
    static PRESS: Cell<(usize, i32, i32)> = const { Cell::new((0, 0, 0)) };
    /// How many of the next middle-button messages are our own click on its way to the page.
    static REPLAY: Cell<u8> = const { Cell::new(0) };
    /// The wheel's turn over a native strip.
    static WHEEL: Cell<Option<Turn>> = const { Cell::new(None) };
}

/// Reads the two switches from the user settings; called at the start and whenever they
/// change (`backend.rs`), from any thread - never from the hook.
pub fn refresh() {
    if let Some(core) = core() {
        let modes = core.user_settings().layout_modes;
        PAN_ON.store(modes.middle_button_pan, Ordering::Release);
        WHEEL_ON.store(modes.wheel_zoom, Ordering::Release);
    }
}

/// A mouse message of the UI thread at the screen pixel `x`/`y` for the window `target`, with
/// the wheel's turn in `wheel`; `true` when it is taken from that window. Inside the hook.
pub fn on_mouse(message: u32, x: i32, y: i32, target: usize, wheel: i16) -> bool {
    match message {
        WM_MBUTTONDOWN | WM_MBUTTONDBLCLK | WM_MBUTTONUP if replaying() => false,
        // A double click's second press is a press: left to the page it would start the
        // autoscroll the first one was kept from.
        WM_MBUTTONDOWN | WM_MBUTTONDBLCLK => {
            let at = dip(x, y);
            let step = PAN.with(|pan| pan.borrow_mut().down(at, on_canvas(at, target)));
            if step == Step::Begin {
                PRESS.with(|press| press.set((target, x, y)));
            }
            apply(step)
        }
        WM_MOUSEMOVE if active() => {
            let step = PAN.with(|pan| {
                pan.borrow_mut()
                    .moved(dip(x, y), held(VK_MBUTTON), held(VK_ESCAPE))
            });
            apply(step)
        }
        WM_MBUTTONUP if active() => {
            let step = PAN.with(|pan| pan.borrow_mut().up(dip(x, y)));
            apply(step)
        }
        WM_MOUSEWHEEL if wheel != 0 && WHEEL_ON.load(Ordering::Acquire) && held(VK_CONTROL) => {
            // Whether it is over a native strip shows on the UI thread's next turn; the board
            // and the pages take their own wheel, so the message is passed on either way.
            on_ui(move || strip_wheel(x, y, wheel));
            false
        }
        _ => false,
    }
}

fn active() -> bool {
    PAN.with(|pan| pan.borrow().active())
}

/// Whether this middle-button message is the click we posted for the page; counts it off.
fn replaying() -> bool {
    REPLAY.with(|replay| {
        let left = replay.get();
        replay.set(left.saturating_sub(1));
        left > 0
    })
}

/// Does what `step` says; `true` when its message is taken from the page.
fn apply(step: Step) -> bool {
    match step {
        Step::Pass => false,
        Step::Take => true,
        Step::Begin => {
            let drag = DRAG.with(|drag| {
                drag.set(drag.get().wrapping_add(1));
                drag.get()
            });
            LATEST.with(|latest| latest.set(None));
            // The camera is the core's: read on the UI thread's own turn, before any move.
            on_ui(move || {
                let from = core().and_then(|core| crate::layout::canvas_live::camera(core).ok());
                FROM.with(|kept| kept.set(from.map(|camera| (drag, camera))));
            });
            true
        }
        Step::Move { dx, dy } => {
            let drag = DRAG.with(Cell::get);
            // One task at a time: it shows the newest move when it runs.
            if LATEST
                .with(|latest| latest.replace(Some((drag, dx, dy))))
                .is_none()
            {
                on_ui(|| {
                    if let Some((drag, dx, dy)) = LATEST.with(Cell::take) {
                        with_drag(drag, |core, from| middle_pan::show(core, from, dx, dy));
                    }
                });
            }
            false
        }
        Step::End { dx, dy, taken } => {
            let drag = DRAG.with(Cell::get);
            LATEST.with(|latest| latest.set(None));
            on_ui(move || {
                with_drag(drag, |core, from| middle_pan::keep(core, from, dx, dy));
                forget(drag);
            });
            taken
        }
        Step::Cancel => {
            let drag = DRAG.with(Cell::get);
            LATEST.with(|latest| latest.set(None));
            on_ui(move || {
                with_drag(drag, |core, _| {
                    crate::layout::canvas_live::back(core);
                    Ok(())
                });
                forget(drag);
            });
            false
        }
        Step::Click { .. } => {
            click_through();
            true
        }
    }
}

/// Runs `job` with the camera the drag `drag` began at - nothing when another drag has begun
/// since or the canvas was gone at the press.
fn with_drag(
    drag: u64,
    job: impl FnOnce(&crate::control::Core, Camera) -> Result<(), rb_commands::CommandError>,
) {
    let from = FROM.with(Cell::get).filter(|(of, _)| *of == drag);
    if let (Some(core), Some((_, from))) = (core(), from)
        && let Err(error) = job(core, from)
    {
        tracing::warn!(%error, "the middle button's pan failed");
    }
}

/// The drag `drag` is over: its camera goes, unless a newer drag has taken the place.
fn forget(drag: u64) {
    FROM.with(|kept| {
        if kept.get().is_some_and(|(of, _)| of == drag) {
            kept.set(None);
        }
    });
}

/// The press was a click: the window under it gets the press and the release it was kept from,
/// at the point of the press.
#[allow(
    unsafe_code,
    reason = "IsWindow, ScreenToClient and PostMessageW have no safe wrapper"
)]
fn click_through() {
    let (target, x, y) = PRESS.with(Cell::get);
    let window = target as HWND;
    let mut point = POINT { x, y };
    // SAFETY: the calls only read the handle; `point` is a valid in-out pointer. A handle that
    // is no window any more is refused by IsWindow before it is used.
    let ok = unsafe { IsWindow(window) != 0 && ScreenToClient(window, &mut point) != 0 };
    if !ok {
        return;
    }
    // The client point as a mouse message carries it: x in the low word, y in the high.
    let place = ((point.y as u16 as isize) << 16) | (point.x as u16 as isize);
    REPLAY.with(|replay| replay.set(2));
    // SAFETY: posting to a window of this thread's own process; the messages carry no pointer.
    let posted = unsafe {
        PostMessageW(window, WM_MBUTTONDOWN, MK_MBUTTON, place) != 0
            && PostMessageW(window, WM_MBUTTONUP, 0, place) != 0
    };
    if !posted {
        REPLAY.with(|replay| replay.set(0));
        tracing::warn!("a middle click could not be handed to the page");
    }
}

/// Ctrl + wheel at the screen pixel `x`/`y`: over a footer strip on the canvas it zooms the
/// canvas, shown at once and kept when the wheel has rested.
fn strip_wheel(x: i32, y: i32, wheel: i16) {
    let (Some(core), Some((area, _))) = (core(), super::canvas_board::area_in_screen()) else {
        return;
    };
    let at = dip(x, y);
    let inside = |rect: &Rect| {
        (rect.x..rect.x.saturating_add(rect.width)).contains(&at.0)
            && (rect.y..rect.y.saturating_add(rect.height)).contains(&at.1)
    };
    if !inside(&area) || !super::cell_footers::drawn_rects().iter().any(inside) {
        return;
    }
    // Away from the user zooms in, as the page's wheel event has it: a negative delta.
    let turned = (-f64::from(wheel) / WHEEL_DELTA * NOTCH).round() as i32;
    let before = WHEEL.with(Cell::get);
    let turn = Turn {
        at: before.map_or((at.0 - area.x, at.1 - area.y), |turn| turn.at),
        total: before.map_or(0, |turn| turn.total).saturating_add(turned),
        frame: before.map_or(0, |turn| turn.frame).wrapping_add(1),
    };
    WHEEL.with(|kept| kept.set(Some(turn)));
    zoom(core, turn, DragPhase::Move);
    on_ui_after(REST_MS, move || {
        let rested = WHEEL.with(Cell::get).filter(|now| now.frame == turn.frame);
        if let (Some(core), Some(turn)) = (super::views::core(), rested) {
            WHEEL.with(|kept| kept.set(None));
            zoom(core, turn, DragPhase::End);
        }
    });
}

fn zoom(core: &crate::control::Core, turn: Turn, phase: DragPhase) {
    if let Err(error) = canvas_zoom::wheel(core, turn.at.0, turn.at.1, turn.total, phase) {
        tracing::warn!(%error, "the wheel's zoom over a strip failed");
    }
}

/// Whether the screen point `at` (DIP) is over the canvas board of the window `target`
/// belongs to, with the pan switched on.
fn on_canvas(at: (i32, i32), target: usize) -> bool {
    if !PAN_ON.load(Ordering::Acquire) {
        return false;
    }
    let Some((area, window)) = super::canvas_board::area_in_screen() else {
        return false;
    };
    let inside = (area.x..area.x.saturating_add(area.width)).contains(&at.0)
        && (area.y..area.y.saturating_add(area.height)).contains(&at.1);
    // A menu or another window over the canvas keeps its own middle button.
    inside && root_of(target) == root_of(window)
}

/// The screen pixel `x`/`y` in DIP, as CEF lays its views out.
fn dip(x: i32, y: i32) -> (i32, i32) {
    let point = display_convert_screen_point_from_pixels(Some(&Point { x, y }));
    (point.x, point.y)
}

/// Whether `key` is down as of the message being handled.
#[allow(unsafe_code, reason = "GetKeyState has no safe wrapper")]
fn held(key: u16) -> bool {
    // SAFETY: GetKeyState only reads the calling thread's key state.
    unsafe { GetKeyState(i32::from(key)) < 0 }
}

/// The top-level window `window` belongs to; `window` itself when it has none.
#[allow(unsafe_code, reason = "GetAncestor has no safe wrapper")]
pub(super) fn root_of(window: usize) -> usize {
    // SAFETY: GetAncestor only reads the handle and answers null for one that is no window.
    let root = unsafe { GetAncestor(window as HWND, GA_ROOT) } as usize;
    if root == 0 { window } else { root }
}

//! The mouse hook of the UI thread, and the right mouse button on a label strip (RB-080-46).
//! A CEF Views button reports its press without the mouse button and only for the left one
//! (`ButtonDelegate::on_button_pressed`), and a Views panel gets no mouse events at all - so in
//! the grid, the focus view, the row and the split a right-click on a strip did nothing. The
//! window's thread hears the button itself: a `WH_MOUSE` hook sees every mouse message the
//! thread takes from its queue, and a `WM_RBUTTONUP` whose point lies on a strip opens that
//! strip's menu ([`crate::layout::view_menu::strip_at`]) - the view menu without making the
//! view active, on the strip of an empty cell the add menu. `ui.viewContextMenu` off opens
//! nothing. The same hook hands the middle button and the wheel to the canvas (RB-080-50,
//! `board_pan.rs`).
//!
//! What the hook is held to:
//!
//! - **This thread only.** `SetWindowsHookExW` with the UI thread's id and no module: no other
//!   thread and no other process runs it. [`uninstall`] removes it when the main window is
//!   destroyed; Windows would at the thread's end.
//! - **It decides and posts, nothing else.** The procedure reads the message, a few atomics and
//!   thread-locals, CEF's rectangle of the canvas board and the keys' state; it takes no lock
//!   of rBrowser's and reads no settings. The hit test of the strips, the menus, the pan and
//!   the zoom run as tasks of the UI thread.
//! - **It passes everything on** (`CallNextHookEx`) but the middle button's press and release
//!   of a gesture over the canvas, which it takes so no page starts its autoscroll - and gives
//!   back as a click when the pointer did not move.
//! - **Other windows.** The hook sees the mouse of every window of the UI thread: the settings
//!   window, the page windows, the DevTools window, a popped-out view. A right-click opens a
//!   menu only on a label strip, and only when the message went to that strip's own window;
//!   none of those windows has a label strip (a popped-out view's strip stays in the grid, over
//!   its placeholder), so nothing opens there. The middle button and the wheel count only over
//!   the canvas board, which lives in the main window. Everywhere else every message passes
//!   untouched.
//!
//! Win32 has no safe wrapper for the hook, so this file carries the unsafe blocks, each with
//! its SAFETY note.

use std::cell::Cell;

use cef::*;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HC_ACTION, HHOOK, MOUSEHOOKSTRUCTEX, SetWindowsHookExW, UnhookWindowsHookEx,
    WH_MOUSE, WM_MOUSEWHEEL, WM_RBUTTONUP,
};

use super::task::on_ui;
use super::views::core;
use crate::layout::view_menu::{Strip, strip_at};

thread_local! {
    /// This (the UI) thread's hook; 0 without one.
    static HOOK: Cell<usize> = const { Cell::new(0) };
}

/// Starts listening for the mouse on the calling thread, once: the UI thread, when the main
/// window is created. The hook is this thread's alone and goes with [`uninstall`].
#[allow(
    unsafe_code,
    reason = "SetWindowsHookExW has no safe wrapper; CEF reports no mouse button for a Views button"
)]
pub fn install() {
    if HOOK.with(Cell::get) != 0 {
        return;
    }
    super::board_pan::refresh();
    // SAFETY: `on_mouse` is a `HOOKPROC` for `WH_MOUSE`; with a thread id and no module the
    // hook is local to this thread, so it only ever runs on it.
    let hook = unsafe {
        SetWindowsHookExW(
            WH_MOUSE,
            Some(on_mouse),
            std::ptr::null_mut(),
            GetCurrentThreadId(),
        )
    };
    if hook.is_null() {
        tracing::warn!(
            error = %std::io::Error::last_os_error(),
            "no mouse hook: a right-click on a label strip opens no menu, the middle button pans nothing"
        );
        return;
    }
    HOOK.with(|kept| kept.set(hook as usize));
    tracing::info!("the window hears the right and the middle mouse button");
}

/// Takes the hook away again: the main window is gone, the app ends. On the thread that
/// installed it.
#[allow(unsafe_code, reason = "UnhookWindowsHookEx has no safe wrapper")]
pub fn uninstall() {
    let hook = HOOK.with(|kept| kept.replace(0));
    if hook == 0 {
        return;
    }
    // SAFETY: `hook` is the handle SetWindowsHookExW gave this thread, removed once.
    if unsafe { UnhookWindowsHookEx(hook as HHOOK) } == 0 {
        tracing::warn!(error = %std::io::Error::last_os_error(), "the mouse hook was not removed");
    }
}

#[allow(
    unsafe_code,
    reason = "a hook procedure is an unsafe extern function by contract"
)]
unsafe extern "system" fn on_mouse(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // `HC_ACTION`: the message is being removed from the queue - once per message.
    if code == HC_ACTION as i32 && lparam != 0 {
        let message = u32::try_from(wparam).unwrap_or(0);
        // SAFETY: for `WH_MOUSE` with a non-negative code `lparam` points to the message's
        // `MOUSEHOOKSTRUCTEX` (it begins with the `MOUSEHOOKSTRUCT`), valid for this call; its
        // `mouseData` is read only for the wheel, whose turn is its high word.
        let (point, target, wheel) = unsafe {
            let info = &*(lparam as *const MOUSEHOOKSTRUCTEX);
            let wheel = if message == WM_MOUSEWHEEL {
                (info.mouseData >> 16) as u16 as i16
            } else {
                0
            };
            (info.Base.pt, info.Base.hwnd as usize, wheel)
        };
        if message == WM_RBUTTONUP {
            on_ui(move || right_click(point.x, point.y, target));
        } else if super::board_pan::on_mouse(message, point.x, point.y, target, wheel) {
            // The middle button over the canvas (RB-080-50): the page under it never sees it.
            return 1;
        }
    }
    // SAFETY: the arguments are the ones this procedure was called with; the first parameter
    // is ignored by Windows.
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

/// The right button was let go at the screen pixel `x`/`y` over the window `target`: on a
/// label strip its menu opens.
fn right_click(x: i32, y: i32, target: usize) {
    let Some(core) = core() else {
        return;
    };
    if !core.user_settings().ui.view_context_menu {
        return;
    }
    // CEF lays its views out in DIP; the hook reports physical pixels.
    let point = display_convert_screen_point_from_pixels(Some(&Point { x, y }));
    let strips = super::cell_labels::strips();
    let rects: Vec<_> = strips
        .iter()
        .map(|(strip, rect, _)| (*strip, *rect))
        .collect();
    let Some(hit) = strip_at(point.x, point.y, &rects) else {
        tracing::debug!(
            x = point.x,
            y = point.y,
            strips = strips.len(),
            "a right-click hit no label strip"
        );
        return;
    };
    let Some(window) = strips
        .into_iter()
        .find(|(strip, _, _)| *strip == hit)
        .map(|(_, _, window)| window)
    else {
        return;
    };
    // An open menu (a window of its own) may lie over a strip: its click is not the strip's.
    let own = window.window_handle().0 as usize;
    if super::board_pan::root_of(target) != super::board_pan::root_of(own) {
        tracing::info!(strip = ?hit, "a right-click over a label strip went to another window");
        return;
    }
    tracing::info!(strip = ?hit, x = point.x, y = point.y, "a right-click on a label strip");
    match hit {
        Strip::View(view) => super::view_menus::show_view_menu_at(view, &window, point, false),
        Strip::Hidden => super::cell_labels::show_add_menu_at(&window, point, None),
    }
}

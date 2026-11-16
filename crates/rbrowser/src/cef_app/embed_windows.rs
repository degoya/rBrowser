//! The Win32 calls of the owned overlay (`embed.rs`, RB-080-39): the top-level windows of a
//! process, a window's styles and owner, its region and its place. Win32 has no safe wrapper
//! for these, so this file carries the unsafe blocks, each with its SAFETY note. Every call
//! that reaches the other process's window goes asynchronously (`SWP_ASYNCWINDOWPOS`), so a
//! Firefox or WebKit that does not answer never holds the UI thread.

use windows_sys::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{ClientToScreen, CreateRectRgn, SetWindowRgn};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GW_OWNER, GWL_EXSTYLE, GWL_STYLE, GWLP_HWNDPARENT,
    GetClassNameW, GetMenu, GetParent, GetWindow, GetWindowLongPtrW, GetWindowRect,
    GetWindowThreadProcessId, HMENU, IsWindow, IsWindowVisible, SWP_ASYNCWINDOWPOS,
    SWP_FRAMECHANGED, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE,
    SWP_NOZORDER, SWP_SHOWWINDOW, SetMenu, SetWindowLongPtrW, SetWindowPos,
};

use rb_engine::external::{Candidate, WindowRect, WindowStyle};

/// The longest window class name read.
const CLASS_NAME_MAX: usize = 256;

/// What `EnumWindows` collects.
struct Finder {
    /// The processes whose windows count.
    process_ids: Vec<u32>,
    /// The class name as UTF-16, without the terminator; `None` takes every class.
    class: Option<Vec<u16>>,
    found: Vec<Candidate>,
}

/// The visible, unowned top-level windows of the processes `process_ids` (of class `class`
/// when given), in z-order, top-most first.
#[allow(
    unsafe_code,
    reason = "EnumWindows and the window queries have no safe wrapper"
)]
pub fn top_level_windows_of(process_ids: &[u32], class: Option<&str>) -> Vec<Candidate> {
    let mut finder = Finder {
        process_ids: process_ids.to_vec(),
        class: class.map(|class| class.encode_utf16().collect()),
        found: Vec::new(),
    };
    let pointer: *mut Finder = &mut finder;
    // SAFETY: `collect` reads `pointer` as the `Finder` it is, which outlives the call;
    // EnumWindows returns before `finder` is dropped.
    unsafe { EnumWindows(Some(collect), pointer as LPARAM) };
    finder.found
}

#[allow(
    unsafe_code,
    reason = "the EnumWindows callback is an unsafe extern function by contract"
)]
unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> i32 {
    // SAFETY: `lparam` is the `Finder` pointer `top_level_windows_of` passed, valid for the
    // whole enumeration, and nothing else holds it meanwhile.
    let finder = unsafe { &mut *(lparam as *mut Finder) };
    let mut process_id = 0u32;
    // SAFETY: `hwnd` is the window Windows enumerates; `process_id` a valid out pointer.
    unsafe { GetWindowThreadProcessId(hwnd, &mut process_id) };
    if !finder.process_ids.contains(&process_id) {
        return 1;
    }
    // SAFETY: `hwnd` as above; both calls only read.
    let (visible, owner) = unsafe { (IsWindowVisible(hwnd), GetWindow(hwnd, GW_OWNER)) };
    if visible == 0 || !owner.is_null() {
        return 1;
    }
    if let Some(class) = &finder.class {
        let mut name = [0u16; CLASS_NAME_MAX];
        // SAFETY: `name` holds CLASS_NAME_MAX UTF-16 units, the size handed over.
        let len = unsafe { GetClassNameW(hwnd, name.as_mut_ptr(), CLASS_NAME_MAX as i32) };
        let len = usize::try_from(len).unwrap_or(0);
        if name.get(..len) != Some(class.as_slice()) {
            return 1;
        }
    }
    if let Some(rect) = window_rect(hwnd) {
        finder.found.push(Candidate {
            handle: hwnd as usize,
            rect,
        });
    }
    1
}

#[allow(unsafe_code, reason = "GetWindowRect has no safe wrapper")]
fn window_rect(hwnd: HWND) -> Option<WindowRect> {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: `hwnd` is a window handle; `rect` a valid out pointer.
    if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
        return None;
    }
    Some(WindowRect {
        x: rect.left,
        y: rect.top,
        width: u32::try_from(rect.right - rect.left).ok()?,
        height: u32::try_from(rect.bottom - rect.top).ok()?,
    })
}

/// What `EnumChildWindows` collects: the direct children of `parent`, in its client
/// coordinates.
struct Children {
    parent: HWND,
    origin: POINT,
    found: Vec<WindowRect>,
}

/// The rectangles of the window's direct child windows, in its client coordinates (the
/// shell's toolbar and web view, RB-080-40).
#[allow(
    unsafe_code,
    reason = "EnumChildWindows and the window queries have no safe wrapper"
)]
pub fn child_rects(handle: usize) -> Vec<WindowRect> {
    let parent = handle as HWND;
    let mut origin = POINT { x: 0, y: 0 };
    // SAFETY: `parent` is a window handle; `origin` a valid out pointer.
    unsafe { ClientToScreen(parent, &mut origin) };
    let mut children = Children {
        parent,
        origin,
        found: Vec::new(),
    };
    let pointer: *mut Children = &mut children;
    // SAFETY: `collect_child` reads `pointer` as the `Children` it is, which outlives the
    // call; EnumChildWindows returns before `children` is dropped.
    unsafe { EnumChildWindows(parent, Some(collect_child), pointer as LPARAM) };
    children.found
}

#[allow(
    unsafe_code,
    reason = "the EnumChildWindows callback is an unsafe extern function by contract"
)]
unsafe extern "system" fn collect_child(hwnd: HWND, lparam: LPARAM) -> i32 {
    // SAFETY: `lparam` is the `Children` pointer `child_rects` passed, valid for the whole
    // enumeration, and nothing else holds it meanwhile.
    let children = unsafe { &mut *(lparam as *mut Children) };
    // SAFETY: `hwnd` is the window Windows enumerates; GetParent only reads.
    if unsafe { GetParent(hwnd) } != children.parent {
        return 1;
    }
    if let Some(rect) = window_rect(hwnd) {
        children.found.push(WindowRect {
            x: rect.x.saturating_sub(children.origin.x),
            y: rect.y.saturating_sub(children.origin.y),
            width: rect.width,
            height: rect.height,
        });
    }
    1
}

/// Takes the window's menu bar away (the shell's File, View, ... menu, RB-080-40) and answers
/// it, so `set_menu` can give it back; 0 for a window without one.
#[allow(unsafe_code, reason = "GetMenu and SetMenu have no safe wrapper")]
pub fn remove_menu(handle: usize) -> usize {
    // SAFETY: GetMenu only reads; SetMenu with a null menu takes the bar away. Both take a
    // window handle; a window gone since makes them fail, nothing more.
    unsafe {
        let menu = GetMenu(handle as HWND);
        if !menu.is_null() {
            SetMenu(handle as HWND, std::ptr::null_mut());
        }
        menu as usize
    }
}

/// Gives the window its menu bar `menu` back (nothing for 0).
#[allow(unsafe_code, reason = "SetMenu has no safe wrapper")]
pub fn set_menu(handle: usize, menu: usize) {
    if menu == 0 {
        return;
    }
    // SAFETY: a window handle and the menu handle `remove_menu` answered for it.
    unsafe { SetMenu(handle as HWND, menu as HMENU) };
}

/// Whether `handle` is still a window.
#[allow(unsafe_code, reason = "IsWindow has no safe wrapper")]
pub fn is_window(handle: usize) -> bool {
    // SAFETY: IsWindow only reads; any value is a valid argument.
    unsafe { IsWindow(handle as HWND) != 0 }
}

/// The window's styles.
#[allow(unsafe_code, reason = "GetWindowLongPtrW has no safe wrapper")]
pub fn style(handle: usize) -> WindowStyle {
    // SAFETY: GetWindowLongPtrW only reads; a window gone since answers 0.
    let (style, ex_style) = unsafe {
        (
            GetWindowLongPtrW(handle as HWND, GWL_STYLE),
            GetWindowLongPtrW(handle as HWND, GWL_EXSTYLE),
        )
    };
    WindowStyle {
        style: style as u32,
        ex_style: ex_style as u32,
    }
}

/// Sets the window's styles; `frame_changed` makes them show.
#[allow(unsafe_code, reason = "SetWindowLongPtrW has no safe wrapper")]
pub fn set_style(handle: usize, style: WindowStyle) {
    let (style, ex_style) = (
        isize::try_from(style.style).unwrap_or(0),
        isize::try_from(style.ex_style).unwrap_or(0),
    );
    // SAFETY: style bits of a window handle; a window gone since makes the calls fail, nothing
    // more.
    unsafe {
        SetWindowLongPtrW(handle as HWND, GWL_STYLE, style);
        SetWindowLongPtrW(handle as HWND, GWL_EXSTYLE, ex_style);
    }
}

/// Makes `owner` the window's owner (`0`: none): an owned window stays above its owner, hides
/// with it when it is minimized and never shows on the taskbar by itself.
#[allow(unsafe_code, reason = "SetWindowLongPtrW has no safe wrapper")]
pub fn set_owner(handle: usize, owner: usize) {
    // SAFETY: GWLP_HWNDPARENT takes a window handle (or 0) as the value.
    unsafe { SetWindowLongPtrW(handle as HWND, GWLP_HWNDPARENT, owner as isize) };
}

/// Tells the window its frame styles changed.
#[allow(unsafe_code, reason = "SetWindowPos has no safe wrapper")]
pub fn frame_changed(handle: usize) {
    // SAFETY: a window handle and flags; no pointers.
    unsafe {
        SetWindowPos(
            handle as HWND,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED
                | SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOZORDER
                | SWP_NOOWNERZORDER
                | SWP_NOACTIVATE
                | SWP_ASYNCWINDOWPOS,
        )
    };
}

/// Clips the window to `region` (in its own coordinates), or shows all of it with `None`. The
/// system owns the region after the call.
#[allow(
    unsafe_code,
    reason = "CreateRectRgn and SetWindowRgn have no safe wrapper"
)]
pub fn set_region(handle: usize, region: Option<WindowRect>) {
    let region = match region {
        Some(rect) => {
            let right = rect
                .x
                .saturating_add(i32::try_from(rect.width).unwrap_or(i32::MAX));
            let bottom = rect
                .y
                .saturating_add(i32::try_from(rect.height).unwrap_or(i32::MAX));
            // SAFETY: four coordinates; the region handle goes to SetWindowRgn, which owns it.
            unsafe { CreateRectRgn(rect.x, rect.y, right, bottom) }
        }
        None => std::ptr::null_mut(),
    };
    // SAFETY: a window handle and a region handle (or null); a failed call leaves the region
    // to the system either way.
    unsafe { SetWindowRgn(handle as HWND, region, 1) };
}

/// Moves and sizes the window to `rect` and shows it, without activating it or changing its
/// place in the z-order (an owned window stays above its owner).
#[allow(unsafe_code, reason = "SetWindowPos has no safe wrapper")]
pub fn place(handle: usize, rect: WindowRect) {
    // SAFETY: a window handle, coordinates and flags; no pointers.
    unsafe {
        SetWindowPos(
            handle as HWND,
            std::ptr::null_mut(),
            rect.x,
            rect.y,
            i32::try_from(rect.width).unwrap_or(i32::MAX),
            i32::try_from(rect.height).unwrap_or(i32::MAX),
            SWP_SHOWWINDOW | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
        )
    };
}

/// Hides the window where it is.
pub fn hide(handle: usize) {
    show_or_hide(handle, SWP_HIDEWINDOW);
}

/// Shows the window where it is, without activating it.
pub fn show(handle: usize) {
    show_or_hide(handle, SWP_SHOWWINDOW);
}

#[allow(unsafe_code, reason = "SetWindowPos has no safe wrapper")]
fn show_or_hide(handle: usize, flag: u32) {
    // SAFETY: a window handle and flags; no pointers.
    unsafe {
        SetWindowPos(
            handle as HWND,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            flag | SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOZORDER
                | SWP_NOOWNERZORDER
                | SWP_NOACTIVATE
                | SWP_ASYNCWINDOWPOS,
        )
    };
}

//! An external view in its grid cell (RB-080-39): the pure half of the owned overlay. The
//! window of a Firefox or WebKit view gets the main window as its owner, loses its frame
//! ([`embedded_style`]) and is kept over its cell's screen rectangle, clipped to the grid area
//! ([`place_in_cell`]); the engine names its window by process and class ([`NativeWindow`]) and
//! the app picks the window among the process's ([`pick_window`]). A shell that keeps a toolbar
//! above its page (Playwright's WebKit, RB-080-40) is moved up by the toolbar's height
//! ([`content_inset`]) and the toolbar clipped away. The Win32 calls live in the app's
//! `cfg(windows)` module; everything here compiles everywhere and is tested here.

use super::placement::WindowRect;

/// `WS_POPUP`: a window without the overlapped frame.
pub const WS_POPUP: u32 = 0x8000_0000;
/// `WS_CAPTION`: the title bar (`WS_BORDER | WS_DLGFRAME`).
pub const WS_CAPTION: u32 = 0x00C0_0000;
/// `WS_SYSMENU`: the window menu on the title bar.
pub const WS_SYSMENU: u32 = 0x0008_0000;
/// `WS_THICKFRAME`: the sizing border.
pub const WS_THICKFRAME: u32 = 0x0004_0000;
/// `WS_MINIMIZEBOX`.
pub const WS_MINIMIZEBOX: u32 = 0x0002_0000;
/// `WS_MAXIMIZEBOX`.
pub const WS_MAXIMIZEBOX: u32 = 0x0001_0000;
/// `WS_EX_TOOLWINDOW`: not on the taskbar, not in Alt+Tab.
pub const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
/// `WS_EX_APPWINDOW`: forced onto the taskbar.
pub const WS_EX_APPWINDOW: u32 = 0x0004_0000;
/// `WS_EX_WINDOWEDGE`: the raised border edge.
pub const WS_EX_WINDOWEDGE: u32 = 0x0000_0100;
/// `WS_EX_CLIENTEDGE`: the sunken client edge.
pub const WS_EX_CLIENTEDGE: u32 = 0x0000_0200;
/// `WS_EX_DLGMODALFRAME`: the double border.
pub const WS_EX_DLGMODALFRAME: u32 = 0x0000_0001;
/// `WS_EX_STATICEDGE`: the three-dimensional border.
pub const WS_EX_STATICEDGE: u32 = 0x0002_0000;

/// The frame bits of an overlapped window, all gone from an embedded one.
const FRAME: u32 = WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
/// The extended edge bits, all gone from an embedded window.
const EDGES: u32 =
    WS_EX_APPWINDOW | WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE | WS_EX_DLGMODALFRAME | WS_EX_STATICEDGE;

/// A window's `GWL_STYLE` and `GWL_EXSTYLE`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowStyle {
    /// `GWL_STYLE`.
    pub style: u32,
    /// `GWL_EXSTYLE`.
    pub ex_style: u32,
}

/// The style of `normal` as an embedded window: a frameless popup (no caption, sizing border,
/// boxes or system menu) that is a tool window (off the taskbar); every other bit stays. The
/// app keeps `normal` to give the window back when it is released.
pub fn embedded_style(normal: WindowStyle) -> WindowStyle {
    WindowStyle {
        style: (normal.style & !FRAME) | WS_POPUP,
        ex_style: (normal.ex_style & !EDGES) | WS_EX_TOOLWINDOW,
    }
}

/// Whether `style` is an embedded window's.
pub fn is_embedded(style: WindowStyle) -> bool {
    style.style & WS_POPUP != 0
        && style.style & FRAME == 0
        && style.ex_style & WS_EX_TOOLWINDOW != 0
}

/// How an engine names the native window of a view, so the app can find it: the process that
/// owns it and, when the engine knows it, its window class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeWindow {
    /// The process id of the window's owner (the engine's UI process).
    pub process_id: u32,
    /// The window class (`MozillaWindowClass` for Firefox); `None` takes any top-level window.
    pub window_class: Option<String>,
}

/// Where an embedded window goes, in physical screen pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbedPlace {
    /// Not shown: its cell is hidden, off screen or wholly outside the grid area.
    Hidden,
    /// Over its cell.
    Shown {
        /// The window's outer rectangle: the cell's view area, plus the toolbar's height above
        /// it when the shell has one.
        rect: WindowRect,
        /// The visible part when the cell reaches outside the grid area (the canvas) or the
        /// window has a toolbar, in the window's own coordinates; `None` when the whole window
        /// shows.
        region: Option<WindowRect>,
    },
}

/// The place of a window over `cell` (the cell's view area, screen px) inside `area` (the grid
/// area, screen px): the window at the cell, clipped to the area with a region when the cell
/// reaches outside it, hidden when nothing of it lies inside or `shown` is false. A window
/// whose page starts `inset` px below its top (the shell's toolbar, [`content_inset`]) goes up
/// by that much and the region leaves the toolbar out, so the cell shows the page alone.
pub fn place_in_cell(cell: WindowRect, area: WindowRect, shown: bool, inset: u32) -> EmbedPlace {
    if !shown {
        return EmbedPlace::Hidden;
    }
    let Some(visible) = intersection(cell, area) else {
        return EmbedPlace::Hidden;
    };
    let inset_px = i32::try_from(inset).unwrap_or(i32::MAX);
    let rect = WindowRect {
        x: cell.x,
        y: cell.y.saturating_sub(inset_px),
        width: cell.width,
        height: cell.height.saturating_add(inset),
    };
    let region = (visible != cell || inset > 0).then(|| WindowRect {
        x: visible.x.saturating_sub(cell.x),
        y: visible.y.saturating_sub(cell.y).saturating_add(inset_px),
        width: visible.width,
        height: visible.height,
    });
    EmbedPlace::Shown { rect, region }
}

/// How far below the window's top its page starts: the top of the largest of `children` (the
/// window's direct child windows, in client coordinates). Playwright's WebKit shell keeps a
/// toolbar across the top and the web view below it; Firefox's page is the window. 0 without
/// children.
pub fn content_inset(children: &[WindowRect]) -> u32 {
    children
        .iter()
        .max_by_key(|child| u64::from(child.width) * u64::from(child.height))
        .map_or(0, |content| u32::try_from(content.y).unwrap_or(0))
}

/// A top-level window of the engine's process the app found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// The native handle.
    pub handle: usize,
    /// Its outer rectangle, screen px.
    pub rect: WindowRect,
}

/// The window of a view among `candidates` (the engine process's visible top-level windows in
/// z-order, top-most first): never one in `taken` (bound to another view); among several free
/// ones the one nearest `expected` in place and size (where the engine put the view's window,
/// at the breakpoint's size - a popup or an inspector of the same process differs in size,
/// another view's window in place), else the first.
pub fn pick_window(
    candidates: &[Candidate],
    taken: &[usize],
    expected: Option<WindowRect>,
) -> Option<usize> {
    let free: Vec<&Candidate> = candidates
        .iter()
        .filter(|candidate| !taken.contains(&candidate.handle))
        .collect();
    let picked: &Candidate = match (free.as_slice(), expected) {
        ([], _) => return None,
        ([one], _) | ([one, ..], None) => one,
        (several, Some(expected)) => several
            .iter()
            .copied()
            .min_by_key(|candidate| distance(candidate.rect, expected))?,
    };
    Some(picked.handle)
}

fn distance(a: WindowRect, b: WindowRect) -> u64 {
    let dx = i64::from(a.x) - i64::from(b.x);
    let dy = i64::from(a.y) - i64::from(b.y);
    let dw = i64::from(a.width) - i64::from(b.width);
    let dh = i64::from(a.height) - i64::from(b.height);
    [dx, dy, dw, dh]
        .into_iter()
        .map(i64::unsigned_abs)
        .fold(0, u64::saturating_add)
}

fn intersection(a: WindowRect, b: WindowRect) -> Option<WindowRect> {
    let left = i64::from(a.x).max(i64::from(b.x));
    let top = i64::from(a.y).max(i64::from(b.y));
    let right = (i64::from(a.x) + i64::from(a.width)).min(i64::from(b.x) + i64::from(b.width));
    let bottom = (i64::from(a.y) + i64::from(a.height)).min(i64::from(b.y) + i64::from(b.height));
    if right <= left || bottom <= top {
        return None;
    }
    Some(WindowRect {
        x: i32::try_from(left).ok()?,
        y: i32::try_from(top).ok()?,
        width: u32::try_from(right - left).ok()?,
        height: u32::try_from(bottom - top).ok()?,
    })
}

#[cfg(test)]
#[path = "embed_tests.rs"]
mod tests;

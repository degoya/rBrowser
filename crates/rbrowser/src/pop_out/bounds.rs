//! Where a popped-out view's window goes and how large it is (RB-080-14), in DIP of the screen,
//! the unit of CEF Views: a breakpoint of 1920 CSS px is 1920 DIP at zoom 1, whatever the
//! monitor's scale (a screenshot then has CSS px x the monitor's DPR pixels). Pure: the window
//! side measures the work area and the frame and asks [`place`].
//!
//! The page keeps its emulated width in the window ([`metrics`]): a window narrower than the
//! breakpoint (the screen is too small, the person made it smaller) shows it scaled down to fit,
//! never up, and the cell's zoom does not count there.

use rb_config::Zoom;
use rb_engine::{CellRect, Emulation};

use crate::fit::{DeviceMetrics, device_metrics};
pub use crate::page_window::ScreenRect;

/// The i18n key of the window title at the real size; `{label}`, `{width}`, `{height}`.
pub const TITLE_KEY: &str = "popout.window.title";
/// The title when the screen only allowed a smaller zoom; also `{percent}`.
pub const TITLE_ZOOMED_KEY: &str = "popout.window.title_zoomed";
/// The title when the page keeps its width but the screen cut its height.
pub const TITLE_CUT_KEY: &str = "popout.window.title_cut";
/// The title when the window is larger than the screen (`popOut.fitToScreen` off).
pub const TITLE_LARGER_KEY: &str = "popout.window.title_larger";
/// The widest frame side taken as real; anything above is a measurement of a window not shown.
const MAX_INSET: i32 = 128;

/// The window frame around the client area, in DIP.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Insets {
    /// The title bar and the top border.
    pub top: i32,
    /// The left border.
    pub left: i32,
    /// The bottom border.
    pub bottom: i32,
    /// The right border.
    pub right: i32,
}

/// What [`place`] needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaceInput {
    /// The emulated viewport in CSS px (the breakpoint's width and height).
    pub viewport: (u32, u32),
    /// The work area of the screen the window goes to (without the taskbar), DIP.
    pub work_area: ScreenRect,
    /// The window's frame.
    pub insets: Insets,
    /// The client area the window had last time for this breakpoint (`popOut.rememberBounds`).
    pub remembered: Option<ScreenRect>,
    /// `popOut.fitToScreen`: keep the window inside the work area, at a zoom below 1 if need be.
    pub fit_to_screen: bool,
}

/// Where the window goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// The window with its frame, screen DIP.
    pub outer: ScreenRect,
    /// The client area: the view.
    pub client: ScreenRect,
    /// The page's zoom in the client area: 1 at the real size, below when it is narrower.
    pub zoom: f64,
    /// The real size does not fit the work area.
    pub exceeds_screen: bool,
}

/// The window for `input`: the real size (or the remembered client area, never wider than the
/// page), centred in the work area unless remembered; with `fit_to_screen` cut to the work area
/// and kept inside it, else only its title bar kept on the screen. No side is ever 0 (CEF takes
/// a size with a side of 0 for empty).
pub fn place(input: &PlaceInput) -> Placement {
    let insets = input.insets;
    let work = input.work_area;
    let page_width = side(input.viewport.0);
    let page_height = side(input.viewport.1);
    let available_width = (work.width - insets.left - insets.right).max(1);
    let available_height = (work.height - insets.top - insets.bottom).max(1);
    let exceeds_screen = page_width > available_width || page_height > available_height;
    let (mut width, mut height) = match input.remembered {
        Some(rect) => (rect.width.clamp(1, page_width), rect.height.max(1)),
        None => (page_width, page_height),
    };
    if input.fit_to_screen {
        width = width.min(available_width);
        height = height.min(available_height);
    }
    let min_x = work.x + insets.left;
    let min_y = work.y + insets.top;
    let (mut x, mut y) = match input.remembered {
        Some(rect) => (rect.x, rect.y),
        None => (
            min_x + (available_width - width) / 2,
            min_y + (available_height - height) / 2,
        ),
    };
    if input.fit_to_screen {
        x = x.min(min_x + available_width - width);
        y = y.min(min_y + available_height - height);
    } else {
        // At least the title bar on the screen, so the window can be moved.
        x = x.min(work.x + work.width - insets.left - 1);
        y = y.min(work.y + work.height - 1);
    }
    let client = ScreenRect {
        x: x.max(min_x),
        y: y.max(min_y),
        width,
        height,
    };
    Placement {
        outer: ScreenRect {
            x: client.x - insets.left,
            y: client.y - insets.top,
            width: client.width + insets.left + insets.right,
            height: client.height + insets.top + insets.bottom,
        },
        client,
        zoom: (f64::from(width) / f64::from(page_width)).min(1.0),
        exceeds_screen,
    }
}

/// The frame of a window whose outer bounds are `outer` and client area `client`; no frame when
/// the two do not nest (a window not laid out yet).
pub fn frame_insets(outer: ScreenRect, client: ScreenRect) -> Insets {
    let insets = Insets {
        top: client.y - outer.y,
        left: client.x - outer.x,
        bottom: (outer.y + outer.height) - (client.y + client.height),
        right: (outer.x + outer.width) - (client.x + client.width),
    };
    let sides = [insets.top, insets.left, insets.bottom, insets.right];
    if sides.iter().all(|side| (0..=MAX_INSET).contains(side)) {
        insets
    } else {
        Insets::default()
    }
}

/// A rectangle in physical pixels as DIP at the monitor's `scale_factor` (1.5 at 150 %).
pub fn dip_area(pixels: ScreenRect, scale_factor: f64) -> ScreenRect {
    let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    // Screen coordinates divided by a factor >= some small positive number stay in i32 range
    // for any real monitor; `as` saturates otherwise.
    let dip = |value: i32| (f64::from(value) / scale).round() as i32;
    ScreenRect {
        x: dip(pixels.x),
        y: dip(pixels.y),
        width: dip(pixels.width),
        height: dip(pixels.height),
    }
}

/// The title key for `placement` (`fit_to_screen` as placed).
pub fn title_key(placement: &Placement, fit_to_screen: bool) -> &'static str {
    if placement.zoom < 1.0 - f64::EPSILON {
        TITLE_ZOOMED_KEY
    } else if placement.exceeds_screen && fit_to_screen {
        TITLE_CUT_KEY
    } else if placement.exceeds_screen {
        TITLE_LARGER_KEY
    } else {
        TITLE_KEY
    }
}

/// `zoom` in whole percent for the title.
pub fn percent(zoom: f64) -> u32 {
    // A zoom is in (0, 1].
    (zoom * 100.0).round().clamp(1.0, 100.0) as u32
}

/// The device metrics of a popped-out view in its window `client` (DIP, client area): the
/// emulated width at the largest scale up to 1 that fits the window, the height filling it. The
/// cell's zoom, a zoom step and `emulation.scale` do not apply there.
pub fn metrics(emulation: &Emulation, client: CellRect) -> DeviceMetrics {
    let unscaled = Emulation {
        scale: None,
        ..emulation.clone()
    };
    device_metrics(&unscaled, client, Zoom::Fit)
}

/// `rect` with its height changed by `by` DIP, at least 1: a window's client area with (`by`
/// positive) or without its view's footer (RB-080-44). The bounds kept for a breakpoint are
/// the page's, so they hold whether the footer is shown or not.
pub fn taller(rect: ScreenRect, by: i32) -> ScreenRect {
    ScreenRect {
        height: rect.height.saturating_add(by).max(1),
        ..rect
    }
}

/// A side in DIP: at least 1, at most what an `i32` holds.
fn side(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX).max(1)
}

#[cfg(test)]
#[path = "bounds_tests.rs"]
mod tests;

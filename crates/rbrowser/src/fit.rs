//! Fitting a breakpoint into its cell (ADR 0001, Decision 3): the device metrics of
//! `Emulation.setDeviceMetricsOverride`, the zoom steps, and the mapping between a point in a
//! view (widget DIP) and the page (CSS px).
//!
//! The emulated viewport is the breakpoint's width at `scale`; its height fills the cell
//! (`cell height / scale`), so a view never shows an empty band or hides part of its viewport.
//! CDP `Input.*` coordinates are widget DIP once `scale` is not 1, so every input rBrowser sends
//! is multiplied by the view's scale ([`scale_input`]).

use rb_config::Zoom;
use rb_engine::{CellRect, Emulation};
use serde_json::{Value, json};

/// The smallest fixed zoom of a cell.
pub const MIN_ZOOM: f64 = 0.25;
/// The largest fixed zoom of a cell.
pub const MAX_ZOOM: f64 = 1.0;
/// The largest scale of a view on the canvas (RB-080-94): its page at 200 % - the canvas zooms
/// past its page's size, the grid never does ([`MAX_ZOOM`]).
pub const MAX_CANVAS_SCALE: f64 = 2.0;
/// The zoom steps of `view.zoom_in` / `view.zoom_out`.
pub const ZOOM_STEPS: [f64; 7] = [0.25, 0.33, 0.5, 0.67, 0.75, 0.9, 1.0];
/// The smallest scale ever sent; CDP refuses 0, and a zero-width cell would ask for it.
const MIN_SCALE: f64 = 0.01;

/// What `Emulation.setDeviceMetricsOverride` gets for one view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceMetrics {
    /// Viewport width in CSS px: the breakpoint's.
    pub width: u32,
    /// Viewport height in CSS px: the cell's height at `scale`.
    pub height: u32,
    /// Device pixel ratio; `0` keeps the display's own.
    pub dpr: f64,
    /// Mobile device type.
    pub mobile: bool,
    /// The scale of the viewport into the cell.
    pub scale: f64,
}

impl DeviceMetrics {
    /// The parameters of `Emulation.setDeviceMetricsOverride`.
    pub fn to_params(&self) -> Value {
        json!({
            "width": self.width,
            "height": self.height,
            "deviceScaleFactor": self.dpr,
            "mobile": self.mobile,
            "scale": self.scale,
        })
    }
}

/// The scale of `emulation` in a cell of `cell` with `zoom`: an explicit `emulation.scale`
/// first, then a fixed zoom (clamped to [`MIN_ZOOM`]..=[`MAX_ZOOM`]), else fit to the width
/// (`min(1, cell width / viewport width)`).
pub fn scale_for(emulation: &Emulation, cell: CellRect, zoom: Zoom) -> f64 {
    scale_up_to(emulation, cell, zoom, MAX_ZOOM)
}

/// [`scale_for`] with fit to the width up to `max` rather than 1: a canvas view fills its
/// rectangle past its page's size ([`MAX_CANVAS_SCALE`]).
fn scale_up_to(emulation: &Emulation, cell: CellRect, zoom: Zoom, max: f64) -> f64 {
    let scale = match (emulation.scale, zoom) {
        (Some(scale), _) => scale,
        (None, Zoom::Factor(factor)) => factor.clamp(MIN_ZOOM, MAX_ZOOM),
        (None, Zoom::Fit) if emulation.width == 0 => 1.0,
        (None, Zoom::Fit) => (f64::from(cell.width) / f64::from(emulation.width)).min(max),
    };
    if scale.is_finite() {
        scale.clamp(MIN_SCALE, max.max(MAX_ZOOM))
    } else {
        1.0
    }
}

/// The device metrics of `emulation` in `cell`. A cell without height (not laid out yet) keeps
/// the emulation's own height.
pub fn device_metrics(emulation: &Emulation, cell: CellRect, zoom: Zoom) -> DeviceMetrics {
    device_metrics_up_to(emulation, cell, zoom, MAX_ZOOM)
}

/// [`device_metrics`] fitted up to the scale `max`: a view on the canvas ([`MAX_CANVAS_SCALE`]).
pub fn device_metrics_up_to(
    emulation: &Emulation,
    cell: CellRect,
    zoom: Zoom,
    max: f64,
) -> DeviceMetrics {
    let scale = scale_up_to(emulation, cell, zoom, max);
    let height = if cell.height == 0 {
        emulation.height
    } else {
        // At most a few thousand DIP over a scale >= 0.01: always inside u32.
        (f64::from(cell.height) / scale).round().max(1.0) as u32
    };
    DeviceMetrics {
        width: emulation.width.max(1),
        height,
        dpr: if emulation.dpr > 0.0 {
            emulation.dpr
        } else {
            0.0
        },
        mobile: emulation.mobile,
        scale,
    }
}

/// The next zoom step above (`up`) or below `current`; stays at the ends.
pub fn zoom_step(current: f64, up: bool) -> f64 {
    const EPSILON: f64 = 0.005;
    if up {
        ZOOM_STEPS
            .iter()
            .copied()
            .find(|step| *step > current + EPSILON)
            .unwrap_or(MAX_ZOOM)
    } else {
        ZOOM_STEPS
            .iter()
            .rev()
            .copied()
            .find(|step| *step < current - EPSILON)
            .unwrap_or(MIN_ZOOM)
    }
}

/// The size a strip's view delegate reports (`preferred_size`, `minimum_size`): `width` x
/// `height` with each side at least 1. CEF takes a size with a side of 0 for empty
/// (`CefSize::IsEmpty`) and falls back to the view's own size, which is 0 for a browser view, so
/// a strip of `{ width: 0, height: 44 }` got no height. The 1 on the free side is stretched away
/// by the box layout's cross axis.
pub fn strip_size(width: i32, height: i32) -> (i32, i32) {
    (width.max(1), height.max(1))
}

/// A point in the view (widget DIP, relative to its top left) to the page (CSS px).
pub fn view_to_page(x: f64, y: f64, scale: f64) -> (f64, f64) {
    (x / scale, y / scale)
}

/// A point in the page (CSS px) to the view (widget DIP).
pub fn page_to_view(x: f64, y: f64, scale: f64) -> (f64, f64) {
    (x * scale, y * scale)
}

/// The keys of an `Input.*` command that are coordinates or distances.
const SCALED_KEYS: [&str; 6] = ["x", "y", "deltaX", "deltaY", "xDistance", "yDistance"];
/// The keys of a touch point that are coordinates or radii.
const SCALED_TOUCH_KEYS: [&str; 4] = ["x", "y", "radiusX", "radiusY"];

/// Converts the CSS px coordinates of an `Input.*` command to widget DIP by multiplying them
/// with `scale` (ADR 0001, Decision 3). Other methods are left alone.
pub fn scale_input(method: &str, params: &mut Value, scale: f64) {
    if !method.starts_with("Input.") || (scale - 1.0).abs() < f64::EPSILON {
        return;
    }
    let Some(object) = params.as_object_mut() else {
        return;
    };
    for key in SCALED_KEYS {
        multiply(object.get_mut(key), scale);
    }
    if let Some(Value::Array(points)) = object.get_mut("touchPoints") {
        for point in points.iter_mut().filter_map(Value::as_object_mut) {
            for key in SCALED_TOUCH_KEYS {
                multiply(point.get_mut(key), scale);
            }
        }
    }
}

fn multiply(value: Option<&mut Value>, scale: f64) {
    if let Some(value) = value
        && let Some(number) = value.as_f64()
    {
        *value = json!(number * scale);
    }
}

#[cfg(test)]
#[path = "fit_tests.rs"]
mod tests;

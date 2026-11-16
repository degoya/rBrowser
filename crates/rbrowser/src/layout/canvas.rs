//! The canvas mode: views placed freely on an unbounded canvas, looked at through a camera
//! (pan and global zoom), snapping to a grid and to the edges of other views.
//!
//! An item's `x`/`y` is the top-left corner of its view in canvas DIP; its label strip sits in
//! the [`LABEL`]-tall band above it, which counts as part of the view when snapping, so two
//! snapped views never cover each other's label. On screen a view is its breakpoint's size times
//! its own zoom times the camera's - the page's zoom, up to [`MAX_PAGE_ZOOM`] (RB-080-94). A view that reaches past the grid area keeps its rectangle
//! and is shown - the window clips it to the area (`cef_app/placed.rs`, RB-080-55); one wholly
//! outside is hidden, and with `layoutModes.showPartialViews` off every view not wholly inside.

use rb_config::{CanvasItem, default_item_zoom};
use rb_engine::CellRect;

use super::{Item, Slot, contains, overlaps, split_label, to_dip};

/// The camera's zoom range: up to the zoom at which a view of the smallest item zoom
/// (`rb_config::CANVAS_ZOOM_RANGE`, 0.1) shows its page at 100 % (RB-080-94). What a canvas
/// may use of it is [`camera_limit`].
pub const ZOOM_RANGE: (f64, f64) = (0.05, 10.0);
/// The largest zoom a page is shown at on the canvas (camera x item zoom): 200 %. Beyond, a view
/// of 1920 DIP would be wider than 3840 DIP, larger than any screen it is looked at on; and the
/// view's rendering grows with it (`crate::fit::MAX_CANVAS_SCALE`).
pub const MAX_PAGE_ZOOM: f64 = crate::fit::MAX_CANVAS_SCALE;
/// The largest zoom fitting every view picks: a lone small view is not blown up.
pub const FIT_MAX: f64 = 1.0;

/// A rectangle on the canvas, in canvas DIP.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorldRect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl WorldRect {
    fn right(&self) -> f64 {
        self.x + self.width
    }

    fn bottom(&self) -> f64 {
        self.y + self.height
    }
}

/// What the grid area shows of the canvas: the canvas point at its top-left corner and the zoom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Canvas x at the area's left edge.
    pub x: f64,
    /// Canvas y at the area's top edge.
    pub y: f64,
    /// Screen DIP per canvas DIP.
    pub zoom: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

impl Camera {
    /// The camera with its zoom held to `max` at most (and to [`ZOOM_RANGE`]).
    pub fn within(self, max: f64) -> Self {
        let camera = self.clamped();
        Self {
            zoom: camera.zoom.min(max),
            ..camera
        }
        .clamped()
    }

    /// The camera with its zoom held to [`ZOOM_RANGE`].
    pub fn clamped(self) -> Self {
        let zoom = if self.zoom.is_finite() {
            self.zoom.clamp(ZOOM_RANGE.0, ZOOM_RANGE.1)
        } else {
            1.0
        };
        let finite = |value: f64| if value.is_finite() { value } else { 0.0 };
        Self {
            x: finite(self.x),
            y: finite(self.y),
            zoom,
        }
    }
}

/// The largest camera zoom for `items`: the one that shows the view of the largest item zoom
/// at [`MAX_PAGE_ZOOM`] (RB-080-94); the camera's range without an item.
pub fn camera_limit(items: &[CanvasItem]) -> f64 {
    let largest = items
        .iter()
        .map(|item| item.zoom)
        .filter(|zoom| zoom.is_finite() && *zoom > 0.0)
        .fold(0.0_f64, f64::max);
    if largest <= 0.0 {
        return ZOOM_RANGE.1;
    }
    (MAX_PAGE_ZOOM / largest).clamp(ZOOM_RANGE.0, ZOOM_RANGE.1)
}

/// The view and its label band of `item` (breakpoint size `size`) on the canvas.
pub fn outer(item: &CanvasItem, size: &Item, label: u32) -> WorldRect {
    let zoom = item.zoom;
    WorldRect {
        x: f64::from(item.x),
        y: f64::from(item.y) - f64::from(label),
        width: f64::from(size.width) * zoom,
        height: f64::from(size.height) * zoom + f64::from(label),
    }
}

/// Where each item goes in `area` through `camera`; `items[i]` shows `sizes[i]`. A view is shown
/// when part of it - of the view, not only of its label band - lies inside `area`; without
/// `partial` (`layoutModes.showPartialViews` off) only when view and label lie wholly inside.
pub fn arrange(
    items: &[CanvasItem],
    sizes: &[Item],
    area: CellRect,
    camera: Camera,
    label: u32,
    partial: bool,
) -> Vec<Slot> {
    let camera = camera.clamped();
    items
        .iter()
        .zip(sizes)
        .map(|(item, size)| {
            let world = outer(item, size, label);
            let screen = to_screen(world, area, camera);
            let slot = split_label(screen, label, true);
            let shown = screen.height > label && screen.width > 0;
            if !partial {
                return Slot {
                    visible: shown && contains(area, screen),
                    ..slot
                };
            }
            // The view below the label band at the area's top: what is left of it under a label
            // held there (the window cuts the view's overlay to the same room).
            let room = super::canvas_board::below_label(area, label);
            Slot {
                label: sticky(slot.label, slot.rect, area),
                visible: shown && overlaps(room, slot.rect),
                ..slot
            }
        })
        .collect()
}

/// The label strip `label` of the view `rect` held at the top edge of `area` while the view's
/// top is cut, as a sticky header is - until what is left of the view is no taller than it,
/// when it goes up with the view's bottom (RB-080-55).
fn sticky(label: CellRect, rect: CellRect, area: CellRect) -> CellRect {
    let bottom = i64::from(rect.y) + i64::from(rect.height) - i64::from(label.height);
    let top = i64::from(label.y).max(i64::from(area.y).min(bottom));
    CellRect {
        y: i32::try_from(top).unwrap_or(label.y),
        ..label
    }
}

/// `world` on screen: through `camera` into `area`.
pub fn to_screen(world: WorldRect, area: CellRect, camera: Camera) -> CellRect {
    let x = f64::from(area.x) + (world.x - camera.x) * camera.zoom;
    let y = f64::from(area.y) + (world.y - camera.y) * camera.zoom;
    CellRect {
        x: round_i32(x),
        y: round_i32(y),
        width: to_dip(world.width * camera.zoom),
        height: to_dip(world.height * camera.zoom),
    }
}

/// What the window shows of a placed view (RB-080-55): `bounds`, the part of its rectangle inside
/// the grid area, and how far the rectangle reaches past each side of `bounds` - the overlay lays
/// its content out that far outside its bounds, where it is cut off, so the page keeps its size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clip {
    /// The part inside the area.
    pub bounds: CellRect,
    /// DIP cut off above.
    pub top: u32,
    /// DIP cut off at the left.
    pub left: u32,
    /// DIP cut off below.
    pub bottom: u32,
    /// DIP cut off at the right.
    pub right: u32,
}

/// `rect` cut to `area`; `None` when they share no area.
pub fn clip(rect: CellRect, area: CellRect) -> Option<Clip> {
    let edges = |rect: CellRect| {
        let (x, y) = (i64::from(rect.x), i64::from(rect.y));
        (x, y, x + i64::from(rect.width), y + i64::from(rect.height))
    };
    let (left, top, right, bottom) = edges(rect);
    let (area_left, area_top, area_right, area_bottom) = edges(area);
    let (x0, y0) = (left.max(area_left), top.max(area_top));
    let (x1, y1) = (right.min(area_right), bottom.min(area_bottom));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    // Every value lies between two i32 edges, or is a difference of two of them within a u32 side.
    let dip = |value: i64| u32::try_from(value).unwrap_or(0);
    Some(Clip {
        bounds: CellRect {
            x: i32::try_from(x0).unwrap_or(0),
            y: i32::try_from(y0).unwrap_or(0),
            width: dip(x1 - x0),
            height: dip(y1 - y0),
        },
        top: dip(y0 - top),
        left: dip(x0 - left),
        bottom: dip(bottom - y1),
        right: dip(right - x1),
    })
}

/// What the window shows of a placed view's overlay: `outer` (label band and view) less the
/// `trim` DIP the canvas board draws the label in, cut to `area` below that band - a label held
/// at the area's top covers no page (RB-080-55). Without an area yet the place counts as
/// inside. `None` when nothing of the view is left.
pub fn overlay_cut(outer: CellRect, trim: u32, area: Option<CellRect>) -> Option<Clip> {
    let view = super::canvas_board::below_label(outer, trim);
    let room = area.map_or(view, |area| super::canvas_board::below_label(area, trim));
    clip(view, room)
}

/// The part of the canvas `area` shows through `camera`.
pub fn viewport(area: CellRect, camera: Camera) -> WorldRect {
    let camera = camera.clamped();
    WorldRect {
        x: camera.x,
        y: camera.y,
        width: f64::from(area.width) / camera.zoom,
        height: f64::from(area.height) / camera.zoom,
    }
}

/// Where a view of `width` x `height` (zoomed, canvas DIP) moved to `x`/`y` comes to rest:
/// an edge of its view and label band within `grid` DIP of an edge of one of `others` (also with
/// label bands) snaps to it; otherwise `x` and `y` snap to multiples of `grid`. A `grid` of 0
/// only rounds.
pub fn snap(
    (x, y): (f64, f64),
    (width, height): (f64, f64),
    others: &[WorldRect],
    grid: u32,
    label: u32,
) -> (i32, i32) {
    if grid == 0 {
        return (round_i32(x), round_i32(y));
    }
    let threshold = f64::from(grid);
    let top = y - f64::from(label);
    let x_edges = [x, x + width];
    let y_edges = [top, y + height];
    let x_targets: Vec<f64> = others.iter().flat_map(|o| [o.x, o.right()]).collect();
    let y_targets: Vec<f64> = others.iter().flat_map(|o| [o.y, o.bottom()]).collect();
    let snapped_x = nearest(&x_edges, &x_targets, threshold)
        .map_or_else(|| on_grid(x, grid), |delta| x + delta);
    let snapped_y = nearest(&y_edges, &y_targets, threshold)
        .map_or_else(|| on_grid(y, grid), |delta| y + delta);
    (round_i32(snapped_x), round_i32(snapped_y))
}

/// The smallest move (within `threshold`) that puts one of `edges` on one of `targets`.
fn nearest(edges: &[f64], targets: &[f64], threshold: f64) -> Option<f64> {
    edges
        .iter()
        .flat_map(|edge| targets.iter().map(move |target| target - edge))
        .filter(|delta| delta.abs() <= threshold)
        .min_by(|a, b| a.abs().total_cmp(&b.abs()))
}

fn on_grid(value: f64, grid: u32) -> f64 {
    let grid = f64::from(grid);
    (value / grid).round() * grid
}

/// The camera that shows every rectangle of `rects` in `area` with `margin` DIP around, centred,
/// at a zoom of [`FIT_MAX`] at most.
pub fn fit(rects: &[WorldRect], area: CellRect, margin: u32) -> Camera {
    let Some(bounds) = union(rects) else {
        return Camera::default();
    };
    let room_w = f64::from(area.width.saturating_sub(2 * margin)).max(1.0);
    let room_h = f64::from(area.height.saturating_sub(2 * margin)).max(1.0);
    let zoom = (room_w / bounds.width.max(1.0))
        .min(room_h / bounds.height.max(1.0))
        .clamp(ZOOM_RANGE.0, FIT_MAX);
    Camera {
        x: bounds.x - (f64::from(area.width) / zoom - bounds.width) / 2.0,
        y: bounds.y - (f64::from(area.height) / zoom - bounds.height) / 2.0,
        zoom,
    }
}

/// The camera at `zoom` whose view of `area` is centred on the canvas point `x`/`y` (a click
/// in the minimap).
pub fn center_on(x: f64, y: f64, area: CellRect, zoom: f64) -> Camera {
    let zoom = Camera {
        x: 0.0,
        y: 0.0,
        zoom,
    }
    .clamped()
    .zoom;
    Camera {
        x: x - f64::from(area.width) / zoom / 2.0,
        y: y - f64::from(area.height) / zoom / 2.0,
        zoom,
    }
}

/// The smallest rectangle around `rects`; `None` for none.
pub fn union(rects: &[WorldRect]) -> Option<WorldRect> {
    let first = rects.first()?;
    let (mut left, mut top) = (first.x, first.y);
    let (mut right, mut bottom) = (first.right(), first.bottom());
    for rect in &rects[1..] {
        left = left.min(rect.x);
        top = top.min(rect.y);
        right = right.max(rect.right());
        bottom = bottom.max(rect.bottom());
    }
    Some(WorldRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}

/// One item per view of `sizes`, in that order: the item of `items` with its breakpoint, else a
/// new one at [`default_item_zoom`] in a row to the right of the others, `gap` apart.
pub fn complete(items: &[CanvasItem], sizes: &[Item], gap: u32, label: u32) -> Vec<CanvasItem> {
    let known: Vec<(&CanvasItem, &Item)> = sizes
        .iter()
        .filter_map(|size| {
            items
                .iter()
                .find(|item| item.breakpoint == size.breakpoint)
                .map(|item| (item, size))
        })
        .collect();
    let rects: Vec<WorldRect> = known
        .iter()
        .map(|(item, size)| outer(item, size, label))
        .collect();
    let (mut x, y) = match union(&rects) {
        Some(bounds) => (bounds.right() + f64::from(gap), bounds.y + f64::from(label)),
        None => (0.0, f64::from(label)),
    };
    sizes
        .iter()
        .map(|size| {
            if let Some(item) = items.iter().find(|item| item.breakpoint == size.breakpoint) {
                return item.clone();
            }
            let item = CanvasItem {
                breakpoint: size.breakpoint.clone(),
                x: round_i32(x),
                y: round_i32(y),
                zoom: default_item_zoom(),
            };
            x += f64::from(size.width) * item.zoom + f64::from(gap);
            item
        })
        .collect()
}

fn round_i32(value: f64) -> i32 {
    if value.is_finite() {
        // Held to the i32 range first.
        value
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
    } else {
        0
    }
}

#[cfg(test)]
#[path = "canvas_tests.rs"]
mod tests;

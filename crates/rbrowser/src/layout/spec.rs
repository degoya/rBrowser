//! What a mode shows and how, resolved from the preset's fields, the session and the user
//! settings: [`ModeSpec`]. Pure; the host keeps the spec in effect and arranges it again for
//! every new grid area.

use rb_config::{
    CanvasItem, Layout, LayoutCell, LayoutMode, LayoutModeSettings, ModeFields, SplitOrientation,
    StripHeight, Zoom,
};
use rb_engine::CellRect;

use super::canvas::{self, Camera};
use super::horizontal::{self, Strip};
use super::split::{self, Split, clamp_ratio};
use super::{Item, Slot};

/// The session's side of the modes: the fields in effect for the current preset and mode (they
/// may be ahead of the file), the fields as the file had them last, the strip's scroll, the
/// canvas camera and the minimap switch. Part of the shell state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModeSession {
    /// The preset the fields belong to; `None` for the top-level layout.
    pub preset: Option<String>,
    /// The mode the fields belong to; `None` before the first mode was shown.
    pub mode: Option<LayoutMode>,
    /// The fields in effect.
    pub fields: ModeFields,
    /// The preset's fields as last read from or written to the config.
    pub persisted: ModeFields,
    /// Horizontal: the view at the left edge.
    pub first: usize,
    /// Canvas: the camera; `None` until the canvas is first shown (it then fits every view).
    pub camera: Option<Camera>,
    /// The minimap switched on or off for this session; the setting when `None`.
    pub minimap: Option<bool>,
    /// `layoutModes` as of the last time a mode was shown (the toolbar's snapshot reads it).
    pub settings: LayoutModeSettings,
}

impl ModeSession {
    /// Takes over `selection`'s preset and mode with the preset's fields `stored`: a new preset
    /// or mode starts afresh (the minimap switch stays); the same one keeps the session's fields
    /// unless the file changed them (an edit by hand wins).
    pub fn enter(&mut self, preset: Option<&str>, mode: LayoutMode, stored: &ModeFields) {
        if self.preset.as_deref() != preset || self.mode != Some(mode) {
            *self = Self {
                preset: preset.map(str::to_owned),
                mode: Some(mode),
                fields: stored.clone(),
                persisted: stored.clone(),
                minimap: self.minimap,
                settings: self.settings,
                ..Self::default()
            };
        } else if self.persisted != *stored {
            self.fields = stored.clone();
            self.persisted = stored.clone();
        }
    }
}

/// How the views of a mode are placed.
#[derive(Clone, Debug, PartialEq)]
pub enum ModeKind {
    /// One row; `first` is the view at the left edge.
    Horizontal {
        /// The views' height.
        height: StripHeight,
        /// The strip's scale.
        zoom: Zoom,
        /// The view at the left edge.
        first: usize,
    },
    /// Two sides; the first `primary` breakpoints are the primary side.
    Split {
        /// Where the divider runs.
        orientation: SplitOrientation,
        /// The primary side's share.
        ratio: f64,
        /// How many of the breakpoints are on the primary side.
        primary: usize,
    },
    /// Free placement; `items` as configured (completed for the views when arranged).
    Canvas {
        /// The snap grid in DIP.
        snap: u32,
        /// The configured places.
        items: Vec<CanvasItem>,
        /// What the area shows; `None` fits every view.
        camera: Option<Camera>,
        /// `layoutModes.showPartialViews`: a view that reaches past the area is shown, clipped.
        partial: bool,
    },
}

/// A mode resolved: the breakpoints shown, in order, the gap and the placement.
#[derive(Clone, Debug, PartialEq)]
pub struct ModeSpec {
    /// The breakpoints, one view each, in this order.
    pub breakpoints: Vec<String>,
    /// The gap in DIP.
    pub gap: u32,
    /// How they are placed.
    pub kind: ModeKind,
}

impl ModeSpec {
    /// The mode.
    pub fn mode(&self) -> LayoutMode {
        match self.kind {
            ModeKind::Horizontal { .. } => LayoutMode::Horizontal,
            ModeKind::Split { .. } => LayoutMode::Split,
            ModeKind::Canvas { .. } => LayoutMode::Canvas,
        }
    }

    /// The grid the host rebuilds its views from: one row, one cell per breakpoint, in order.
    /// It only matches and orders the views; their bounds come from [`Self::arrange`].
    pub fn grid(&self) -> Layout {
        let cells: Vec<LayoutCell> = self
            .breakpoints
            .iter()
            .enumerate()
            .map(|(index, breakpoint)| LayoutCell {
                breakpoint: breakpoint.clone(),
                col: index as u32 + 1,
                row: 1,
                col_span: 1,
                row_span: 1,
                zoom: Zoom::Fit,
            })
            .collect();
        Layout {
            columns: (cells.len() as u32).max(1),
            rows: 1,
            gap: self.gap,
            cells,
            ..Default::default()
        }
    }

    /// Where each view goes in `area`; `sizes[i]` is the size of `breakpoints[i]`.
    pub fn arrange(&self, sizes: &[Item], area: CellRect, label: u32) -> Vec<Slot> {
        match &self.kind {
            ModeKind::Horizontal {
                height,
                zoom,
                first,
            } => horizontal::arrange(
                sizes,
                area,
                &Strip {
                    height: *height,
                    gap: self.gap,
                    label,
                    zoom: *zoom,
                    first: *first,
                },
            ),
            ModeKind::Split {
                orientation,
                ratio,
                primary,
            } => {
                let primary = (*primary).min(sizes.len());
                split::arrange(
                    primary,
                    sizes.len() - primary,
                    area,
                    &Split {
                        orientation: *orientation,
                        ratio: *ratio,
                        gap: self.gap,
                        label,
                    },
                )
            }
            ModeKind::Canvas { items, camera, .. } => {
                let items = self.canvas_items(items, sizes, label);
                let camera = camera.unwrap_or_else(|| self.fit(&items, sizes, area, label));
                self.canvas_slots(&items, sizes, area, camera, label)
            }
        }
    }

    /// Where the canvas `items` go in `area` through `camera` - the camera in effect or one a
    /// gesture tries out - by this spec's `partial` ([`canvas::arrange`]).
    pub fn canvas_slots(
        &self,
        items: &[CanvasItem],
        sizes: &[Item],
        area: CellRect,
        camera: Camera,
        label: u32,
    ) -> Vec<Slot> {
        let partial = matches!(self.kind, ModeKind::Canvas { partial: true, .. });
        canvas::arrange(items, sizes, area, camera, label, partial)
    }

    /// The canvas items for the views of `sizes`: the configured ones, the others placed after.
    pub fn canvas_items(
        &self,
        items: &[CanvasItem],
        sizes: &[Item],
        label: u32,
    ) -> Vec<CanvasItem> {
        canvas::complete(items, sizes, 2 * self.gap.max(1), label)
    }

    /// The camera that shows every item.
    pub fn fit(&self, items: &[CanvasItem], sizes: &[Item], area: CellRect, label: u32) -> Camera {
        let rects: Vec<canvas::WorldRect> = items
            .iter()
            .zip(sizes)
            .map(|(item, size)| canvas::outer(item, size, label))
            .collect();
        canvas::fit(&rects, area, self.gap)
    }
}

/// What a mode needs besides the session: the base grid of the selection (its breakpoints and
/// gap), the user settings, the active view's breakpoint, which breakpoints may be shown and
/// which are hidden.
pub struct ModeInputs<'a> {
    /// The selection's grid.
    pub grid: &'a Layout,
    /// `layoutModes`.
    pub settings: &'a LayoutModeSettings,
    /// The active view's breakpoint.
    pub active: Option<&'a str>,
    /// The default zoom of cells rBrowser lays out itself (the strip's scale).
    pub default_zoom: Zoom,
    /// Whether a breakpoint id may be shown (a breakpoint of the config, on any engine).
    pub known: &'a dyn Fn(&str) -> bool,
    /// The hidden selection (`layout.hidden`, RB-080-41): these get no view in any mode.
    pub hidden: &'a [String],
}

/// The spec of `mode` for `session`; `None` for grid and focus.
pub fn resolve_mode(
    mode: LayoutMode,
    session: &ModeSession,
    inputs: &ModeInputs<'_>,
) -> Option<ModeSpec> {
    let fields = &session.fields;
    let shown = |id: &str| (inputs.known)(id) && !inputs.hidden.iter().any(|hidden| hidden == id);
    let order: Vec<String> = reading_order(inputs.grid)
        .into_iter()
        .filter(|id| !inputs.hidden.contains(id))
        .collect();
    let gap = fields.gap.unwrap_or(inputs.grid.gap);
    let spec = match mode {
        LayoutMode::Grid | LayoutMode::Focus => return None,
        LayoutMode::Horizontal => ModeSpec {
            breakpoints: order,
            gap,
            kind: ModeKind::Horizontal {
                height: fields.height.unwrap_or(inputs.settings.horizontal_height),
                zoom: inputs.default_zoom,
                first: session.first,
            },
        },
        LayoutMode::Split => {
            let mut primary = known_unique(&fields.primary, &shown, &[]);
            if primary.is_empty() {
                let pick = inputs
                    .active
                    .filter(|active| order.iter().any(|id| id == active))
                    .map(str::to_owned)
                    .or_else(|| order.first().cloned());
                primary.extend(pick);
            }
            let secondary = if fields.secondary.is_empty() {
                order
                    .iter()
                    .filter(|id| !primary.contains(id))
                    .cloned()
                    .collect()
            } else {
                known_unique(&fields.secondary, &shown, &primary)
            };
            let count = primary.len();
            primary.extend(secondary);
            ModeSpec {
                breakpoints: primary,
                gap: inputs.grid.gap,
                kind: ModeKind::Split {
                    orientation: fields.orientation.unwrap_or_default(),
                    ratio: clamp_ratio(fields.ratio.unwrap_or(0.5)),
                    primary: count,
                },
            }
        }
        LayoutMode::Canvas => {
            let mut breakpoints = order;
            for item in &fields.items {
                if shown(&item.breakpoint) && !breakpoints.contains(&item.breakpoint) {
                    breakpoints.push(item.breakpoint.clone());
                }
            }
            ModeSpec {
                breakpoints,
                gap: inputs.grid.gap,
                kind: ModeKind::Canvas {
                    snap: fields.snap.unwrap_or(inputs.settings.snap),
                    items: fields.items.clone(),
                    camera: session.camera,
                    partial: inputs.settings.show_partial_views,
                },
            }
        }
    };
    Some(spec)
}

/// The grid's breakpoints in reading order (row, then column), each once.
fn reading_order(grid: &Layout) -> Vec<String> {
    let mut cells: Vec<&LayoutCell> = grid.cells.iter().collect();
    cells.sort_by_key(|cell| (cell.row, cell.col));
    let mut order: Vec<String> = Vec::with_capacity(cells.len());
    for cell in cells {
        if !order.contains(&cell.breakpoint) {
            order.push(cell.breakpoint.clone());
        }
    }
    order
}

/// The ids of `list` that are known and not in `taken`, each once.
fn known_unique(list: &[String], known: &dyn Fn(&str) -> bool, taken: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in list {
        if known(id) && !taken.contains(id) && !out.contains(id) {
            out.push(id.clone());
        }
    }
    out
}

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;

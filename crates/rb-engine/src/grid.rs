//! Where views sit in the window: the computed cells of the grid.
//!
//! Coordinates are DIP in the main window's client area (ADR 0001, Decision 2). RB-010-04
//! computes them; the shell (RB-010-07) places cell labels and badges in `label` rects, never
//! over a view.

use rb_config::{Layout, Zoom};
use serde::{Deserialize, Serialize};

use crate::EngineError;

/// A rectangle in DIP, relative to the main window's client area.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellRect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

/// One computed cell of the grid: its place in the layout and its rectangles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridCell {
    /// The breakpoint id of `layout.cells[]`.
    pub breakpoint: String,
    /// Column, 1-based.
    pub col: u32,
    /// Row, 1-based.
    pub row: u32,
    /// Columns spanned.
    pub col_span: u32,
    /// Rows spanned.
    pub row_span: u32,
    /// The cell's zoom.
    pub zoom: Zoom,
    /// Where the view is.
    pub rect: CellRect,
    /// The strip in the gap above the view for its label, size and badges.
    pub label: CellRect,
}

/// Lays `layout` out in `area` (the grid area below the shell), reserving `label_height` DIP
/// above each cell. Fails on a cell outside the grid or on two cells sharing a slot.
/// RB-010-04.
///
/// Geometry: `gap` around and between the cells; every row is a label strip of `label_height`
/// followed by the view, so a label always sits in the gap above its view and never over one.
/// The space left after gaps and labels is split evenly over columns and rows, the remainder
/// spread one DIP at a time, so the cells fill `area` exactly. A cell spanning several rows
/// takes the label strips and gaps between them into its view. Cells come back in
/// `layout.cells` order. An area too small for the gaps gives empty cells, not an error.
pub fn compute_grid(
    layout: &Layout,
    area: CellRect,
    label_height: u32,
) -> Result<Vec<GridCell>, EngineError> {
    validate(layout)?;
    let columns = Axis::new(area.x, area.width, layout.columns, layout.gap, 0);
    let rows = Axis::new(area.y, area.height, layout.rows, layout.gap, label_height);
    Ok(layout
        .cells
        .iter()
        .map(|cell| {
            let (x, right) = columns.span(cell.col - 1, cell.col - 1 + cell.col_span - 1);
            let (top, bottom) = rows.span(cell.row - 1, cell.row - 1 + cell.row_span - 1);
            let width = to_size(right - x);
            let label = label_height.min(to_size(bottom - top));
            GridCell {
                breakpoint: cell.breakpoint.clone(),
                col: cell.col,
                row: cell.row,
                col_span: cell.col_span,
                row_span: cell.row_span,
                zoom: cell.zoom,
                rect: CellRect {
                    x,
                    y: top + label as i32,
                    width,
                    height: to_size(bottom - top - label as i32),
                },
                label: CellRect {
                    x,
                    y: top,
                    width,
                    height: label,
                },
            }
        })
        .collect())
}

/// One axis of the grid: where each track (column, or row with its label strip) starts and
/// how long its share of the free space is.
struct Axis {
    origin: i64,
    count: i64,
    gap: i64,
    lead: i64,
    free: i64,
}

impl Axis {
    fn new(origin: i32, length: u32, count: u32, gap: u32, lead: u32) -> Self {
        let (count, gap, lead) = (i64::from(count), i64::from(gap), i64::from(lead));
        let free = (i64::from(length) - (count + 1) * gap - count * lead).max(0);
        Self {
            origin: i64::from(origin),
            count,
            gap,
            lead,
            free,
        }
    }

    /// The share of the free space before track `index` (0-based).
    fn before(&self, index: u32) -> i64 {
        i64::from(index) * self.free / self.count
    }

    /// Start of track `first` and end of track `last` (0-based, inclusive).
    fn span(&self, first: u32, last: u32) -> (i32, i32) {
        let start =
            self.origin + self.gap + i64::from(first) * (self.gap + self.lead) + self.before(first);
        let end = self.origin
            + self.gap
            + i64::from(last) * (self.gap + self.lead)
            + self.lead
            + self.before(last + 1);
        (clamp_i32(start), clamp_i32(end))
    }
}

fn clamp_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value < 0 { i32::MIN } else { i32::MAX })
}

fn to_size(value: i32) -> u32 {
    u32::try_from(value).unwrap_or(0)
}

/// Every cell inside the grid, no two sharing a slot.
fn validate(layout: &Layout) -> Result<(), EngineError> {
    if layout.columns == 0 || layout.rows == 0 {
        return Err(EngineError::Invalid(format!(
            "the grid needs at least one column and one row, not {} x {}",
            layout.columns, layout.rows
        )));
    }
    let slots = layout.columns as usize * layout.rows as usize;
    let mut owner: Vec<Option<usize>> = vec![None; slots];
    for (index, cell) in layout.cells.iter().enumerate() {
        let inside = |start: u32, span: u32, count: u32| {
            start >= 1 && span >= 1 && start.checked_add(span - 1).is_some_and(|end| end <= count)
        };
        if !inside(cell.col, cell.col_span, layout.columns)
            || !inside(cell.row, cell.row_span, layout.rows)
        {
            return Err(EngineError::Invalid(format!(
                "layout.cells[{index}] ({}) lies outside the {} x {} grid",
                cell.breakpoint, layout.columns, layout.rows
            )));
        }
        for row in cell.row - 1..cell.row - 1 + cell.row_span {
            for col in cell.col - 1..cell.col - 1 + cell.col_span {
                let slot = row as usize * layout.columns as usize + col as usize;
                if let Some(other) = owner[slot] {
                    return Err(EngineError::Invalid(format!(
                        "layout.cells[{index}] ({}) overlaps layout.cells[{other}]",
                        cell.breakpoint
                    )));
                }
                owner[slot] = Some(index);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "grid_tests.rs"]
mod tests;

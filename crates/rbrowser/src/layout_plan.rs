//! How a grid becomes nested CEF BoxLayouts (ADR 0001, Decision 2: manual `set_bounds` does
//! not hold, a panel's FillLayout overrides it).
//!
//! The grid is cut recursively along column or row boundaries that no cell crosses (a
//! guillotine partition): each cut is a horizontal or vertical box, each leaf a cell or an
//! empty spacer. A child's flex weight is the number of columns (rows) it spans, and its
//! preferred size is the fixed part inside it (the gaps and label strips it contains), so the
//! box distributes exactly the space [`rb_engine::compute_grid`] gives the tracks. A layout no
//! cut can split (a "pinwheel" of spanning cells) is refused.

use rb_config::Layout;
use rb_engine::{CellRect, EngineError};

/// A rectangle of grid slots: 0-based first column and row, and how many of each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    /// First column, 0-based.
    pub col: u32,
    /// First row, 0-based.
    pub row: u32,
    /// Columns covered.
    pub cols: u32,
    /// Rows covered.
    pub rows: u32,
}

/// One node of the plan.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanNode {
    /// The slots the node covers.
    pub region: Region,
    /// What it is.
    pub kind: NodeKind,
}

/// A node's content.
#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    /// The cell `layout.cells[index]`: a label strip over a view. With hidden breakpoints
    /// ([`plan_hiding`]) `index` counts the visible cells only, as [`visible_cells`] lists them.
    Cell(usize),
    /// The slot of a cell whose breakpoint is hidden (RB-080-41): empty, with a label strip
    /// that names the breakpoint and offers to open it again.
    Hidden(String),
    /// Slots no cell uses.
    Spacer,
    /// A box: children side by side (`horizontal`) or stacked.
    Split {
        /// Side by side when `true`.
        horizontal: bool,
        /// The children, left to right or top to bottom.
        children: Vec<PlanNode>,
    },
}

impl PlanNode {
    /// The flex weight inside a parent box: columns in a horizontal box, rows in a vertical one.
    pub fn flex(&self, horizontal_parent: bool) -> u32 {
        if horizontal_parent {
            self.region.cols
        } else {
            self.region.rows
        }
    }

    /// The preferred size: the gaps between its columns, and the label strips and gaps of its
    /// rows. The box adds the node's share of the free space to it.
    pub fn preferred(&self, gap: u32, label_height: u32) -> (u32, u32) {
        let Region { cols, rows, .. } = self.region;
        (
            cols.saturating_sub(1) * gap,
            rows * label_height + rows.saturating_sub(1) * gap,
        )
    }
}

/// The plan of `layout`. Expects a layout [`rb_engine::compute_grid`] accepts (cells inside
/// the grid, none sharing a slot).
pub fn plan(layout: &Layout) -> Result<PlanNode, EngineError> {
    plan_hiding(layout, &[])
}

/// The plan of `layout` with the cells of the `hidden` breakpoints as [`NodeKind::Hidden`]
/// leaves (RB-080-41): they keep their slots, and the visible cells are numbered as
/// [`visible_cells`] lists them.
pub fn plan_hiding(layout: &Layout, hidden: &[String]) -> Result<PlanNode, EngineError> {
    let mut visible = 0;
    let spans: Vec<(Leaf, Region)> = layout
        .cells
        .iter()
        .map(|cell| {
            let leaf = if hidden.contains(&cell.breakpoint) {
                Leaf::Hidden(cell.breakpoint.clone())
            } else {
                visible += 1;
                Leaf::Cell(visible - 1)
            };
            (
                leaf,
                Region {
                    col: cell.col.saturating_sub(1),
                    row: cell.row.saturating_sub(1),
                    cols: cell.col_span,
                    rows: cell.row_span,
                },
            )
        })
        .collect();
    let whole = Region {
        col: 0,
        row: 0,
        cols: layout.columns,
        rows: layout.rows,
    };
    split(whole, &spans)
}

/// `layout` without the cells of the `hidden` breakpoints; the grid's size stays, so the
/// remaining cells keep their slots (RB-080-41).
pub fn visible_cells(layout: &Layout, hidden: &[String]) -> Layout {
    Layout {
        cells: layout
            .cells
            .iter()
            .filter(|cell| !hidden.contains(&cell.breakpoint))
            .cloned()
            .collect(),
        ..layout.clone()
    }
}

/// What a leaf of the plan becomes.
#[derive(Clone, Debug, PartialEq)]
enum Leaf {
    Cell(usize),
    Hidden(String),
}

impl Leaf {
    fn kind(&self) -> NodeKind {
        match self {
            Leaf::Cell(index) => NodeKind::Cell(*index),
            Leaf::Hidden(breakpoint) => NodeKind::Hidden(breakpoint.clone()),
        }
    }
}

fn split(region: Region, cells: &[(Leaf, Region)]) -> Result<PlanNode, EngineError> {
    let kind = match cells {
        [] => NodeKind::Spacer,
        [(leaf, only)] if *only == region => leaf.kind(),
        _ => {
            if let Some(children) = cut(region, cells, true)? {
                NodeKind::Split {
                    horizontal: true,
                    children,
                }
            } else if let Some(children) = cut(region, cells, false)? {
                NodeKind::Split {
                    horizontal: false,
                    children,
                }
            } else {
                return Err(EngineError::Invalid(format!(
                    "the cells in columns {}-{}, rows {}-{} cannot be arranged in rows and \
                     columns; change the spans",
                    region.col + 1,
                    region.col + region.cols,
                    region.row + 1,
                    region.row + region.rows
                )));
            }
        }
    };
    Ok(PlanNode { region, kind })
}

/// Cuts `region` at every column (`vertical_lines`) or row boundary no cell crosses; `None`
/// when there is none.
fn cut(
    region: Region,
    cells: &[(Leaf, Region)],
    vertical_lines: bool,
) -> Result<Option<Vec<PlanNode>>, EngineError> {
    let (start, count) = if vertical_lines {
        (region.col, region.cols)
    } else {
        (region.row, region.rows)
    };
    let extent = |cell: &Region| {
        if vertical_lines {
            (cell.col, cell.cols)
        } else {
            (cell.row, cell.rows)
        }
    };
    let crossed = |line: u32| {
        cells.iter().any(|(_, cell)| {
            let (first, span) = extent(cell);
            first < line && line < first + span
        })
    };
    let mut lines: Vec<u32> = (start + 1..start + count)
        .filter(|line| !crossed(*line))
        .collect();
    if lines.is_empty() {
        return Ok(None);
    }
    lines.insert(0, start);
    lines.push(start + count);
    let mut children = Vec::with_capacity(lines.len() - 1);
    for pair in lines.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let part = if vertical_lines {
            Region {
                col: from,
                cols: to - from,
                ..region
            }
        } else {
            Region {
                row: from,
                rows: to - from,
                ..region
            }
        };
        let inside: Vec<(Leaf, Region)> = cells
            .iter()
            .filter(|(_, cell)| {
                let (first, _) = extent(cell);
                from <= first && first < to
            })
            .cloned()
            .collect();
        children.push(split(part, &inside)?);
    }
    Ok(Some(children))
}

/// Lays the plan out the way CEF's BoxLayout does (preferred size plus the flex share of the
/// free space, rounded cumulatively; `gap` between children and around the root) and returns
/// each cell's view rectangle by `layout.cells` index. Used by the tests to hold the plan to
/// [`rb_engine::compute_grid`].
pub fn simulate(
    root: &PlanNode,
    area: CellRect,
    gap: u32,
    label_height: u32,
) -> Vec<(usize, CellRect)> {
    let inner = CellRect {
        x: area.x + gap as i32,
        y: area.y + gap as i32,
        width: area.width.saturating_sub(2 * gap),
        height: area.height.saturating_sub(2 * gap),
    };
    let mut out = Vec::new();
    place(root, inner, gap, label_height, &mut out);
    out.sort_by_key(|(index, _)| *index);
    out
}

fn place(
    node: &PlanNode,
    bounds: CellRect,
    gap: u32,
    label_height: u32,
    out: &mut Vec<(usize, CellRect)>,
) {
    match &node.kind {
        NodeKind::Spacer | NodeKind::Hidden(_) => {}
        NodeKind::Cell(index) => {
            let label = label_height.min(bounds.height);
            out.push((
                *index,
                CellRect {
                    y: bounds.y + label as i32,
                    height: bounds.height - label,
                    ..bounds
                },
            ));
        }
        NodeKind::Split {
            horizontal,
            children,
        } => {
            let main = if *horizontal {
                bounds.width
            } else {
                bounds.height
            };
            let preferred: Vec<u32> = children
                .iter()
                .map(|child| {
                    let (width, height) = child.preferred(gap, label_height);
                    if *horizontal { width } else { height }
                })
                .collect();
            let spacing = gap * (children.len() as u32).saturating_sub(1);
            let fixed: u32 = preferred.iter().sum::<u32>() + spacing;
            let free = i64::from(main) - i64::from(fixed);
            let total_flex: i64 = children
                .iter()
                .map(|child| i64::from(child.flex(*horizontal)))
                .sum();
            let (mut offset, mut flex_so_far, mut given) = (0i64, 0i64, 0i64);
            for (child, own) in children.iter().zip(preferred) {
                flex_so_far += i64::from(child.flex(*horizontal));
                let share = if total_flex > 0 {
                    free * flex_so_far / total_flex - given
                } else {
                    0
                };
                given += share;
                let size = u32::try_from((i64::from(own) + share).max(0)).unwrap_or(0);
                let child_bounds = if *horizontal {
                    CellRect {
                        x: bounds.x + offset as i32,
                        width: size,
                        ..bounds
                    }
                } else {
                    CellRect {
                        y: bounds.y + offset as i32,
                        height: size,
                        ..bounds
                    }
                };
                place(child, child_bounds, gap, label_height, out);
                offset += i64::from(size) + i64::from(gap);
            }
        }
    }
}

#[cfg(test)]
#[path = "layout_plan_tests.rs"]
mod tests;

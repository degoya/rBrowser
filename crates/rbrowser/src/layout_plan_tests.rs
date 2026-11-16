use super::*;
use rb_config::{LayoutCell, ProjectConfig, Zoom};
use rb_engine::compute_grid;

const FIXTURE: &str = include_str!("../../../tests/fixtures/grid-project/.rbrowser/config.json");
const LABEL: u32 = 20;

fn fixture() -> Layout {
    let config: ProjectConfig = serde_json::from_str(FIXTURE).expect("fixture parses");
    config.layout
}

fn cell(col: u32, row: u32, col_span: u32, row_span: u32) -> LayoutCell {
    LayoutCell {
        breakpoint: format!("c{col}-{row}"),
        col,
        row,
        col_span,
        row_span,
        zoom: Zoom::Fit,
    }
}

fn area(width: u32, height: u32) -> CellRect {
    CellRect {
        x: 0,
        y: 44,
        width,
        height,
    }
}

fn assert_matches_compute_grid(layout: &Layout, area: CellRect) {
    let plan = plan(layout).expect("plans");
    let simulated = simulate(&plan, area, layout.gap, LABEL);
    let expected = compute_grid(layout, area, LABEL).expect("lays out");
    assert_eq!(simulated.len(), expected.len());
    for ((index, got), want) in simulated.iter().zip(&expected) {
        let want = want.rect;
        let close = |a: i64, b: i64| (a - b).abs() <= 1;
        assert!(
            close(got.x.into(), want.x.into())
                && close(got.y.into(), want.y.into())
                && close(got.width.into(), want.width.into())
                && close(got.height.into(), want.height.into()),
            "cell {index}: boxes give {got:?}, the grid {want:?}"
        );
    }
}

#[test]
fn the_fixture_splits_at_every_free_column_line() {
    let root = plan(&fixture()).expect("plans");
    let NodeKind::Split {
        horizontal: true,
        children,
    } = &root.kind
    else {
        panic!("root is not a horizontal box: {root:?}");
    };
    // The tall desktop cell | laptop over mobile | tablet and tablet-landscape over mobile-small.
    assert_eq!(children.len(), 3, "{children:?}");
    assert_eq!(children[0].kind, NodeKind::Cell(0));
    let flex: Vec<u32> = children.iter().map(|child| child.flex(true)).collect();
    assert_eq!(flex, vec![1, 1, 2]);
    for child in &children[1..] {
        assert!(
            matches!(
                child.kind,
                NodeKind::Split {
                    horizontal: false,
                    ..
                }
            ),
            "{child:?}"
        );
    }
    assert_eq!(children[0].preferred(8, LABEL), (0, 2 * LABEL + 8));
    assert_eq!(children[2].preferred(8, LABEL), (8, 2 * LABEL + 8));
}

#[test]
fn the_boxes_reproduce_the_computed_grid_at_any_size() {
    let layout = fixture();
    for (width, height) in [(1200, 800), (1333, 777), (1919, 1003), (641, 401)] {
        assert_matches_compute_grid(&layout, area(width, height));
    }
}

#[test]
fn empty_slots_become_spacers_and_keep_the_other_cells_in_place() {
    let layout = Layout {
        columns: 3,
        rows: 2,
        gap: 6,
        cells: vec![cell(1, 1, 1, 1), cell(3, 2, 1, 1)],
        ..Default::default()
    };
    let root = plan(&layout).expect("plans");
    let mut spacers = 0;
    let mut stack = vec![&root];
    while let Some(node) = stack.pop() {
        match &node.kind {
            NodeKind::Spacer => spacers += 1,
            NodeKind::Split { children, .. } => stack.extend(children),
            NodeKind::Cell(_) | NodeKind::Hidden(_) => {}
        }
    }
    assert!(spacers >= 2, "{root:?}");
    assert_matches_compute_grid(&layout, area(1000, 700));
}

#[test]
fn a_single_cell_and_an_empty_grid_are_plain_nodes() {
    let single = Layout {
        columns: 1,
        rows: 1,
        gap: 8,
        cells: vec![cell(1, 1, 1, 1)],
        ..Default::default()
    };
    assert_eq!(plan(&single).expect("plans").kind, NodeKind::Cell(0));
    let empty = Layout {
        cells: Vec::new(),
        ..single
    };
    assert_eq!(plan(&empty).expect("plans").kind, NodeKind::Spacer);
}

#[test]
fn a_pinwheel_cannot_be_built_from_boxes() {
    let pinwheel = Layout {
        columns: 3,
        rows: 3,
        gap: 8,
        cells: vec![
            cell(1, 1, 2, 1),
            cell(3, 1, 1, 2),
            cell(2, 3, 2, 1),
            cell(1, 2, 1, 2),
            cell(2, 2, 1, 1),
        ],
        ..Default::default()
    };
    assert!(matches!(plan(&pinwheel), Err(EngineError::Invalid(_))));
}

/// A hidden breakpoint of an explicit grid keeps its slot as an empty cell (RB-080-41): the
/// plan puts a `Hidden` leaf there, and the visible cells are numbered by their place among the
/// visible ones, as the host's cell list has them.
#[test]
fn a_hidden_cell_keeps_its_slot_and_the_visible_ones_are_renumbered() {
    let layout = Layout {
        columns: 3,
        rows: 1,
        cells: vec![cell(1, 1, 1, 1), cell(2, 1, 1, 1), cell(3, 1, 1, 1)],
        ..Layout::default()
    };
    let root = plan_hiding(&layout, &["c2-1".to_owned()]).expect("plans");
    let NodeKind::Split { children, .. } = &root.kind else {
        panic!("{root:?}");
    };
    assert_eq!(children[0].kind, NodeKind::Cell(0));
    assert_eq!(children[1].kind, NodeKind::Hidden("c2-1".to_owned()));
    assert_eq!(children[2].kind, NodeKind::Cell(1));
    // The visible layout the host computes its cells from is the plan's numbering.
    let visible = visible_cells(&layout, &["c2-1".to_owned()]);
    assert_eq!(visible.cells.len(), 2);
    assert_eq!(visible.cells[1].breakpoint, "c3-1");
    assert_eq!(visible.columns, 3, "the slots stay");
    // Without a hidden breakpoint the plan is the plain one.
    assert_eq!(
        plan_hiding(&layout, &[]).expect("plans"),
        plan(&layout).expect("plans")
    );
    // A hidden leaf is placed like a cell: nothing is reported for it, the others keep their
    // rectangles.
    let placed = simulate(&root, area(1000, 500), 8, LABEL);
    assert_eq!(placed.len(), 2);
    assert_eq!(placed[0].0, 0);
    assert_eq!(placed[1].0, 1);
}

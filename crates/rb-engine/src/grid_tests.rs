use super::*;
use rb_config::{LayoutCell, ProjectConfig};

const FIXTURE: &str = include_str!("../../../tests/fixtures/grid-project/.rbrowser/config.json");

fn rect(x: i32, y: i32, width: u32, height: u32) -> CellRect {
    CellRect {
        x,
        y,
        width,
        height,
    }
}

fn cell(breakpoint: &str, col: u32, row: u32, col_span: u32, row_span: u32) -> LayoutCell {
    LayoutCell {
        breakpoint: breakpoint.to_owned(),
        col,
        row,
        col_span,
        row_span,
        zoom: Zoom::Fit,
    }
}

fn layout(columns: u32, rows: u32, gap: u32, cells: Vec<LayoutCell>) -> Layout {
    Layout {
        columns,
        rows,
        gap,
        cells,
        ..Default::default()
    }
}

#[test]
fn cell_rect_serializes_camel_case() {
    assert_eq!(
        serde_json::to_value(rect(8, 40, 480, 300)).expect("serializes"),
        serde_json::json!({ "x": 8, "y": 40, "width": 480, "height": 300 })
    );
}

#[test]
fn the_fixture_config_gives_six_cells_at_the_expected_bounds() {
    let config: ProjectConfig = serde_json::from_str(FIXTURE).expect("fixture parses");
    let cells = compute_grid(&config.layout, rect(0, 44, 1200, 800), 20).expect("lays out");
    let views: Vec<(&str, CellRect)> = cells
        .iter()
        .map(|cell| (cell.breakpoint.as_str(), cell.rect))
        .collect();
    assert_eq!(
        views,
        vec![
            ("desktop", rect(8, 72, 290, 764)),
            ("laptop", rect(306, 72, 290, 368)),
            ("tablet", rect(604, 72, 290, 368)),
            ("tablet-landscape", rect(902, 72, 290, 368)),
            ("mobile", rect(306, 468, 290, 368)),
            ("mobile-small", rect(604, 468, 588, 368)),
        ]
    );
    let labels: Vec<CellRect> = cells.iter().map(|cell| cell.label).collect();
    assert_eq!(
        labels,
        vec![
            rect(8, 52, 290, 20),
            rect(306, 52, 290, 20),
            rect(604, 52, 290, 20),
            rect(902, 52, 290, 20),
            rect(306, 448, 290, 20),
            rect(604, 448, 588, 20),
        ]
    );
    assert_eq!(cells[3].zoom, Zoom::Factor(0.25));
    assert_eq!((cells[0].row_span, cells[5].col_span), (2, 2));
}

#[test]
fn cells_never_overlap_each_other_or_the_labels() {
    let config: ProjectConfig = serde_json::from_str(FIXTURE).expect("fixture parses");
    let cells = compute_grid(&config.layout, rect(0, 44, 1333, 777), 18).expect("lays out");
    let mut rects = Vec::new();
    for cell in &cells {
        rects.push(cell.rect);
        rects.push(cell.label);
    }
    for (i, a) in rects.iter().enumerate() {
        for b in &rects[i + 1..] {
            let apart = a.x + a.width as i32 <= b.x
                || b.x + b.width as i32 <= a.x
                || a.y + a.height as i32 <= b.y
                || b.y + b.height as i32 <= a.y;
            assert!(apart, "{a:?} overlaps {b:?}");
        }
    }
}

#[test]
fn the_remainder_is_spread_so_the_cells_fill_the_area_exactly() {
    let cells = compute_grid(
        &layout(
            3,
            1,
            0,
            vec![
                cell("a", 1, 1, 1, 1),
                cell("b", 2, 1, 1, 1),
                cell("c", 3, 1, 1, 1),
            ],
        ),
        rect(0, 0, 1001, 300),
        0,
    )
    .expect("lays out");
    let widths: Vec<u32> = cells.iter().map(|cell| cell.rect.width).collect();
    assert_eq!(widths, vec![333, 334, 334]);
    assert_eq!(cells[2].rect.x + cells[2].rect.width as i32, 1001);
}

#[test]
fn a_tiny_area_gives_empty_cells_instead_of_an_error() {
    let cells = compute_grid(
        &layout(2, 2, 8, vec![cell("a", 1, 1, 1, 1)]),
        rect(0, 0, 10, 10),
        20,
    )
    .expect("lays out");
    assert_eq!((cells[0].rect.width, cells[0].rect.height), (0, 0));
}

#[test]
fn a_cell_outside_the_grid_is_refused() {
    for bad in [
        cell("a", 3, 1, 1, 1),
        cell("a", 0, 1, 1, 1),
        cell("a", 2, 1, 2, 1),
        cell("a", 1, 2, 1, 2),
        cell("a", 1, 1, 0, 1),
    ] {
        let result = compute_grid(&layout(2, 2, 8, vec![bad]), rect(0, 0, 800, 600), 20);
        assert!(matches!(result, Err(EngineError::Invalid(_))), "{result:?}");
    }
}

#[test]
fn overlapping_cells_and_an_empty_grid_are_refused() {
    let overlapping = layout(2, 2, 8, vec![cell("a", 1, 1, 1, 2), cell("b", 1, 2, 2, 1)]);
    assert!(matches!(
        compute_grid(&overlapping, rect(0, 0, 800, 600), 20),
        Err(EngineError::Invalid(_))
    ));
    assert!(matches!(
        compute_grid(&layout(0, 1, 8, vec![]), rect(0, 0, 800, 600), 20),
        Err(EngineError::Invalid(_))
    ));
}

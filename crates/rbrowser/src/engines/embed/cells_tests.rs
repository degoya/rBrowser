use std::collections::HashMap;

use rb_config::EngineKind;
use rb_engine::ViewId;
use rb_engine::external::{CellPixels, CellScale};

use super::*;

fn cell(width: u32, height: u32) -> CellPixels {
    CellPixels { width, height }
}

fn view(id: u32, engine: EngineKind, width: u32, pixels: Option<CellPixels>) -> CellInput {
    CellInput {
        view: ViewId(id),
        engine,
        viewport_width: width,
        pixels,
        wanted: true,
        running: true,
    }
}

#[test]
fn a_webkit_view_gets_its_own_quantized_factor_and_the_cells_height() {
    let scales = plan(&[view(3, EngineKind::Webkit, 768, Some(cell(640, 500)))]);
    assert_eq!(
        scales.per_view,
        HashMap::from([(
            ViewId(3),
            Some(CellScale {
                factor: 0.82,
                width: 768,
                height: 610,
            })
        )])
    );
    assert_eq!(scales.firefox, None);
}

#[test]
fn every_firefox_view_shares_the_smallest_factor_and_keeps_its_own_height() {
    let scales = plan(&[
        view(3, EngineKind::Firefox, 768, Some(cell(640, 500))),
        view(4, EngineKind::Firefox, 1280, Some(cell(640, 500))),
    ]);
    assert_eq!(scales.firefox, Some(0.5));
    assert_eq!(
        scales.per_view[&ViewId(3)],
        Some(CellScale {
            factor: 0.5,
            width: 768,
            height: 1000,
        })
    );
    assert_eq!(
        scales.per_view[&ViewId(4)],
        Some(CellScale {
            factor: 0.5,
            width: 1280,
            height: 1000,
        })
    );
}

#[test]
fn a_view_not_wanted_in_its_cell_or_without_a_measured_cell_is_unscaled() {
    let mut released = view(3, EngineKind::Firefox, 768, Some(cell(640, 500)));
    released.wanted = false;
    let scales = plan(&[
        released,
        view(4, EngineKind::Webkit, 768, None),
        view(5, EngineKind::Firefox, 768, Some(cell(1000, 500))),
    ]);
    assert_eq!(scales.per_view[&ViewId(3)], None);
    assert_eq!(scales.per_view[&ViewId(4)], None);
    // The released view does not pull the shared factor down.
    assert_eq!(scales.firefox, Some(1.0));
    assert_eq!(
        scales.per_view[&ViewId(5)],
        Some(CellScale {
            factor: 1.0,
            width: 768,
            height: 500,
        })
    );
    assert_eq!(plan(&[]).firefox, None);
}

/// A view that does not run (closed by hand, its engine away) has no window to fit, so it
/// pulls the shared factor of the others down for nothing (review finding 4); it keeps the
/// scale it would get at that factor, which its restart opens with.
#[test]
fn a_view_that_does_not_run_pulls_no_factor_but_keeps_its_scale() {
    let mut closed = view(3, EngineKind::Firefox, 1920, Some(cell(320, 500)));
    closed.running = false;
    let scales = plan(&[
        closed,
        view(5, EngineKind::Firefox, 768, Some(cell(640, 500))),
    ]);
    assert_eq!(scales.firefox, Some(0.82));
    assert_eq!(
        scales.per_view[&ViewId(3)],
        Some(CellScale {
            factor: 0.82,
            width: 1920,
            height: 610,
        })
    );
    let mut alone = view(3, EngineKind::Firefox, 1920, Some(cell(320, 500)));
    alone.running = false;
    assert_eq!(plan(&[alone]).firefox, None);
}

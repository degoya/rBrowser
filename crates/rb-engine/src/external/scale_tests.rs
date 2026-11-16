use super::*;

fn cell(width: u32, height: u32) -> CellPixels {
    CellPixels { width, height }
}

#[test]
fn the_factor_fits_the_breakpoint_width_into_the_cell_at_most_one_to_one() {
    // The owner's case: md (768) in a cell of 640 x 500.
    let factor = cell_factor(cell(640, 500), 768);
    assert!((factor - 640.0 / 768.0).abs() < 1e-9, "{factor}");
    // A cell wider than the breakpoint shows the page at 100 %, like a Chromium cell.
    assert_eq!(cell_factor(cell(1200, 500), 768), 1.0);
    assert_eq!(cell_factor(cell(768, 500), 768), 1.0);
}

#[test]
fn the_factor_never_falls_below_the_minimum_and_survives_bad_input() {
    assert_eq!(cell_factor(cell(10, 500), 1920), MIN_FACTOR);
    assert_eq!(cell_factor(cell(0, 0), 768), MIN_FACTOR);
    assert_eq!(cell_factor(cell(640, 500), 0), 1.0);
}

#[test]
fn quantizing_rounds_down_to_the_step_so_the_page_never_outgrows_the_cell() {
    assert_eq!(quantize(640.0 / 768.0), 0.82);
    assert_eq!(quantize(1.0), 1.0);
    assert_eq!(quantize(0.5), 0.5);
    assert_eq!(quantize(0.333), 0.32);
    // Held to the range as well.
    assert_eq!(quantize(0.1), MIN_FACTOR);
    assert_eq!(quantize(2.0), 1.0);
    assert_eq!(quantize(f64::NAN), 1.0);
}

#[test]
fn a_shared_factor_is_the_smallest_so_every_page_fits_its_cell() {
    assert_eq!(shared_factor([0.82, 0.5, 1.0]), Some(0.5));
    assert_eq!(shared_factor([]), None);
    assert_eq!(shared_factor([f64::NAN, 0.7]), Some(0.7));
}

#[test]
fn the_cell_scale_keeps_the_breakpoint_width_and_fills_the_cell_height() {
    let scale = cell_scale(cell(640, 500), 768, 0.82);
    assert_eq!(
        scale,
        CellScale {
            factor: 0.82,
            width: 768,
            height: 610,
        }
    );
    // A factor of 1: the cell's height as it is.
    assert_eq!(cell_scale(cell(900, 700), 768, 1.0).height, 700);
    // Never a viewport without height or width.
    let empty = cell_scale(cell(0, 0), 0, 0.5);
    assert_eq!((empty.width, empty.height), (1, 1));
}

#[test]
fn the_pref_value_is_the_factor_with_two_decimals() {
    assert_eq!(pref_value(0.82), "0.82");
    assert_eq!(pref_value(1.0), "1.00");
    assert_eq!(pref_value(0.5), "0.50");
}

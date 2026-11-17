use super::*;
use crate::layout::overlaps;

fn items(widths: &[u32]) -> Vec<Item> {
    widths
        .iter()
        .enumerate()
        .map(|(index, width)| Item {
            breakpoint: format!("bp{index}"),
            width: *width,
            height: 800,
        })
        .collect()
}

fn area(width: u32, height: u32) -> CellRect {
    CellRect {
        x: 0,
        y: 44,
        width,
        height,
    }
}

fn strip() -> Strip {
    Strip {
        height: StripHeight::Fit,
        gap: 8,
        label: 20,
        zoom: Zoom::Fit,
        first: 0,
    }
}

fn widths_of(slots: &[Slot]) -> Vec<u32> {
    slots.iter().map(|slot| slot.rect.width).collect()
}

fn assert_no_overlap(slots: &[Slot], area: CellRect) {
    let shown: Vec<&Slot> = slots.iter().filter(|slot| slot.visible).collect();
    for (index, slot) in shown.iter().enumerate() {
        assert!(contains(area, slot.outer()), "{slot:?} leaves {area:?}");
        assert!(
            !overlaps(slot.label, slot.rect),
            "label over view: {slot:?}"
        );
        for other in &shown[index + 1..] {
            assert!(!overlaps(slot.outer(), other.outer()), "{slot:?} {other:?}");
        }
    }
}

/// Mobile, tablet and desktop fit the row at three window sizes, at one scale, the same height.
#[test]
fn fit_rows_at_three_window_sizes() {
    let items = items(&[393, 768, 1440]);
    for (size, widths) in [
        ((1920, 1000), [285, 557, 1045]),
        ((1280, 700), [189, 368, 691]),
        ((800, 600), [116, 227, 425]),
    ] {
        let area = area(size.0, size.1);
        let slots = arrange(&items, area, &strip());
        assert_eq!(widths_of(&slots), widths, "{size:?}");
        assert!(slots.iter().all(|slot| slot.visible), "{size:?}: all fit");
        let heights: Vec<u32> = slots.iter().map(|slot| slot.rect.height).collect();
        assert_eq!(heights, [size.1 - 16 - 20; 3], "{size:?}: same height");
        assert_eq!(slots[0].label.x, 8);
        assert_eq!(slots[0].label.y, 44 + 8);
        assert_eq!(slots[0].rect.y, 44 + 8 + 20);
        assert_eq!(slots[1].label.x, 8 + widths[0] as i32 + 8);
        assert_no_overlap(&slots, area);
    }
}

#[test]
fn a_fixed_height_is_kept_below_the_area() {
    let items = items(&[393, 768]);
    let mut fixed = strip();
    fixed.height = StripHeight::Px(400);
    let slots = arrange(&items, area(1280, 700), &fixed);
    assert!(slots.iter().all(|slot| slot.rect.height == 400));
    fixed.height = StripHeight::Px(5000);
    let slots = arrange(&items, area(1280, 700), &fixed);
    assert!(slots.iter().all(|slot| slot.rect.height == 700 - 16 - 20));
}

/// Eight desktop views do not fit 800 DIP even at the smallest scale: the row scrolls one view
/// at a time and only whole views are shown.
#[test]
fn a_row_too_wide_scrolls_and_hides_what_is_out_of_sight() {
    let items = items(&[1440; 8]);
    let area = area(800, 600);
    assert_eq!(scale(&items, area, &strip()), MIN_SCALE);
    let slots = arrange(&items, area, &strip());
    let shown: Vec<bool> = slots.iter().map(|slot| slot.visible).collect();
    assert_eq!(
        shown,
        [true, true, false, false, false, false, false, false]
    );
    assert_no_overlap(&slots, area);

    let mut scrolled = strip();
    scrolled.first = step(&items, area, &strip(), true);
    assert_eq!(scrolled.first, 1);
    let slots = arrange(&items, area, &scrolled);
    assert_eq!(slots[1].label.x, 8, "the first view sits at the left edge");
    assert!(!slots[0].visible && slots[1].visible && slots[2].visible);
    assert_no_overlap(&slots, area);
}

#[test]
fn scrolling_stops_where_the_rest_fits() {
    let items = items(&[1440; 8]);
    let area = area(800, 600);
    assert_eq!(last_first(&items, area, &strip()), 6);
    let mut far = strip();
    far.first = 99;
    let slots = arrange(&items, area, &far);
    assert!(slots[6].visible && slots[7].visible);
    far.first = 6;
    assert_eq!(step(&items, area, &far, true), 6);
    far.first = 0;
    assert_eq!(step(&items, area, &far, false), 0);
}

#[test]
fn reveal_scrolls_only_when_the_view_is_hidden() {
    let items = items(&[1440; 8]);
    let area = area(800, 600);
    assert_eq!(reveal(&items, area, &strip(), 1), 0, "shown already");
    assert_eq!(reveal(&items, area, &strip(), 4), 4);
    assert_eq!(reveal(&items, area, &strip(), 7), 6);
}

#[test]
fn a_zoom_factor_is_taken_as_it_is_and_one_view_never_outgrows_the_area() {
    let items = items(&[1440, 393]);
    let mut zoomed = strip();
    zoomed.zoom = Zoom::Factor(1.0);
    let slots = arrange(&items, area(1000, 600), &zoomed);
    assert_eq!(widths_of(&slots), [1000 - 16, 393]);
    assert!(slots[0].visible && !slots[1].visible);
}

#[test]
fn no_views_and_no_area_give_nothing_shown() {
    assert!(arrange(&[], area(800, 600), &strip()).is_empty());
    let slots = arrange(&items(&[393]), area(0, 0), &strip());
    assert!(!slots[0].visible);
}

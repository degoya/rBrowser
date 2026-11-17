use super::*;
use crate::layout::{contains, overlaps};

fn area(width: u32, height: u32) -> CellRect {
    CellRect {
        x: 0,
        y: 44,
        width,
        height,
    }
}

fn split(orientation: SplitOrientation, ratio: f64) -> Split {
    Split {
        orientation,
        ratio,
        gap: 8,
        label: 20,
    }
}

fn assert_clean(slots: &[Slot], area: CellRect) {
    for (index, slot) in slots.iter().enumerate() {
        assert!(slot.visible, "{slot:?}");
        assert!(contains(area, slot.outer()), "{slot:?} leaves {area:?}");
        assert_eq!(slot.label.height, 20);
        assert!(!overlaps(slot.label, slot.rect));
        for other in &slots[index + 1..] {
            assert!(!overlaps(slot.outer(), other.outer()), "{slot:?} {other:?}");
        }
    }
}

/// One large view left at 0.6, two stacked right, at three window sizes.
#[test]
fn a_vertical_split_at_three_window_sizes() {
    for (size, first, second, x2, height) in [
        ((1920, 1000), 1138, 758, 1154, 488),
        ((1280, 700), 754, 502, 770, 338),
        ((800, 600), 466, 310, 482, 288),
    ] {
        let area = area(size.0, size.1);
        let slots = arrange(1, 2, area, &split(SplitOrientation::Vertical, 0.6));
        assert_eq!(slots.len(), 3);
        assert_eq!(
            slots[0].outer(),
            CellRect {
                x: 8,
                y: 52,
                width: first,
                height: size.1 - 16
            }
        );
        assert_eq!(
            (
                slots[1].outer().x,
                slots[1].outer().width,
                slots[1].outer().height
            ),
            (x2, second, height),
            "{size:?}"
        );
        assert_eq!(slots[2].outer().y, 52 + height as i32 + 8);
        assert_clean(&slots, area);
        assert_eq!(
            divider(1, 2, area, &split(SplitOrientation::Vertical, 0.6)),
            Some(CellRect {
                x: 8 + first as i32,
                y: 44,
                width: 8,
                height: size.1
            })
        );
    }
}

/// The primary side on top, two side by side below, at three window sizes.
#[test]
fn a_horizontal_split_at_three_window_sizes() {
    for (size, top, width) in [
        ((1920, 1000), 488, 948),
        ((1280, 700), 338, 628),
        ((800, 600), 288, 388),
    ] {
        let area = area(size.0, size.1);
        let slots = arrange(1, 2, area, &split(SplitOrientation::Horizontal, 0.5));
        assert_eq!(slots[0].outer().height, top, "{size:?}");
        assert_eq!(slots[0].outer().width, size.0 - 16);
        assert_eq!(slots[1].outer().y, 52 + top as i32 + 8);
        assert_eq!(
            (slots[1].outer().width, slots[2].outer().width),
            (width, width)
        );
        assert_eq!(slots[2].outer().x, 8 + width as i32 + 8);
        assert_clean(&slots, area);
    }
}

#[test]
fn an_empty_side_gives_its_space_to_the_other() {
    let area = area(1280, 700);
    let only_primary = arrange(2, 0, area, &split(SplitOrientation::Vertical, 0.3));
    assert!(
        only_primary
            .iter()
            .all(|slot| slot.outer().width == 1280 - 16)
    );
    let only_secondary = arrange(0, 1, area, &split(SplitOrientation::Vertical, 0.3));
    assert_eq!(only_secondary[0].outer().width, 1280 - 16);
    assert_eq!(
        divider(0, 1, area, &split(SplitOrientation::Vertical, 0.3)),
        None
    );
    assert!(arrange(0, 0, area, &split(SplitOrientation::Vertical, 0.3)).is_empty());
}

#[test]
fn the_ratio_is_held_to_its_range() {
    assert_eq!(clamp_ratio(0.0), 0.1);
    assert_eq!(clamp_ratio(1.5), 0.9);
    assert_eq!(clamp_ratio(f64::NAN), 0.5);
    let area = area(1000, 600);
    let wide = arrange(1, 1, area, &split(SplitOrientation::Vertical, 5.0));
    assert_eq!(wide[0].outer().width, 878, "0.9 of 976");
}

#[test]
fn keyboard_steps_move_the_divider_and_stop_at_the_ends() {
    assert_eq!(step_ratio(0.5, 0.05, true), 0.55);
    assert_eq!(step_ratio(0.55, 0.05, false), 0.5);
    assert_eq!(step_ratio(0.88, 0.05, true), 0.9);
    assert_eq!(step_ratio(0.12, 0.05, false), 0.1);
}

#[test]
fn a_tiny_area_hides_what_has_no_room_for_a_view() {
    let slots = arrange(1, 3, area(100, 40), &split(SplitOrientation::Vertical, 0.5));
    assert!(slots[0].visible, "24 DIP: a label and 4 DIP of view");
    assert!(slots[1..].iter().all(|slot| !slot.visible), "{slots:?}");
}

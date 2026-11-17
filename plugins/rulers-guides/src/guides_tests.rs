use super::*;
use crate::model::{Axis, Guide, GuideUnit, MAX_GUIDES};

fn guide(id: u32, axis: Axis, unit: GuideUnit, value: f64) -> Guide {
    Guide {
        id,
        axis,
        unit,
        value,
    }
}

#[test]
fn a_50_per_cent_guide_lies_in_the_middle_of_every_width() {
    let half = guide(1, Axis::X, GuideUnit::Percent, 50.0);
    for width in [320.0, 375.0, 768.0, 1023.0, 1440.0] {
        assert_eq!(position(&half, width), width / 2.0);
    }
}

#[test]
fn a_px_guide_lies_at_its_px_in_every_width() {
    let at = guide(1, Axis::X, GuideUnit::Px, 320.0);
    for width in [375.0, 768.0, 1440.0] {
        assert_eq!(position(&at, width), 320.0);
    }
    assert_eq!(
        position(&guide(2, Axis::Y, GuideUnit::Px, 320.0), 375.0),
        320.0
    );
}

#[test]
fn a_px_from_the_right_guide_follows_the_right_edge() {
    let right = guide(1, Axis::X, GuideUnit::PxRight, 16.0);
    assert_eq!(position(&right, 375.0), 359.0);
    assert_eq!(position(&right, 1440.0), 1424.0);
}

#[test]
fn a_page_position_becomes_the_guides_value() {
    assert_eq!(value_at(Axis::X, GuideUnit::Percent, 187.5, 375.0), 50.0);
    assert_eq!(value_at(Axis::X, GuideUnit::PxRight, 359.0, 375.0), 16.0);
    assert_eq!(value_at(Axis::Y, GuideUnit::Px, 1234.5, 375.0), 1234.5);
    // Nothing outside the page: clamped to its edges.
    assert_eq!(value_at(Axis::X, GuideUnit::Percent, 500.0, 375.0), 100.0);
    assert_eq!(value_at(Axis::X, GuideUnit::Px, -5.0, 375.0), 0.0);
    // A page of no width gives 0 %, never NaN.
    assert_eq!(value_at(Axis::X, GuideUnit::Percent, 5.0, 0.0), 0.0);
}

#[test]
fn snapping_rounds_to_the_step() {
    assert_eq!(snap(203.0, 8), 200.0);
    assert_eq!(snap(204.0, 8), 208.0);
    assert_eq!(snap(203.4, 1), 203.0);
    assert_eq!(snap(203.4, 0), 203.4);
}

#[test]
fn adding_gives_the_next_id_and_refuses_past_the_limit() {
    let mut guides = vec![guide(4, Axis::X, GuideUnit::Px, 1.0)];
    assert_eq!(add(&mut guides, Axis::Y, GuideUnit::Px, 20.0), Ok(5));
    assert_eq!(guides.len(), 2);
    assert_eq!(
        add(&mut guides, Axis::Y, GuideUnit::Percent, 20.0),
        Err("guide.unit")
    );
    let mut full: Vec<Guide> = (0..MAX_GUIDES as u32)
        .map(|id| guide(id, Axis::Y, GuideUnit::Px, 1.0))
        .collect();
    assert_eq!(add(&mut full, Axis::Y, GuideUnit::Px, 2.0), Err("guides"));
}

#[test]
fn moving_and_removing_find_the_guide_by_id() {
    let mut guides = vec![
        guide(1, Axis::X, GuideUnit::Percent, 10.0),
        guide(2, Axis::Y, GuideUnit::Px, 10.0),
    ];
    assert_eq!(move_to(&mut guides, 1, 25.0), Ok(()));
    assert_eq!(guides[0].value, 25.0);
    assert_eq!(move_to(&mut guides, 1, 125.0), Err("guide.value"));
    assert_eq!(guides[0].value, 25.0);
    assert_eq!(move_to(&mut guides, 9, 1.0), Err("guide.id"));
    assert!(remove(&mut guides, 2));
    assert!(!remove(&mut guides, 2));
    assert_eq!(guides.len(), 1);
}

#[test]
fn positions_are_listed_with_their_axis() {
    let guides = vec![
        guide(1, Axis::X, GuideUnit::Percent, 50.0),
        guide(2, Axis::Y, GuideUnit::Px, 320.0),
    ];
    let placed = place(&guides, 375.0);
    assert_eq!(placed.len(), 2);
    assert_eq!(
        (placed[0].id, placed[0].axis, placed[0].pos),
        (1, Axis::X, 187.5)
    );
    assert_eq!(
        (placed[1].id, placed[1].axis, placed[1].pos),
        (2, Axis::Y, 320.0)
    );
}

use super::*;

fn rect(x: i32, y: i32, width: u32, height: u32) -> CellRect {
    CellRect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn contains_and_overlaps_treat_edges_as_touching() {
    let area = rect(0, 0, 100, 100);
    assert!(contains(area, rect(0, 0, 100, 100)));
    assert!(!contains(area, rect(1, 0, 100, 100)));
    assert!(!contains(area, rect(-1, 0, 10, 10)));
    assert!(
        !overlaps(rect(0, 0, 10, 10), rect(10, 0, 10, 10)),
        "touching"
    );
    assert!(overlaps(rect(0, 0, 10, 10), rect(9, 9, 10, 10)));
}

#[test]
fn a_slot_is_a_label_strip_over_its_view() {
    let slot = split_label(rect(5, 50, 200, 300), 20, true);
    assert_eq!(slot.label, rect(5, 50, 200, 20));
    assert_eq!(slot.rect, rect(5, 70, 200, 280));
    assert_eq!(slot.outer(), rect(5, 50, 200, 300));
    let thin = split_label(rect(0, 0, 10, 12), 20, false);
    assert_eq!((thin.label.height, thin.rect.height), (12, 0));
}

#[test]
fn to_dip_rounds_and_holds_the_range() {
    assert_eq!(to_dip(2.5), 3);
    assert_eq!(to_dip(-4.0), 0);
    assert_eq!(to_dip(f64::NAN), 0);
    assert_eq!(to_dip(1e20), u32::MAX);
}

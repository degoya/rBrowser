use super::{Count, covered, length, merge, overlaps, used_segments};

#[test]
fn overlapping_and_touching_ranges_become_one() {
    let merged = merge(vec![(10, 20), (0, 5), (5, 8), (15, 30), (40, 40), (35, 36)]);
    assert_eq!(merged, vec![(0, 8), (10, 30), (35, 36)]);
    assert_eq!(length(&merged), 8 + 20 + 1);
}

#[test]
fn an_empty_or_reversed_range_counts_nothing() {
    assert_eq!(merge(vec![(5, 5), (9, 3)]), Vec::<(u32, u32)>::new());
    assert_eq!(length(&[]), 0);
}

#[test]
fn the_innermost_range_decides_as_v8_reports_blocks() {
    // A script of 100 characters: the top level ran once, a function 10..60 ran, a branch
    // 20..30 inside it did not, a second function 70..90 never ran.
    let ranges = [
        Count::new(0, 100, 1),
        Count::new(10, 60, 2),
        Count::new(20, 30, 0),
        Count::new(70, 90, 0),
    ];
    assert_eq!(used_segments(&ranges), vec![(0, 20), (30, 70), (90, 100)]);
    assert_eq!(length(&used_segments(&ranges)), 70);
}

#[test]
fn a_never_executed_script_has_no_used_segment() {
    let ranges = [Count::new(0, 50, 0), Count::new(5, 10, 0)];
    assert!(used_segments(&ranges).is_empty());
}

#[test]
fn unsorted_input_and_equal_starts_keep_the_nesting() {
    // Same start: the shorter range is the inner one.
    let ranges = [
        Count::new(0, 10, 0),
        Count::new(0, 40, 1),
        Count::new(35, 38, 0),
    ];
    assert_eq!(used_segments(&ranges), vec![(10, 35), (38, 40)]);
}

#[test]
fn a_rule_is_covered_when_a_used_range_lies_over_it() {
    let used = merge(vec![(0, 10), (50, 80)]);
    assert!(covered(&used, 52, 70));
    assert!(!covered(&used, 5, 20));
    assert!(overlaps(&used, 5, 20));
    assert!(!overlaps(&used, 20, 50));
}

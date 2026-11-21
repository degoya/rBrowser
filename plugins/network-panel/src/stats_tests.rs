use serde_json::json;

use super::{Spread, median, percentile, sorted};

#[test]
fn the_median_of_an_odd_and_an_even_count() {
    assert_eq!(median(&sorted([5.0, 1.0, 3.0])), Some(3.0));
    assert_eq!(median(&sorted([4.0, 1.0, 3.0, 2.0])), Some(2.5));
    assert_eq!(median(&[]), None);
}

#[test]
fn the_95th_percentile_is_the_nearest_rank() {
    let twenty: Vec<f64> = (1..=20).map(f64::from).collect();
    assert_eq!(percentile(&twenty, 95.0), Some(19.0));
    let hundred: Vec<f64> = (1..=100).map(f64::from).collect();
    assert_eq!(percentile(&hundred, 95.0), Some(95.0));
    assert_eq!(percentile(&[7.0], 95.0), Some(7.0));
    assert_eq!(percentile(&[], 95.0), None);
}

#[test]
fn values_that_are_no_numbers_are_left_out_and_the_rest_sorted() {
    assert_eq!(sorted([3.0, f64::NAN, 1.0, f64::INFINITY]), vec![1.0, 3.0]);
}

#[test]
fn a_spread_answers_median_and_p95_rounded_or_nothing() {
    assert_eq!(
        Spread::of([10.04, 20.0, 30.0]).map(|spread| spread.json()),
        Some(json!({ "median": 20.0, "p95": 30.0 }))
    );
    assert_eq!(Spread::of([1.234]).map(|spread| spread.median), Some(1.2));
    assert!(Spread::of(Vec::<f64>::new()).is_none());
}

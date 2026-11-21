use super::*;

#[test]
fn a_percentile_is_the_nearest_rank() {
    let sorted = [10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0];
    assert_eq!(percentile(&sorted, 0.0), Some(10.0));
    assert_eq!(percentile(&sorted, 50.0), Some(50.0));
    assert_eq!(percentile(&sorted, 95.0), Some(100.0));
    assert_eq!(percentile(&sorted, 90.0), Some(90.0));
    assert_eq!(percentile(&sorted, 100.0), Some(100.0));
    assert_eq!(percentile(&sorted, 150.0), Some(100.0));
    assert_eq!(percentile(&[], 50.0), None);
    assert_eq!(percentile(&[7.0], 99.0), Some(7.0));
}

#[test]
fn a_summary_holds_min_median_p95_max_and_mean() {
    let samples = [30.0, 10.0, 20.0, 40.0];
    let summary = Summary::of(&samples).expect("samples");
    assert_eq!(
        summary,
        Summary {
            count: 4,
            min: 10.0,
            median: 20.0,
            p95: 40.0,
            max: 40.0,
            mean: 25.0,
        }
    );
}

#[test]
fn samples_that_are_no_number_are_left_out() {
    let summary = Summary::of(&[f64::NAN, 5.0, f64::INFINITY]).expect("one sample");
    assert_eq!(summary.count, 1);
    assert_eq!(summary.min, 5.0);
    assert_eq!(Summary::of(&[f64::NAN]), None);
    assert_eq!(Summary::of(&[]), None);
}

#[test]
fn the_summary_of_many_samples_keeps_the_order_statistics() {
    let samples: Vec<f64> = (1..=100).rev().map(f64::from).collect();
    let summary = Summary::of(&samples).expect("samples");
    assert_eq!(summary.median, 50.0);
    assert_eq!(summary.p95, 95.0);
    assert_eq!(summary.min, 1.0);
    assert_eq!(summary.max, 100.0);
    assert_eq!(sorted(&samples).first(), Some(&1.0));
}

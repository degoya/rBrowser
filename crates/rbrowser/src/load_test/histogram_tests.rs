use super::*;

#[test]
fn an_empty_histogram_knows_nothing() {
    let histogram = Histogram::default();
    assert_eq!(histogram.count(), 0);
    assert_eq!(histogram.percentile(50.0), None);
    assert_eq!(histogram.latency(), None);
}

#[test]
fn percentiles_are_within_a_percent_of_the_samples() {
    let mut histogram = Histogram::default();
    for ms in 1..=1000 {
        histogram.add(f64::from(ms));
    }
    let near = |p: f64, want: f64| {
        let got = histogram.percentile(p).expect("samples");
        assert!((got - want).abs() <= want * 0.011, "p{p}: {got} for {want}");
    };
    near(50.0, 500.0);
    near(95.0, 950.0);
    near(99.0, 990.0);
    let latency = histogram.latency().expect("samples");
    assert_eq!((latency.min, latency.max), (1.0, 1000.0));
    assert!((latency.mean - 500.5).abs() < 1e-9);
}

#[test]
fn extremes_and_odd_samples_stay_in_range() {
    let mut histogram = Histogram::default();
    histogram.add(0.0);
    histogram.add(1e12);
    histogram.add(f64::NAN);
    histogram.add(-5.0);
    assert_eq!(histogram.count(), 2);
    assert_eq!(histogram.percentile(100.0), Some(1e12));
    assert_eq!(histogram.percentile(1.0), Some(0.0));
}

#[test]
fn two_histograms_merge() {
    let (mut a, mut b) = (Histogram::default(), Histogram::default());
    a.add(10.0);
    b.add(20.0);
    a.merge(&b);
    assert_eq!(a.count(), 2);
    assert_eq!(a.latency().expect("samples").max, 20.0);
}

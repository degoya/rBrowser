use super::*;
use crate::load_test::scenario::AbortRules;
use crate::load_test::stats::Window;

fn rules() -> AbortRules {
    AbortRules {
        error_rate: Some(0.2),
        p95_ms: Some(1000.0),
        window_s: 10,
        min_requests: 20,
    }
}

fn window(requests: u64, errors: u64, p95: Option<f64>) -> Window {
    Window {
        requests,
        errors,
        p95,
    }
}

#[test]
fn too_many_failures_stop_the_run() {
    let abort = judge(&rules(), &window(100, 21, Some(10.0))).expect("aborts");
    assert_eq!(abort.rule, "errorRate");
    assert!((abort.value - 0.21).abs() < 1e-9);
    assert!((abort.threshold - 0.2).abs() < 1e-9);
    assert_eq!(judge(&rules(), &window(100, 20, Some(10.0))), None);
}

#[test]
fn a_slow_p95_stops_the_run() {
    let abort = judge(&rules(), &window(100, 0, Some(1500.0))).expect("aborts");
    assert_eq!(abort.rule, "p95Ms");
    assert_eq!(abort.value, 1500.0);
    assert_eq!(judge(&rules(), &window(100, 0, Some(1000.0))), None);
}

#[test]
fn few_requests_are_not_judged_and_no_rule_never_stops() {
    assert_eq!(judge(&rules(), &window(19, 19, Some(9999.0))), None);
    assert_eq!(
        judge(&AbortRules::default(), &window(1000, 1000, Some(9e9))),
        None
    );
}

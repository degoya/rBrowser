use std::time::{Duration, Instant};

use super::*;

#[test]
fn the_phases_follow_the_marks() {
    let t0 = Instant::now();
    let at = |ms: u64| t0 + Duration::from_millis(ms);
    let times = Times {
        started: Some(at(0)),
        resolve_started: Some(at(1)),
        resolved: Some(at(11)),
        connected: Some(at(31)),
        tls: Some(at(61)),
    };
    let phases = times.phases(at(161), at(200));
    assert_eq!(phases.dns, Some(10.0));
    assert_eq!(phases.connect, Some(20.0));
    assert_eq!(phases.tls, Some(30.0));
    assert_eq!(phases.first_byte, 100.0);
    assert_eq!(phases.total, 200.0);
}

#[test]
fn a_phase_that_did_not_happen_is_none() {
    let t0 = Instant::now();
    let at = |ms: u64| t0 + Duration::from_millis(ms);
    let times = Times {
        started: Some(at(0)),
        ..Times::default()
    };
    let phases = times.phases(at(5), at(7));
    assert_eq!(phases.dns, None);
    assert_eq!(phases.tls, None);
    assert_eq!(phases.first_byte, 5.0);
    assert_eq!(phases.total, 7.0);
}

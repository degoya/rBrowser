use super::*;

#[test]
fn the_check_and_the_claim_run_under_one_gate() {
    // A recording's start waits while a flow checks and claims, and the other way round.
    let held = exclusively(|| GATE.try_lock().is_err());
    assert!(held);
    assert!(GATE.try_lock().is_ok());
}

#[test]
fn no_flow_runs_before_one_starts() {
    assert_eq!(running_flow(), None);
}

use std::sync::{Arc, Mutex};

use super::*;
use crate::cdp::lock;

fn pauser_at(now: &Arc<Mutex<f64>>) -> Pauser {
    let clock = Arc::clone(now);
    Pauser::new(Arc::new(move || *lock(&clock)))
}

#[test]
fn a_pause_and_its_resume_are_logged_on_the_clock() {
    let now = Arc::new(Mutex::new(100.0));
    let pauser = pauser_at(&now);
    let mut follower = pauser.subscribe();
    assert!(!pauser.is_paused());
    assert!(pauser.pause());
    assert!(
        follower.has_changed().unwrap_or(false),
        "the writers learn it"
    );
    drop(follower.borrow_and_update());
    assert!(!pauser.pause(), "paused already");
    assert!(
        !follower.has_changed().unwrap_or(true),
        "nothing new to learn"
    );
    *lock(&now) = 103.5;
    assert!(pauser.resume());
    assert!(!pauser.is_paused());
    assert!((pauser.pauses().ended_secs() - 3.5).abs() < 1e-9);
    assert!(!pauser.resume(), "not paused");
}

#[test]
fn a_toggle_pauses_a_running_recording_and_resumes_a_paused_one() {
    let now = Arc::new(Mutex::new(10.0));
    let pauser = pauser_at(&now);
    let shared = pauser.clone();
    assert_eq!(
        apply(&pauser, PauseRequest::Toggle),
        PauseState {
            paused: true,
            changed: true
        }
    );
    assert!(shared.is_paused(), "clones share the log");
    assert_eq!(
        apply(&shared, PauseRequest::Pause),
        PauseState {
            paused: true,
            changed: false
        }
    );
    *lock(&now) = 12.0;
    assert_eq!(
        apply(&pauser, PauseRequest::Toggle),
        PauseState {
            paused: false,
            changed: true
        }
    );
    assert_eq!(
        apply(&pauser, PauseRequest::Resume),
        PauseState {
            paused: false,
            changed: false
        }
    );
}

#[test]
fn seconds_become_whole_milliseconds() {
    assert_eq!(millis(1.2345), 1235);
    assert_eq!(millis(0.0), 0);
    assert_eq!(millis(-3.0), 0);
    assert_eq!(millis(f64::NAN), 0);
    assert_eq!(millis(1_794_910_500.25), 1_794_910_500_250);
}

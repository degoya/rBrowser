use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use super::Hold;

/// How long a test gives a thread that should have finished: a give-up deadline, never a bound.
const GIVE_UP: Duration = Duration::from_secs(30);

#[test]
fn an_open_gate_lets_through_at_once() {
    let hold = Hold::new();
    assert!(!hold.is_held());
    hold.wait();
    assert!(!hold.release(), "nothing was held");
}

/// RB-091-08: the load waits while the first choice is open, and goes on once it is answered.
#[test]
fn a_held_gate_waits_until_it_is_released() {
    let hold = Arc::new(Hold::new());
    assert!(hold.try_hold());
    assert!(hold.is_held());
    let (done, finished) = mpsc::channel();
    let waiter = {
        let hold = hold.clone();
        std::thread::spawn(move || {
            hold.wait();
            let _ = done.send(());
        })
    };
    // Still waiting a while later (a lower bound: it holds under any load).
    assert!(
        finished.recv_timeout(Duration::from_millis(100)).is_err(),
        "the waiter went on before the answer"
    );
    assert!(hold.release(), "it was held");
    finished
        .recv_timeout(GIVE_UP)
        .expect("the waiter goes on after the answer");
    waiter.join().expect("the waiter ends");
    assert!(!hold.is_held());
    assert!(!hold.release(), "released once");
}

#[test]
fn every_waiter_goes_on_after_one_release() {
    let hold = Arc::new(Hold::new());
    assert!(hold.try_hold());
    let (done, finished) = mpsc::channel();
    let waiters: Vec<_> = (0..3)
        .map(|_| {
            let (hold, done) = (hold.clone(), done.clone());
            std::thread::spawn(move || {
                hold.wait();
                let _ = done.send(());
            })
        })
        .collect();
    hold.release();
    for _ in 0..3 {
        finished.recv_timeout(GIVE_UP).expect("a waiter goes on");
    }
    for waiter in waiters {
        waiter.join().expect("ends");
    }
}

/// Review of RB-091-08: a load already under way (begun before the hold) is visible - the hold is
/// refused, so the wizard's answer says "next start", never "live".
#[test]
fn a_load_under_way_refuses_the_hold() {
    let hold = Hold::new();
    assert!(hold.begin_load(), "nothing holds: the load may start");
    assert!(!hold.try_hold(), "the load runs already");
    assert!(!hold.is_held());
    assert!(!hold.release(), "nothing was held: restart");
}

/// The other order: a hold taken first keeps every load from starting until the answer.
#[test]
fn a_hold_taken_first_keeps_the_load_from_starting() {
    let hold = Hold::new();
    assert!(hold.try_hold());
    assert!(!hold.begin_load(), "held: no load, nothing kept");
    assert!(hold.release(), "held: the answer loads live");
    assert!(hold.begin_load());
    assert!(!hold.try_hold(), "never held again once the load began");
}

/// The window never opened or the app quits: waiters go on and nothing loads any more.
#[test]
fn an_abandoned_hold_wakes_its_waiters_and_loads_nothing() {
    let hold = Arc::new(Hold::new());
    assert!(hold.try_hold());
    let (done, finished) = mpsc::channel();
    let waiter = {
        let hold = hold.clone();
        std::thread::spawn(move || {
            hold.wait();
            let _ = done.send(());
        })
    };
    hold.abandon();
    finished
        .recv_timeout(GIVE_UP)
        .expect("the waiter goes on after the hold is abandoned");
    waiter.join().expect("the waiter ends");
    assert!(!hold.begin_load(), "abandoned: no load starts");
    assert!(!hold.try_hold());
}

use std::sync::mpsc;

use super::*;

/// One check at a time: a caller that asked while another check ran takes that outcome instead
/// of running its own (RB-090-14 review).
#[test]
fn a_caller_that_waited_for_a_running_check_takes_its_outcome() {
    let serial = Serial::new();
    let waiting = serial.ticket();
    // Another caller's check runs and finishes meanwhile.
    let ran = serial.run(serial.ticket(), || "checked", || "joined");
    assert_eq!(ran, "checked");
    assert_eq!(serial.run(waiting, || "checked", || "joined"), "joined");
    // A caller after it checks again.
    assert_eq!(
        serial.run(serial.ticket(), || "checked", || "joined"),
        "checked"
    );
}

/// Two threads: the second asks while the first runs, waits, and joins.
#[test]
fn a_second_check_waits_for_the_first_and_joins_it() {
    static SERIAL: Serial = Serial::new();
    let (started, on_start) = mpsc::channel();
    let (release, on_release) = mpsc::channel::<()>();
    let first = std::thread::spawn(move || {
        SERIAL.run(
            SERIAL.ticket(),
            || {
                started.send(()).expect("started");
                on_release.recv().expect("released");
                "checked"
            },
            || "joined",
        )
    });
    on_start.recv().expect("the first runs");
    let ticket = SERIAL.ticket();
    let second = std::thread::spawn(move || SERIAL.run(ticket, || "checked", || "joined"));
    release.send(()).expect("release");
    assert_eq!(first.join().expect("first"), "checked");
    assert_eq!(second.join().expect("second"), "joined");
}

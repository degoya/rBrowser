use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use super::*;

#[test]
fn sending_does_not_wait_for_a_blocked_write_and_the_order_holds() {
    let written = Arc::new(Mutex::new(Vec::new()));
    let (release, gate) = mpsc::channel::<()>();
    let writer = ThreadWriter::spawn("rb-test-writer", {
        let written = Arc::clone(&written);
        move |frame: &[u8]| {
            // The first write blocks as a full pipe would.
            let _ = gate.recv();
            crate::cdp::lock(&written).push(frame.to_vec());
            Ok(())
        }
    });
    for frame in [b"a".to_vec(), b"b".to_vec(), b"c".to_vec()] {
        writer.send(frame).expect("handed to the writer at once");
    }
    for _ in 0..3 {
        release.send(()).expect("the writer waits");
    }
    for _ in 0..200 {
        if crate::cdp::lock(&written).len() == 3 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        *crate::cdp::lock(&written),
        [b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]
    );
}

#[test]
fn after_a_failed_write_every_send_fails() {
    let writer = ThreadWriter::spawn("rb-test-writer", |_: &[u8]| Err(EngineError::Closed));
    let _ = writer.send(b"x".to_vec());
    for _ in 0..200 {
        if writer.send(b"y".to_vec()).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the writer still took frames after its write failed");
}

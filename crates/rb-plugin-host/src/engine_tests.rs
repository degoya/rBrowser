use std::time::{Duration, Instant};

use super::SharedEngine;
use crate::runtime::EPOCH_TICK;

/// The smallest valid component: the preamble of the component encoding and nothing else.
pub(crate) const EMPTY_COMPONENT: [u8; 8] = [0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00];

/// The same component with a custom section appended, so its bytes - and nothing about
/// what it does - differ.
pub(crate) fn variant(tag: u8) -> Vec<u8> {
    let mut bytes = EMPTY_COMPONENT.to_vec();
    // Section id 0 (custom), 3 bytes: a one-byte name "t", then the payload.
    bytes.extend_from_slice(&[0x00, 0x03, 0x01, b't', tag]);
    bytes
}

#[test]
fn a_component_is_compiled_once_per_content() {
    let shared = SharedEngine::new(None).expect("engine");
    shared.compile(&EMPTY_COMPONENT).expect("first compile");
    shared.compile(&EMPTY_COMPONENT).expect("second compile");
    assert_eq!(shared.compilations(), 1, "the same bytes compile once");

    shared.compile(&variant(1)).expect("changed bytes");
    assert_eq!(shared.compilations(), 2, "different bytes compile anew");
}

#[test]
fn a_component_that_does_not_compile_is_not_remembered() {
    let shared = SharedEngine::new(None).expect("engine");
    assert!(shared.compile(b"not a component").is_err());
    assert!(shared.compile(b"not a component").is_err());
    assert_eq!(shared.compilations(), 0);
}

#[test]
fn a_cache_folder_that_cannot_be_made_private_leaves_the_engine_running_without_it() {
    let root = tempfile::tempdir().expect("temp dir");
    // A file where the folder should be: neither created nor restricted.
    let blocked = root.path().join("cache");
    std::fs::write(&blocked, b"not a folder").expect("write");
    let shared = SharedEngine::new(Some(&blocked)).expect("the engine runs without a disk cache");
    assert!(shared.disk.is_none());
    shared
        .compile(&EMPTY_COMPONENT)
        .expect("compiles in memory");
}

/// Polls `done` until it holds; panics after a generous give-up deadline (no upper bound on the
/// time it takes).
fn wait_until(done: impl Fn() -> bool) {
    let give_up = Instant::now() + Duration::from_secs(30);
    while !done() {
        if Instant::now() >= give_up {
            panic!("gave up waiting");
        }
        std::thread::sleep(EPOCH_TICK);
    }
}

/// RB-090-38: idle, the ticker thread sleeps without waking; a call keeps the epoch moving, and
/// after the last one it stops again.
#[test]
fn the_epoch_ticks_only_while_a_call_runs() {
    let shared = SharedEngine::new(None).expect("engine");
    std::thread::sleep(EPOCH_TICK * 5);
    assert_eq!(shared.ticks(), 0, "no call: no tick");
    let first = shared.ticking();
    let second = shared.ticking();
    wait_until(|| shared.ticks() >= 2);
    drop(first);
    let before = shared.ticks();
    wait_until(|| shared.ticks() >= before + 2);
    drop(second);
    let stopped = shared.ticks();
    std::thread::sleep(EPOCH_TICK * 5);
    assert!(
        shared.ticks() <= stopped + 1,
        "after the last call at most the tick in flight"
    );
    let again = shared.ticking();
    let resumed = shared.ticks();
    wait_until(|| shared.ticks() > resumed);
    drop(again);
}

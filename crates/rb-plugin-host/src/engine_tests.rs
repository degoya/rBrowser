use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{EngineSlot, SharedEngine};
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

/// Every file under `root`, at any depth.
fn files_under(root: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(root) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() { files_under(&path) } else { 1 }
        })
        .sum()
}

/// RB-094-30: `plugins.compileCache` applies to the next load - on, the engine of the next
/// compile reads and writes the disk cache; off, nothing is read from or written to it. Plugins
/// loaded before keep the engine they were compiled on.
#[test]
fn the_compile_cache_choice_applies_to_the_next_engine() {
    let root = tempfile::tempdir().expect("temp dir");
    let slot = EngineSlot::new();
    let memory = slot.get().expect("engine");
    assert!(memory.disk.is_none(), "no cache until one is chosen");
    assert!(
        Arc::ptr_eq(&memory, &slot.get().expect("engine")),
        "the same choice keeps the engine"
    );

    slot.set_cache(Some(root.path()));
    let cached = slot.get().expect("engine");
    assert!(cached.disk.is_some());
    assert!(!Arc::ptr_eq(&memory, &cached), "a new choice: a new engine");
    slot.set_cache(Some(root.path()));
    assert!(Arc::ptr_eq(&cached, &slot.get().expect("engine")));
    cached.compile(&EMPTY_COMPONENT).expect("compiles");
    let written = files_under(root.path());
    assert!(written > 0, "on: the compiled code is kept on disk");

    slot.set_cache(None);
    let off = slot.get().expect("engine");
    assert!(off.disk.is_none());
    off.compile(&variant(9)).expect("compiles in memory");
    assert_eq!(
        files_under(root.path()),
        written,
        "off: nothing is written to the cache"
    );
    memory
        .compile(&variant(3))
        .expect("an engine handed out before still compiles");
}

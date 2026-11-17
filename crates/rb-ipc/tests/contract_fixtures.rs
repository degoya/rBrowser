//! The Rust half of the Rust <-> TS contract test: every message of
//! `ui/src/ipc/contract-fixtures.json` (one per type, typed against `protocol.ts` by
//! `ui/src/ipc/contract.test.ts`) parses into the Rust types and is written back unchanged.

use rb_ipc::{CoreToShell, ShellToCore, parse_request};
use serde_json::Value;

fn fixtures() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../ui/src/ipc/contract-fixtures.json"
    );
    let text = std::fs::read_to_string(path).expect("the fixtures file is part of the repository");
    serde_json::from_str(&text).expect("the fixtures are JSON")
}

fn entries<'a>(fixtures: &'a Value, side: &str) -> &'a Vec<Value> {
    fixtures[side]
        .as_array()
        .unwrap_or_else(|| panic!("`{side}` is a list"))
}

#[test]
fn every_shell_message_round_trips() {
    let fixtures = fixtures();
    let list = entries(&fixtures, "shellToCore");
    assert!(!list.is_empty());
    for entry in list {
        let message: ShellToCore =
            parse_request(&entry.to_string()).unwrap_or_else(|error| panic!("{entry}: {error}"));
        let back = serde_json::to_value(&message).expect("serializes");
        assert_eq!(&back, entry, "shellToCore entry changed on the way back");
    }
}

#[test]
fn every_core_message_round_trips() {
    let fixtures = fixtures();
    let list = entries(&fixtures, "coreToShell");
    assert!(!list.is_empty());
    for entry in list {
        let message: CoreToShell = serde_json::from_value(entry.clone())
            .unwrap_or_else(|error| panic!("{entry}: {error}"));
        let back = serde_json::to_value(&message).expect("serializes");
        assert_eq!(&back, entry, "coreToShell entry changed on the way back");
    }
}

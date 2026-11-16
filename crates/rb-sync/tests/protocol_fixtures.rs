//! The Rust half of the Rust <-> TS contract test of the sync protocol (RB-020-01): every
//! message of `agent/src/protocol-fixtures.json` (typed against `protocol.ts` by
//! `agent/src/protocol.test.ts`) parses into the Rust types and is written back unchanged.

use rb_sync::{AgentCommand, AgentMessage, CommandResult, parse_message};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn fixtures() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../agent/src/protocol-fixtures.json"
    );
    let text = std::fs::read_to_string(path).expect("the fixtures file is part of the repository");
    serde_json::from_str(&text).expect("the fixtures are JSON")
}

fn round_trip<T: DeserializeOwned + Serialize>(side: &str) -> usize {
    let fixtures = fixtures();
    let list = fixtures[side]
        .as_array()
        .unwrap_or_else(|| panic!("`{side}` is a list"));
    assert!(!list.is_empty());
    for entry in list {
        let parsed: T = serde_json::from_value(entry.clone())
            .unwrap_or_else(|error| panic!("{entry}: {error}"));
        let back = serde_json::to_value(&parsed).expect("serializes");
        assert_eq!(&back, entry, "`{side}` entry changed on the way back");
    }
    list.len()
}

#[test]
fn every_agent_message_round_trips() {
    assert!(round_trip::<AgentMessage>("agentToCore") >= 20);
}

#[test]
fn every_agent_message_passes_the_payload_parser() {
    let fixtures = fixtures();
    for entry in fixtures["agentToCore"].as_array().into_iter().flatten() {
        parse_message(&entry.to_string()).unwrap_or_else(|error| panic!("{entry}: {error}"));
    }
}

#[test]
fn every_command_round_trips() {
    round_trip::<AgentCommand>("coreToAgent");
}

#[test]
fn every_result_round_trips() {
    round_trip::<CommandResult>("results");
}

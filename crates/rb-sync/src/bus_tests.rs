use std::cell::Cell;

use serde::{Serialize, Serializer};
use serde_json::json;

use super::*;
use crate::{AgentCommand, AgentMessage, parse_message};

/// A payload that counts how often it is serialised.
struct Counting(Cell<usize>);

impl Serialize for Counting {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.set(self.0.get() + 1);
        "payload".serialize(serializer)
    }
}

fn settings() -> BusSettings {
    BusSettings {
        enabled: true,
        max_payload_bytes: 1024,
        max_per_second: 100,
    }
}

fn message(r#type: &str, payload: Value) -> BusMessage {
    BusMessage {
        v: 1,
        top: true,
        frame: None,
        time: 1,
        origin: "https://app.test".into(),
        r#type: r#type.into(),
        payload,
    }
}

#[test]
fn bus_types_are_short_names() {
    for good in ["canvas:pan", "a", "app/state.v2", "x_y-z", &"A".repeat(64)] {
        assert!(is_bus_type(good), "{good}");
    }
    for bad in ["", ":pan", &"A".repeat(65), "a b", "a\nb", "<x>", "\u{e4}"] {
        assert!(!is_bus_type(bad), "{bad}");
    }
}

#[test]
fn a_valid_message_passes_with_its_payload_serialised() {
    let checked = check_message(&message("canvas:pan", json!({ "x": 1 })), &settings());
    assert_eq!(
        checked,
        Ok(BusPayload::of(&json!({ "x": 1 })).expect("serializes"))
    );
    assert_eq!(checked.expect("passes").get(), r#"{"x":1}"#);
}

/// RB-080-27: the check serialises the payload once; the deliveries to every view pass those
/// bytes on and never serialise it again.
#[test]
fn a_payload_is_serialised_once_for_the_check_and_every_delivery() {
    let payload = Counting(Cell::new(0));
    let checked = check_payload("flow:step", &payload, &settings()).expect("fits");
    assert_eq!(checked.len(), r#""payload""#.len());
    let command = AgentCommand::Bus {
        from: None,
        origin: None,
        r#type: "flow:step".into(),
        payload: checked,
    };
    for _ in 0..8 {
        let text = serde_json::to_string(&command).expect("serializes");
        assert!(text.contains(r#""payload":"payload""#), "{text}");
    }
    assert_eq!(payload.0.get(), 1);
}

#[test]
fn the_payload_is_measured_as_json_bytes() {
    // 1022 characters and the two quotes: exactly the limit.
    let fits = message("x", json!("y".repeat(1022)));
    assert!(check_message(&fits, &settings()).is_ok());
    let over = message("x", json!("y".repeat(1023)));
    assert_eq!(
        check_message(&over, &settings()),
        Err(BusError::TooLarge {
            size: 1025,
            limit: 1024
        })
    );
    assert_eq!(
        BusPayload::of(&json!("\u{e4}")).expect("serializes").len(),
        4
    );
}

#[test]
fn refuses_what_the_core_does_not_relay() {
    let off = BusSettings {
        enabled: false,
        ..settings()
    };
    assert_eq!(
        check_message(&message("x", json!(1)), &off),
        Err(BusError::Off)
    );
    assert_eq!(
        check_message(&message("a b", json!(1)), &settings()),
        Err(BusError::Type)
    );
    let framed = BusMessage {
        top: false,
        ..message("x", json!(1))
    };
    assert_eq!(check_message(&framed, &settings()), Err(BusError::NotTop));
    for origin in ["", "null", &"h".repeat(MAX_ORIGIN_LEN + 1)] {
        let opaque = BusMessage {
            origin: origin.into(),
            ..message("x", json!(1))
        };
        assert_eq!(check_message(&opaque, &settings()), Err(BusError::Origin));
    }
}

#[test]
fn errors_never_quote_the_payload() {
    let over = message("x", json!("hunter2".repeat(500)));
    let text = check_message(&over, &settings())
        .expect_err("over the limit")
        .to_string();
    assert!(!text.contains("hunter2"), "{text}");
}

#[test]
fn settings_are_held_to_their_ranges() {
    let wild = BusSettings {
        enabled: true,
        max_payload_bytes: u32::MAX,
        max_per_second: 0,
    };
    assert_eq!(
        wild.clamped(),
        BusSettings {
            enabled: true,
            max_payload_bytes: MAX_PAYLOAD_LIMIT,
            max_per_second: 1
        }
    );
    assert_eq!(BusSettings::default().max_payload_bytes, 65_536);
    assert_eq!(BusSettings::default().max_per_second, 100);
}

#[test]
fn the_rate_limiter_drops_above_the_rate() {
    let mut limiter = RateLimiter::new(100, 1_000);
    let passed = (0..1000).filter(|_| limiter.take(1_000)).count();
    assert_eq!(passed, 100);
    let later = (0..1000).filter(|_| limiter.take(1_500)).count();
    assert_eq!(later, 50);
    // Never more than one second stored.
    let much_later = (0..1000).filter(|_| limiter.take(90_000)).count();
    assert_eq!(much_later, 100);
    // A clock that goes back refills nothing.
    assert!(!(0..10).any(|_| limiter.take(10)));
}

#[test]
fn a_new_rate_caps_what_is_stored() {
    let mut limiter = RateLimiter::new(10, 0);
    limiter.set_rate(2, 0);
    let passed = (0..10).filter(|_| limiter.take(0)).count();
    assert_eq!(passed, 2);
}

#[test]
fn a_bus_message_parses_from_the_binding() {
    let payload = r#"{"kind":"bus","v":1,"top":true,"time":5,"origin":"https://app.test","type":"canvas:pan","payload":{"x":1}}"#;
    let parsed = parse_message(payload).expect("a valid bus message");
    assert_eq!(
        parsed,
        AgentMessage::Bus(message("canvas:pan", json!({ "x": 1 })).with_time(5))
    );
    assert_eq!(parsed.version(), 1);
}

#[test]
fn the_bus_command_leaves_out_what_is_not_set() {
    let command = AgentCommand::Bus {
        from: None,
        origin: None,
        r#type: "flow:step".into(),
        payload: BusPayload::of(&Value::Null).expect("serializes"),
    };
    assert_eq!(
        serde_json::to_value(&command).expect("serializes"),
        json!({ "cmd": "bus", "type": "flow:step", "payload": null })
    );
}

impl BusMessage {
    fn with_time(mut self, time: u64) -> Self {
        self.time = time;
        self
    }
}

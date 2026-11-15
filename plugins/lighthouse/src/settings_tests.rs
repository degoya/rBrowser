use super::*;
use crate::fake::FakeHost;

#[test]
fn throttling_is_simulated_unless_set() {
    assert_eq!(throttling(&mut FakeHost::default()), "simulate");
    let mut host = FakeHost::default().set("throttling", "\"devtools\"");
    assert_eq!(throttling(&mut host), "devtools");
}

#[test]
fn a_value_outside_the_schema_is_the_default() {
    let mut host = FakeHost::default().set("throttling", "\"fast\"");
    assert_eq!(throttling(&mut host), "simulate");
    let mut host = FakeHost::default().set("throttling", "3");
    assert_eq!(throttling(&mut host), "simulate");
}

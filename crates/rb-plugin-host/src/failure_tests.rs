use rb_plugin_types::codes;

use super::{readable, stopped};

#[test]
fn readable_spells_out_what_would_hide() {
    assert_eq!(readable("a\nb", 10), "a\\nb");
    assert_eq!(readable("x\u{202e}y", 10), "x\\u{202e}y");
    assert_eq!(readable("abcdef", 3), "abc\u{2026}");
}

#[test]
fn a_call_the_wall_clock_ended_timed_out() {
    assert_eq!(stopped(None).code(), Some(codes::TIMED_OUT));
}

#[test]
fn traps_get_their_codes() {
    let fuel = wasmtime::Error::new(wasmtime::Trap::OutOfFuel);
    assert_eq!(stopped(Some(&fuel)).code(), Some(codes::OUT_OF_FUEL));
    let epoch = wasmtime::Error::new(wasmtime::Trap::Interrupt);
    assert_eq!(stopped(Some(&epoch)).code(), Some(codes::TIMED_OUT));
    let crash = wasmtime::Error::new(wasmtime::Trap::UnreachableCodeReached);
    assert_eq!(stopped(Some(&crash)).code(), Some(codes::CRASHED));
}

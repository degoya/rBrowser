use super::*;

#[test]
fn the_report_names_place_message_and_backtrace() {
    let payload: Box<dyn Any + Send> = Box::new("boom");
    let report = panic_report(
        payload.as_ref(),
        Some(Location::caller()),
        &"frame 0\nframe 1",
    );
    assert!(report.starts_with("panicked at "), "{report}");
    assert!(report.contains("panic_tests.rs:"), "{report}");
    assert!(
        report.contains(": boom\nbacktrace:\nframe 0\nframe 1"),
        "{report}"
    );
}

#[test]
fn a_string_payload_and_a_foreign_payload_are_read() {
    let owned: Box<dyn Any + Send> = Box::new(String::from("owned message"));
    assert!(panic_report(owned.as_ref(), None, &"").contains(": owned message"));
    let foreign: Box<dyn Any + Send> = Box::new(42_u8);
    let report = panic_report(foreign.as_ref(), None, &"");
    assert!(report.contains("an unknown place"), "{report}");
    assert!(report.contains("a panic without a message"), "{report}");
}

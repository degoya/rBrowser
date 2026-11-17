use std::path::Path;

use super::*;

#[test]
fn an_elevated_start_is_named() {
    let failure = StartFailure::new(38);
    assert_eq!(failure.reason_key(), "startup.cefFailed.elevated");
    assert_eq!(failure.exit_code(), 38);
}

#[test]
fn a_profile_still_in_use_is_named() {
    // PROFILE_IN_USE, and NORMAL_EXIT_PROCESS_NOTIFIED: another process holds the profile.
    for code in [21, 24] {
        assert_eq!(
            StartFailure::new(code).reason_key(),
            "startup.cefFailed.profileInUse"
        );
    }
}

#[test]
fn the_sandbox_and_unknown_codes_are_named() {
    assert_eq!(
        StartFailure::new(31).reason_key(),
        "startup.cefFailed.sandbox"
    );
    assert_eq!(StartFailure::new(7).reason_key(), "startup.cefFailed.other");
}

#[test]
fn without_a_code_the_process_still_fails() {
    assert_eq!(StartFailure::new(0).exit_code(), 1);
    assert_eq!(StartFailure::new(-5).exit_code(), 1);
}

#[test]
fn the_message_names_the_reason_the_code_and_the_log() {
    let failure = StartFailure::new(21);
    let text = failure.message(Path::new(r"C:\p\profile"), Some(Path::new(r"C:\p\logs")));
    assert!(text.contains(r"C:\p\profile"), "{text}");
    assert!(text.contains("21"), "{text}");
    assert!(text.contains(r"C:\p\logs"), "{text}");
    assert!(
        !text.contains("startup.cefFailed"),
        "a key is missing: {text}"
    );
    let unknown = StartFailure::new(7).message(Path::new("p"), None);
    assert!(unknown.contains("administrator"), "{unknown}");
}

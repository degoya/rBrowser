use super::*;

fn origin(url: &str) -> AuthOrigin {
    AuthOrigin::parse(url).expect(url)
}

#[test]
fn a_view_gets_one_answer_per_origin_until_the_credential_changes() {
    let mut attempts = Attempts::default();
    let staging = origin("https://staging.example.test/");
    assert!(
        attempts.take(1, &staging, "Staging"),
        "the first challenge is answered"
    );
    // The same challenge again means the server refused the credential: the person is asked,
    // rBrowser does not send it in a loop.
    assert!(!attempts.take(1, &staging, "Staging"));
    assert!(!attempts.take(1, &staging, "Staging"));
    // Another view (its own browser) and another origin each get their own answer.
    assert!(attempts.take(2, &staging, "Staging"));
    assert!(attempts.take(1, &origin("https://staging.example.test:8443/"), "Staging"));
    // A new credential is tried once more everywhere.
    attempts.reset();
    assert!(attempts.take(1, &staging, "Staging"));
    assert!(attempts.take(2, &staging, "Staging"));
    assert!(!attempts.take(2, &staging, "Staging"));
}

/// RB-094-10 (review w28): Chromium caches an answer per origin and realm, so a second realm on
/// one origin is a first challenge of its own and is answered once too.
#[test]
fn a_second_realm_on_one_origin_gets_its_own_answer() {
    let mut attempts = Attempts::default();
    let staging = origin("https://staging.example.test/");
    assert!(attempts.take(1, &staging, "Staging"));
    assert!(attempts.take(1, &staging, "Admin area"));
    assert!(!attempts.take(1, &staging, "Admin area"));
    assert!(!attempts.take(1, &staging, "Staging"));
}

/// Review w35: an answer the server accepted is no refusal - the same challenge long after it
/// (a new session's first load, the auth cache cleared) is answered from the store again.
#[test]
fn a_challenge_long_after_the_answer_is_answered_again() {
    use std::time::{Duration, Instant};

    use super::super::prompt::REFUSAL_WINDOW;

    let mut attempts = Attempts::default();
    let staging = origin("https://staging.example.test/");
    let start = Instant::now();
    assert!(attempts.take_at(1, &staging, "Staging", start));
    assert!(!attempts.take_at(1, &staging, "Staging", start + Duration::from_secs(1)));
    let later = start + REFUSAL_WINDOW + Duration::from_secs(2);
    assert!(attempts.take_at(1, &staging, "Staging", later));
    assert!(!attempts.take_at(1, &staging, "Staging", later));
}

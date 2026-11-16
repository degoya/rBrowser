use std::time::{Duration, Instant};

use rb_plugin_types::{Failure, FailureKind, codes};

use super::{LOG_EVERY, PANEL_KEYS, Throttle, host_texts, throttle_key};

fn timeout() -> Failure {
    Failure::coded(FailureKind::Other, codes::HOST_FAILED).with_param("reason", "timeout")
}

/// RB-080-90: a panel got only its plugin's catalogue and showed `plugins.error.host_failed`;
/// it gets the host codes' texts now, and only texts - never a key as its own text.
#[test]
fn a_panel_gets_the_host_codes_texts() {
    let texts = host_texts();
    let host_failed = texts.get(codes::HOST_FAILED).expect("the host_failed text");
    assert!(!host_failed.starts_with("plugins."), "{host_failed}");
    assert!(texts.iter().all(|(key, text)| key != text));
    assert!(
        texts
            .keys()
            .all(|key| codes::ALL.contains(&key.as_str()) || PANEL_KEYS.contains(&key.as_str()))
    );
}

#[test]
fn the_same_failure_is_logged_once_a_minute() {
    let throttle = Throttle::default();
    let start = Instant::now();
    let key = throttle_key("fonts", "list", &timeout());
    assert_eq!(key, "fonts/list/plugins.error.host_failed/timeout");
    assert!(throttle.allows(&key, start));
    assert!(!throttle.allows(&key, start + Duration::from_secs(59)));
    // Another reason, another call or another plugin is another line.
    let missing = Failure::coded(FailureKind::NotFound, codes::NOT_FOUND);
    assert!(throttle.allows(&throttle_key("fonts", "list", &missing), start));
    assert!(throttle.allows(&throttle_key("fonts", "mark", &timeout()), start));
    assert!(throttle.allows(&key, start + LOG_EVERY));
}

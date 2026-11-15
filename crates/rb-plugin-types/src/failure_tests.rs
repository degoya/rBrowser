use super::{Failure, FailureKind};

#[test]
fn a_failure_shows_its_code_or_its_class() {
    assert_eq!(Failure::new(FailureKind::NotFound).to_string(), "not_found");
    let coded = Failure::coded(FailureKind::AccessDenied, "plugins.error.capability_denied")
        .with_param("capability", "http");
    assert_eq!(coded.to_string(), "plugins.error.capability_denied");
    assert_eq!(coded.param("capability"), Some("http"));
}

#[test]
fn a_failure_round_trips_through_json() {
    let failure = Failure::coded(FailureKind::Limit, "plugins.error.timed_out").with_param("n", 3);
    let json = serde_json::to_string(&failure).expect("json");
    assert!(json.contains("\"kind\":\"limit\""), "{json}");
    let back: Failure = serde_json::from_str(&json).expect("parse");
    assert_eq!(back, failure);
    let bare: Failure = serde_json::from_str(r#"{"kind":"io"}"#).expect("parse");
    assert_eq!(bare, Failure::new(FailureKind::Io));
}

#[test]
fn every_class_has_its_own_name() {
    let mut names: Vec<&str> = FailureKind::ALL.iter().map(|kind| kind.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), FailureKind::ALL.len());
}

#[test]
fn a_view_of_an_engine_that_cannot_serve_is_told_apart_from_other_failures() {
    let unserved = Failure::engine_unserved("evaluate", 2, "webkit");
    assert_eq!(unserved.code(), Some(crate::codes::NOT_AVAILABLE));
    assert_eq!(unserved.param("service"), Some("evaluate"));
    assert_eq!(unserved.param("view"), Some("2"));
    // Only for the view it names: a refusal of another view is not this view's skip.
    assert_eq!(unserved.unserved_engine_in(2), Some("webkit"));
    assert_eq!(unserved.unserved_engine_in(1), None);
    let not_wired = Failure::coded(FailureKind::NotSupported, crate::codes::NOT_AVAILABLE)
        .with_param("service", "sidecar");
    assert_eq!(not_wired.unserved_engine_in(2), None);
    let other = Failure::coded(FailureKind::Other, crate::codes::TIMED_OUT)
        .with_param("engine", "webkit")
        .with_param("view", "2");
    assert_eq!(other.unserved_engine_in(2), None);
}

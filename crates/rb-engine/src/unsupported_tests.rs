use super::*;
use crate::test_support::conformance::{check_unavailable, sample_spec};

#[tokio::test]
async fn an_engine_that_has_not_landed_is_unavailable_and_opens_nothing() {
    for kind in [EngineKind::Firefox, EngineKind::Webkit] {
        let engine = UnsupportedEngine::not_yet(kind);
        assert!(engine.external());
        assert!(engine.reason().contains("not available yet"));
        check_unavailable(&engine, &sample_spec(kind, 3)).await;
    }
}

#[tokio::test]
async fn the_reason_is_the_one_given() {
    let engine = UnsupportedEngine::because(EngineKind::Firefox, "Firefox is not installed".into());
    assert_eq!(
        engine.health(),
        EngineHealth::Unavailable("Firefox is not installed".into())
    );
    let error = engine
        .create_view(&sample_spec(EngineKind::Firefox, 3))
        .await
        .err()
        .expect("no view");
    assert_eq!(error.to_string(), "Firefox is not installed");
}

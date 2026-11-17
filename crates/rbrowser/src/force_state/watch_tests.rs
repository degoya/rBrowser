use std::collections::BTreeSet;

use rb_engine::ViewId;
use serde_json::json;

use super::*;
use crate::force_state::model::PseudoClass;

fn registry() -> Registry {
    let mut registry = Registry::default();
    registry.open(ViewId(1), "OWN1".to_owned(), 1);
    let hover: BTreeSet<PseudoClass> = [PseudoClass::Hover].into_iter().collect();
    registry.apply(ViewId(1), 42, vec![10, 9], hover);
    registry
}

fn event(method: &str, session: Option<&str>, params: serde_json::Value) -> CdpEvent {
    CdpEvent {
        method: method.to_owned(),
        params,
        session_id: session.map(str::to_owned),
    }
}

#[test]
fn a_new_document_on_rbrowsers_session_ends_the_views_states() {
    let mut registry = registry();
    let ended = on_event(
        &mut registry,
        &event(DOCUMENT_UPDATED, Some("OWN1"), json!({})),
    );
    assert_eq!(ended, Some((ViewId(1), true)));
    assert!(registry.forced_views().is_empty());
}

/// DevTools force on their own session: their events change nothing of rBrowser's.
#[test]
fn another_sessions_events_are_not_rbrowsers() {
    let mut registry = registry();
    assert_eq!(
        on_event(
            &mut registry,
            &event(DOCUMENT_UPDATED, Some("DEVTOOLS"), json!({}))
        ),
        None
    );
    assert_eq!(
        on_event(&mut registry, &event(DOCUMENT_UPDATED, None, json!({}))),
        None
    );
    assert_eq!(
        on_event(
            &mut registry,
            &event("Page.frameNavigated", Some("OWN1"), json!({}))
        ),
        None
    );
    assert_eq!(registry.forced_views(), [ViewId(1)]);
}

#[test]
fn a_detached_session_forgets_the_view() {
    let mut registry = registry();
    let ended = on_event(
        &mut registry,
        &event(
            DETACHED,
            None,
            json!({ "sessionId": "OWN1", "targetId": "T" }),
        ),
    );
    assert_eq!(ended, Some((ViewId(1), true)));
    assert_eq!(registry.session(ViewId(1)), None);
    assert_eq!(
        on_event(
            &mut registry,
            &event(DETACHED, None, json!({ "sessionId": "OTHER" }))
        ),
        None
    );
}

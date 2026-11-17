use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};

use super::*;
use crate::registry::tests::def;
use crate::{CommandId, CommandRegistry, Group};

fn registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();
    registry
        .register(
            def(
                "layout.zoom_to_fit",
                Group::Layout,
                &["Ctrl+Shift+Space L Z"],
            ),
            Arc::new(|_args| Ok(json!("fitted"))),
        )
        .expect("registers");
    registry
}

#[test]
fn canvas_zoom_fit_is_zoom_to_fit_now() {
    assert_eq!(successor("canvas.zoom.fit"), Some("layout.zoom_to_fit"));
    assert_eq!(successor("layout.zoom_to_fit"), None);
}

// RB-090-63 review: a retired id still runs, as its successor (a palette entry, a script, an
// agent's runCommand by id).
#[test]
fn a_retired_id_runs_its_successor() {
    let registry = registry();
    let old = CommandId::new("canvas.zoom.fit").expect("an id");
    assert_eq!(
        registry.dispatch(&old, Value::Null).expect("runs"),
        json!("fitted")
    );
    let handler = registry.handler(&old).expect("the successor's handler");
    assert_eq!(handler(Value::Null).expect("runs"), json!("fitted"));
    let unknown = CommandId::new("canvas.zoom.gone").expect("an id");
    assert!(registry.handler(&unknown).is_none());
}

// A user's binding of a retired id loads without a notice at every start, and binds nothing.
#[test]
fn a_saved_binding_of_a_retired_id_is_dropped_without_a_notice() {
    let mut registry = registry();
    let saved = BTreeMap::from([(
        "canvas.zoom.fit".to_owned(),
        vec!["Ctrl+Shift+Space L A".to_owned()],
    )]);
    assert!(registry.load_user_bindings(&saved).is_empty());
    assert!(registry.user_bindings().is_empty());
}

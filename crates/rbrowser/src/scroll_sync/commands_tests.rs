use rb_commands::CommandError;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::scroll_sync::tests::setup_on;
use crate::sync_hub::on_start;

fn features(core: &Core) -> rb_config::SyncConfig {
    crate::ipc::snapshot(core.host()).sync.features
}

#[tokio::test(flavor = "multi_thread")]
async fn the_toggle_needs_the_hub_and_answers_only_for_itself() {
    let setup = setup_on(&Handle::current());
    assert!(matches!(
        run_command(&setup.core, TOGGLE_SCROLL, &Value::Null),
        Some(Err(CommandError::Failed(_)))
    ));
    assert!(run_command(&setup.core, "sync.toggle", &Value::Null).is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_toggle_switches_scroll_sync_and_leaves_click_sync_on() {
    let setup = setup_on(&Handle::current());
    on_start(&setup.core);
    let core = &setup.core;
    assert!(features(core).scroll);
    assert_eq!(
        run_command(core, TOGGLE_SCROLL, &Value::Null),
        Some(Ok(json!({ "scroll": false })))
    );
    let now = features(core);
    assert!(!now.scroll);
    assert!(now.click && now.input && now.hover);
    assert_eq!(
        run_command(core, TOGGLE_SCROLL, &json!({ "enabled": true })),
        Some(Ok(json!({ "scroll": true })))
    );
    assert!(features(core).scroll);
    assert!(matches!(
        run_command(core, TOGGLE_SCROLL, &json!({ "on": true })),
        Some(Err(CommandError::InvalidArgs(_)))
    ));
}

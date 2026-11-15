use rb_commands::CommandError;
use serde_json::{Value, json};

use super::*;
use crate::sync_hub::on_start;
use crate::sync_hub::tests::setup;

fn focus_on(core: &Core) -> bool {
    crate::ipc::snapshot(core.host()).sync.features.focus
}

#[tokio::test(flavor = "multi_thread")]
async fn toggle_focus_needs_the_hub() {
    let setup = setup();
    assert!(matches!(
        run_command(&setup.core, TOGGLE_FOCUS, &Value::Null),
        Some(Err(CommandError::Failed(_)))
    ));
    assert!(run_command(&setup.core, "sync.toggle", &Value::Null).is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn toggle_focus_flips_or_sets_focus_sync() {
    let setup = setup();
    on_start(&setup.core);
    let core = &setup.core;
    assert!(!focus_on(core), "off by default");
    assert_eq!(
        core.run_command(TOGGLE_FOCUS, Value::Null),
        Ok(json!({ "focus": true }))
    );
    assert!(focus_on(core));
    let sync = for_host(core.host()).expect("started");
    let agent = sync.agent_config(&core.host().config().sync, rb_engine::ViewId(1));
    assert!(agent.capture.focus, "the agents capture focus now");
    assert_eq!(
        core.run_command(TOGGLE_FOCUS, json!({ "enabled": false })),
        Ok(json!({ "focus": false }))
    );
    assert!(matches!(
        core.run_command(TOGGLE_FOCUS, json!({ "on": true })),
        Err(CommandError::InvalidArgs(_))
    ));
}

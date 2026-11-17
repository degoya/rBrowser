use rb_commands::CommandError;
use serde_json::json;

use super::*;
use crate::sync_hub::on_start;
use crate::sync_hub::tests::setup;

fn state(core: &Core) -> rb_ipc::SyncState {
    crate::ipc::snapshot(core.host()).sync
}

#[tokio::test(flavor = "multi_thread")]
async fn the_sync_commands_need_the_hub() {
    let setup = setup();
    assert!(matches!(
        run_command(&setup.core, TOGGLE, &Value::Null),
        Some(Err(CommandError::Failed(_)))
    ));
    assert!(run_command(&setup.core, "view.back", &Value::Null).is_none());
    let message = ShellToCore::SetSync {
        feature: SyncFeature::All,
        enabled: false,
    };
    assert!(ipc(&setup.core, &message).is_none(), "501 until it runs");
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_toggle_flips_or_sets_the_global_switch() {
    let setup = setup();
    on_start(&setup.core);
    let core = &setup.core;
    assert!(state(core).enabled);
    assert_eq!(
        core.run_command(TOGGLE, Value::Null),
        Ok(json!({ "enabled": false }))
    );
    assert!(!state(core).enabled);
    assert_eq!(
        core.run_command(TOGGLE, json!({ "enabled": true })),
        Ok(json!({ "enabled": true }))
    );
    assert!(matches!(
        core.run_command(TOGGLE, json!({ "on": true })),
        Err(CommandError::InvalidArgs(_))
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_toggle_view_takes_the_active_view_out_and_back() {
    let setup = setup();
    on_start(&setup.core);
    let core = &setup.core;
    assert!(matches!(
        core.run_command(TOGGLE_VIEW, Value::Null),
        Err(CommandError::Failed(_))
    ));
    core.host().set_active_view(ViewId(2)).expect("a view");
    assert_eq!(
        core.run_command(TOGGLE_VIEW, Value::Null),
        Ok(json!({ "view": 2, "enabled": false }))
    );
    assert_eq!(state(core).excluded, vec![ViewId(2)]);
    assert_eq!(
        core.run_command(TOGGLE_VIEW, json!({ "view": 2, "enabled": true })),
        Ok(json!({ "view": 2, "enabled": true }))
    );
    assert!(state(core).excluded.is_empty());
    assert!(matches!(
        core.run_command(TOGGLE_VIEW, json!({ "view": 9 })),
        Err(CommandError::InvalidArgs(_))
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_skip_next_and_resync_answer() {
    let setup = setup();
    on_start(&setup.core);
    let core = &setup.core;
    assert_eq!(core.run_command(SKIP_NEXT, Value::Null), Ok(Value::Null));
    let sync = for_host(core.host()).expect("started");
    assert!(sync.hub().with(|hub| hub.status().skip_next_click));
    assert_eq!(
        core.run_command(RESYNC, Value::Null),
        Ok(json!({ "views": [] })),
        "nothing desynced"
    );
    assert_eq!(
        core.run_command(RESYNC, json!({ "view": 3 })),
        Ok(json!({ "views": [3] }))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn set_sync_switches_sync_or_one_kind() {
    let setup = setup();
    on_start(&setup.core);
    let core = &setup.core;
    let answer = crate::ipc::dispatch(
        core,
        &json!({ "type": "setSync", "feature": "click", "enabled": false }).to_string(),
    )
    .expect("answered");
    assert!(answer.contains("ack"), "{answer}");
    let now = state(core);
    assert!(now.enabled && !now.features.click && now.features.hover);
    crate::ipc::dispatch(
        core,
        &json!({ "type": "setSync", "feature": "all", "enabled": false }).to_string(),
    )
    .expect("answered");
    assert!(!state(core).enabled);
}

#[test]
fn every_feature_names_its_kind() {
    assert_eq!(kind_of(SyncFeature::All), None);
    assert_eq!(kind_of(SyncFeature::Hover), Some(SyncKind::Hover));
    assert_eq!(kind_of(SyncFeature::Navigation), Some(SyncKind::Navigation));
}

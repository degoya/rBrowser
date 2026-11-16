use serde_json::{Value, json};

use super::*;
use crate::layout::defs::{MINIMAP_TOGGLE, MODE_CANVAS, MODE_HORIZONTAL, MODE_SPLIT};
use crate::layout::test_support::{config, setup};

fn state(setup: &crate::layout::test_support::Setup) -> Option<ModeState> {
    let host = setup.core.host();
    mode_state(host, &host.shell_state().modes)
}

fn run(core: &crate::control::Core, id: &str) {
    crate::layout::actions::run(core, id, &Value::Null).expect("runs");
}

#[tokio::test]
async fn each_mode_reports_its_controls() {
    let setup = setup(config());
    assert_eq!(state(&setup), None, "the grid has none");
    run(&setup.core, MODE_SPLIT);
    assert_eq!(
        state(&setup).and_then(|state| state.split),
        Some(SplitState {
            ratio_permille: 500,
            vertical: true,
            step_permille: 50
        })
    );
    run(&setup.core, MODE_HORIZONTAL);
    assert_eq!(
        state(&setup).and_then(|state| state.strip),
        Some(StripState {
            first: 0,
            last_first: 0,
            count: 3
        })
    );
    run(&setup.core, MODE_CANVAS);
    let minimap = state(&setup)
        .and_then(|state| state.canvas)
        .expect("a minimap");
    assert_eq!(minimap.items.len(), 3);
    assert!(minimap.items.iter().all(|item| item.visible), "fitted");
    assert!(minimap.viewport.width > 0);
    let json = serde_json::to_value(&minimap).expect("serializes");
    assert_eq!(json["position"], json!("end"));
    run(&setup.core, MINIMAP_TOGGLE);
    assert_eq!(
        state(&setup).map(|state| state.canvas),
        Some(None),
        "switched off"
    );
}

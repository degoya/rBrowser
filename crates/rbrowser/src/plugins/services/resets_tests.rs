use rb_engine::ViewId;
use serde_json::json;

use super::{CdpResets, RESETS};

const VISION: &str = "Emulation.setEmulatedVisionDeficiency";
const SCRIPTS: &str = "Emulation.setScriptExecutionDisabled";

#[test]
fn a_reset_able_method_a_plugin_sent_is_undone_on_its_views_only() {
    let resets = CdpResets::default();
    resets.note("simulators", 1, VISION, &json!({ "type": "deuteranopia" }));
    resets.note("simulators", 2, VISION, &json!({ "type": "blurredVision" }));
    resets.note("overlays", 2, SCRIPTS, &json!({ "value": true }));
    let undo = resets.take("simulators");
    assert_eq!(
        undo,
        vec![
            (ViewId(1), VISION, json!({ "type": "none" })),
            (ViewId(2), VISION, json!({ "type": "none" })),
        ]
    );
    // Taken once: a second stop has nothing left; the other plugin's stays.
    assert!(resets.take("simulators").is_empty());
    assert_eq!(
        resets.take("overlays"),
        vec![(ViewId(2), SCRIPTS, json!({ "value": false }))]
    );
}

#[test]
fn a_plugin_that_undid_it_itself_or_sent_anything_else_leaves_nothing() {
    let resets = CdpResets::default();
    resets.note("simulators", 1, VISION, &json!({ "type": "achromatopsia" }));
    resets.note("simulators", 1, VISION, &json!({ "type": "none" }));
    resets.note("simulators", 1, "DOM.getDocument", &json!({}));
    assert!(resets.take("simulators").is_empty());
}

#[test]
fn every_reset_is_a_method_the_plugins_may_send() {
    for method in RESETS {
        let (domain, name) = method.split_once('.').expect("Domain.method");
        assert!(
            rb_plugin_host::PLUGIN_CDP_METHODS
                .iter()
                .any(|(known, methods)| *known == domain && methods.contains(&name)),
            "{method}"
        );
    }
}

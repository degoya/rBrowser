use serde_json::json;

use super::*;

#[test]
fn a_plugin_call_names_its_plugin_its_fraction_and_its_cancel_command() {
    let progress = PluginProgress {
        plugin: "width-sweep".into(),
        name: "Width sweep".into(),
        done: 12,
        total: 160,
        cancel: Some("plugin.width_sweep.cancel".into()),
    };
    let json = serde_json::to_value(&progress).expect("json");
    assert_eq!(
        json,
        json!({
            "plugin": "width-sweep",
            "name": "Width sweep",
            "done": 12,
            "total": 160,
            "cancel": "plugin.width_sweep.cancel",
        })
    );
    let back: PluginProgress = serde_json::from_value(json).expect("reads back");
    assert_eq!(back, progress);
}

#[test]
fn a_plugin_without_a_cancel_command_writes_none() {
    let progress = PluginProgress {
        plugin: "seo".into(),
        name: "SEO".into(),
        done: 0,
        total: 3,
        cancel: None,
    };
    let json = serde_json::to_value(&progress).expect("json");
    assert!(json.get("cancel").is_none());
}

#[test]
fn the_plugins_activity_is_written_plugins() {
    assert_eq!(
        serde_json::to_value(crate::Activity::Plugins).expect("json"),
        json!("plugins")
    );
    let read: crate::Activity = serde_json::from_value(json!("plugins")).expect("reads");
    assert_eq!(read, crate::Activity::Plugins);
}

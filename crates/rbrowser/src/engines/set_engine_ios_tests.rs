//! `view.set_engine` with iOS (RB-095-04, RB-095-05): the device beside the engine, and where
//! it is written.

use super::*;

/// RB-095-04, RB-095-02: iOS takes a device beside the engine; Android is no breakpoint's engine.
#[tokio::test]
async fn set_engine_writes_the_ios_device_beside_the_engine_and_only_with_ios() {
    let (dir, setup) = project();
    let core = &setup.core;
    let mobile = view_of(core, "mobile");
    let pick = json!({ "view": mobile, "engine": "ios", "iosDevice": "iPhone 16" });
    let answer = run(core, &pick).expect("runs");
    assert_eq!(answer["engine"], "ios");
    assert_eq!(answer["iosDevice"], "iPhone 16");
    let local = file(&dir, ConfigFile::Local);
    let entries = local["breakpoints"].as_array().cloned().unwrap_or_default();
    let entry = entries
        .iter()
        .find(|item| item["id"] == "mobile")
        .expect("the entry");
    assert_eq!(
        (&entry["engine"], &entry["iosDevice"]),
        (&json!("ios"), &json!("iPhone 16"))
    );
    // Another model for a view on iOS already is written too.
    let again =
        json!({ "view": mobile, "engine": "ios", "iosDevice": "iPhone SE (3rd generation)" });
    assert_eq!(run(core, &again).expect("runs")["changed"], json!(true));
    for refused in [
        json!({ "view": mobile, "engine": "firefox", "iosDevice": "iPhone 16" }),
        json!({ "view": mobile, "engine": "ios", "iosDevice": "a\nb" }),
    ] {
        assert!(
            matches!(run(core, &refused), Err(CommandError::InvalidArgs(_))),
            "{refused}"
        );
    }
    let error = run(core, &json!({ "view": mobile, "engine": "android" })).expect_err("refused");
    assert!(error.to_string().contains("view.add_android"), "{error}");
}

/// RB-095-05: a device - a UDID above all - is the person's own. With `file: "project"` the
/// engine goes into the shared `config.json`, the device into `config.local.json`.
#[tokio::test]
async fn the_ios_device_goes_into_the_local_file_when_the_engine_goes_into_the_project() {
    let (dir, setup) = project();
    let core = &setup.core;
    let mobile = view_of(core, "mobile");
    let udid = "00008110-001A2B3C4D5E801E";
    let pick = json!({ "view": mobile, "engine": "ios", "iosDevice": udid, "file": "project" });
    let answer = run(core, &pick).expect("runs");
    assert_eq!(answer["iosDevice"], udid);
    let entry_of = |file_kind: ConfigFile| {
        file(&dir, file_kind)["breakpoints"]
            .as_array()
            .and_then(|list| list.iter().find(|item| item["id"] == "mobile").cloned())
            .unwrap_or(Value::Null)
    };
    let shared = entry_of(ConfigFile::Project);
    assert_eq!(shared["engine"], "ios");
    assert!(shared.get("iosDevice").is_none(), "{shared}");
    assert_eq!(entry_of(ConfigFile::Local)["iosDevice"], udid);
    let read = core.host().config();
    let merged = read
        .breakpoints
        .iter()
        .find(|breakpoint| breakpoint.id == "mobile")
        .expect("the breakpoint");
    assert_eq!(merged.ios_device.as_deref(), Some(udid));
}

#[test]
fn the_ios_device_adds_the_entry_a_file_lacks() {
    let mut document = json!({});
    patch_ios_device(&mut document, "mobile", "iPhone 16");
    assert_eq!(
        document,
        json!({ "breakpoints": [ { "id": "mobile", "iosDevice": "iPhone 16" } ] })
    );
    patch_ios_device(&mut document, "mobile", "iPhone 17");
    assert_eq!(document["breakpoints"][0]["iosDevice"], "iPhone 17");
}

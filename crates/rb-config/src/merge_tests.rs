use super::*;
use serde_json::json;

#[test]
fn objects_merge_key_by_key() {
    let mut base = json!({ "sync": { "scroll": true, "click": true }, "startUrl": "a" });
    merge(&mut base, json!({ "sync": { "scroll": false } }));
    assert_eq!(
        base,
        json!({ "sync": { "scroll": false, "click": true }, "startUrl": "a" })
    );
}

#[test]
fn id_arrays_merge_by_id_and_keep_the_base_order() {
    let mut base = json!({ "breakpoints": [
        { "id": "mobile", "width": 393, "height": 852 },
        { "id": "desktop", "width": 1440, "height": 900 }
    ] });
    merge(
        &mut base,
        json!({ "breakpoints": [
            { "id": "desktop", "width": 1920 },
            { "id": "tablet", "width": 820, "height": 1180 }
        ] }),
    );
    assert_eq!(
        base["breakpoints"],
        json!([
            { "id": "mobile", "width": 393, "height": 852 },
            { "id": "desktop", "width": 1920, "height": 900 },
            { "id": "tablet", "width": 820, "height": 1180 }
        ])
    );
}

#[test]
fn other_arrays_and_scalars_are_replaced() {
    let mut base =
        json!({ "reload": { "watch": ["src/**", "public/**"] }, "sessions": ["default"] });
    merge(
        &mut base,
        json!({ "reload": { "watch": ["app/**"] }, "sessions": [] }),
    );
    assert_eq!(base["reload"]["watch"], json!(["app/**"]));
    assert_eq!(base["sessions"], json!([]));
}

#[test]
fn origins_name_project_and_local_values_by_dotted_key() {
    let project = json!({
        "version": 1,
        "sync": { "scroll": true },
        "breakpoints": [{ "id": "mobile", "width": 393 }],
        "sessions": ["default"]
    });
    let local = json!({ "breakpoints": [{ "id": "mobile", "width": 400 }] });
    let origins = origins(&project, Some(&local));
    assert_eq!(origins["version"], Origin::Project);
    assert_eq!(origins["sync.scroll"], Origin::Project);
    assert_eq!(origins["breakpoints.mobile.width"], Origin::Local);
    assert_eq!(origins["breakpoints.mobile.id"], Origin::Local);
    assert_eq!(origins["sessions"], Origin::Project);
    assert!(!origins.contains_key("sync.click"), "defaults are absent");
}

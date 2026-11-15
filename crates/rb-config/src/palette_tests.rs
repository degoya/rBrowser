use serde_json::json;

use crate::UserSettings;

#[test]
fn the_palette_defaults_and_reads_its_section() {
    let user: UserSettings = serde_json::from_value(json!({})).expect("settings");
    assert_eq!(user.palette.recent_count, 8);
    assert!(user.palette.show_descriptions && user.palette.prefixes);
    let user: UserSettings =
        serde_json::from_value(json!({ "palette": { "recentCount": 0, "prefixes": false } }))
            .expect("settings");
    assert_eq!(user.palette.recent_count, 0);
    assert!(!user.palette.prefixes);
    assert!(user.palette.show_descriptions);
    assert!(!user.extra.contains_key("palette"));
}

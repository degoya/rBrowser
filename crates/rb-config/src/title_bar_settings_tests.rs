use serde_json::json;

use super::*;
use crate::UserSettings;

#[test]
fn the_title_bar_shows_version_and_credit_unless_told_not_to() {
    let user: UserSettings = serde_json::from_value(json!({})).expect("settings");
    assert_eq!(
        user.ui.title_bar,
        TitleBarSettings {
            show_version: true,
            show_credit: true,
        }
    );
    let user: UserSettings =
        serde_json::from_value(json!({ "ui": { "titleBar": { "showCredit": false } } }))
            .expect("settings");
    assert!(
        user.ui.title_bar.show_version,
        "an unset key keeps its default"
    );
    assert!(!user.ui.title_bar.show_credit);
    assert_eq!(
        serde_json::to_value(user.ui.title_bar).expect("json"),
        json!({ "showVersion": true, "showCredit": false })
    );
}

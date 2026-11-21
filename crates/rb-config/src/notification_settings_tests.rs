use serde_json::json;

use super::*;
use crate::UserSettings;

// RB-094-20: the error of an action the person started shows at the top too, unless switched off.
#[test]
fn the_error_toast_is_on_by_default_and_switchable() {
    let user: UserSettings = serde_json::from_value(json!({})).expect("settings");
    assert_eq!(user.ui.notifications, NotificationSettings::default());
    assert!(user.ui.notifications.error_toast);
    let off: UserSettings = serde_json::from_value(json!({
        "ui": { "notifications": { "errorToast": false } }
    }))
    .expect("settings");
    assert!(!off.ui.notifications.error_toast);
    assert_eq!(
        serde_json::to_value(NotificationSettings::default()).expect("json"),
        json!({ "errorToast": true })
    );
}

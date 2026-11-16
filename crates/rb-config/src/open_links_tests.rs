use super::OpenLinksIn;
use crate::UserSettings;

#[test]
fn links_open_in_the_system_browser_unless_the_user_says_otherwise() {
    assert_eq!(
        UserSettings::default().ui.open_links_in,
        OpenLinksIn::System
    );
    let settings: UserSettings =
        serde_json::from_str(r#"{ "version": 1, "ui": { "openLinksIn": "activeView" } }"#)
            .expect("parses");
    assert_eq!(settings.ui.open_links_in, OpenLinksIn::ActiveView);
    let ask: UserSettings =
        serde_json::from_str(r#"{ "version": 1, "ui": { "openLinksIn": "ask" } }"#)
            .expect("parses");
    assert_eq!(ask.ui.open_links_in, OpenLinksIn::Ask);
}

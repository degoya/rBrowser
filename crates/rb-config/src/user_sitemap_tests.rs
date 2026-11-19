use crate::{UserSettings, annotate};

use super::*;

#[test]
fn the_gallery_opens_in_the_apps_window_by_default() {
    assert_eq!(
        UserSettings::default().sitemap.gallery_opens_in,
        GalleryOpensIn::Window
    );
    let settings: UserSettings =
        serde_json::from_str(r#"{ "version": 1, "sitemap": { "galleryOpensIn": "system" } }"#)
            .expect("parses");
    assert_eq!(settings.sitemap.gallery_opens_in, GalleryOpensIn::System);
    let bad = r#"{ "version": 1, "sitemap": { "galleryOpensIn": "grid" } }"#;
    assert!(serde_json::from_str::<UserSettings>(bad).is_err());
}

#[test]
fn where_the_gallery_opens_is_a_protected_user_key() {
    let key = annotate::USER_KEYS
        .iter()
        .find(|key| key.key == "sitemap.galleryOpensIn")
        .expect("a user key");
    assert!(key.protected, "MCP may not set it");
    assert!(
        !annotate::PROJECT_KEYS
            .iter()
            .any(|key| key.key == "sitemap.galleryOpensIn"),
        "no project key"
    );
}

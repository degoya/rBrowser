use super::*;
use serde_json::json;

#[test]
fn the_defaults_are_the_job_files() {
    let settings: PageZoomSettings = serde_json::from_value(json!({})).expect("defaults");
    assert_eq!(settings, PageZoomSettings::default());
    assert!(
        settings.keys,
        "Ctrl+0, Ctrl+Plus and Ctrl+Minus zoom the page"
    );
    assert!(settings.wheel, "Ctrl+wheel zooms the page, as in Chromium");
    assert!(!settings.sync, "a zoom is the view's own");
    assert!(settings.keep_on_reload, "Chromium keeps the zoom");
}

#[test]
fn the_keys_are_camel_case() {
    let settings: PageZoomSettings = serde_json::from_value(json!({
        "keys": false, "wheel": false, "sync": true, "keepOnReload": false
    }))
    .expect("reads");
    assert!(!settings.keys);
    assert!(!settings.wheel);
    assert!(settings.sync);
    assert!(!settings.keep_on_reload);
}

#[test]
fn the_user_settings_carry_it_under_views_page_zoom() {
    let user: crate::UserSettings =
        serde_json::from_value(json!({ "views": { "pageZoom": { "sync": true } } }))
            .expect("reads");
    assert!(user.views.page_zoom.sync);
    assert!(user.views.page_zoom.wheel);
    let empty: crate::UserSettings = serde_json::from_value(json!({})).expect("reads");
    assert_eq!(empty.views, ViewsSettings::default());
}

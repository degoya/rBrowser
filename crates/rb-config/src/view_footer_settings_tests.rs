use serde_json::json;

use super::*;
use crate::UserSettings;

#[test]
fn the_footer_is_on_with_range_and_class_by_default() {
    // RB-080-78, the owner's decision: the name and the size are in the label strip above.
    let user: UserSettings = serde_json::from_value(json!({})).expect("settings");
    let footer = &user.ui.view_footer;
    assert!(footer.visible);
    assert!(footer.shown());
    assert_eq!(
        footer.fields.iter().copied().collect::<Vec<_>>(),
        [FooterField::Range, FooterField::Framework]
    );
}

#[test]
fn the_old_default_fields_are_lifted_to_the_new_default() {
    // A file that holds exactly the default before RB-080-78, in any order.
    for old in [
        json!(["breakpoint", "range", "size", "queries"]),
        json!(["queries", "size", "range", "breakpoint", "size"]),
    ] {
        let user: UserSettings =
            serde_json::from_value(json!({ "ui": { "viewFooter": { "fields": old } } }))
                .expect("settings");
        assert_eq!(
            user.ui.view_footer.fields,
            ViewFooterSettings::default().fields,
            "{old}"
        );
    }
}

#[test]
fn a_choice_of_its_own_stays() {
    for own in [
        json!(["breakpoint", "range"]),
        json!(["breakpoint", "range", "size", "queries", "framework"]),
        json!(["size"]),
    ] {
        let user: UserSettings =
            serde_json::from_value(json!({ "ui": { "viewFooter": { "fields": own } } }))
                .expect("settings");
        let names = serde_json::to_value(&user.ui.view_footer.fields).expect("names");
        let mut wanted: Vec<String> = serde_json::from_value(own.clone()).expect("names");
        wanted.dedup();
        assert_eq!(names.as_array().map(Vec::len), Some(wanted.len()), "{own}");
    }
}

#[test]
fn the_fields_keep_the_strip_order_whatever_the_file_says() {
    let user: UserSettings = serde_json::from_value(json!({
        "ui": { "viewFooter": { "fields": ["framework", "size", "breakpoint", "size"] } }
    }))
    .expect("settings");
    let footer = &user.ui.view_footer;
    assert!(footer.visible, "an absent switch stays on");
    assert_eq!(
        footer.fields.iter().copied().collect::<Vec<_>>(),
        [
            FooterField::Breakpoint,
            FooterField::Size,
            FooterField::Framework
        ]
    );
}

#[test]
fn a_footer_switched_off_or_without_fields_is_not_shown() {
    let off: UserSettings =
        serde_json::from_value(json!({ "ui": { "viewFooter": { "visible": false } } }))
            .expect("settings");
    assert!(!off.ui.view_footer.shown());
    let empty: UserSettings =
        serde_json::from_value(json!({ "ui": { "viewFooter": { "fields": [] } } }))
            .expect("settings");
    assert!(empty.ui.view_footer.visible);
    assert!(!empty.ui.view_footer.shown());
}

#[test]
fn an_unknown_field_is_refused() {
    let bad = serde_json::from_value::<ViewFooterSettings>(json!({ "fields": ["colour"] }));
    assert!(bad.is_err());
}

#[test]
fn the_schema_lists_the_fields_as_a_set_of_names() {
    let schema = serde_json::to_value(schemars::schema_for!(ViewFooterSettings)).expect("schema");
    let text = schema.to_string();
    for name in ["breakpoint", "range", "size", "queries", "framework"] {
        assert!(text.contains(&format!("\"{name}\"")), "{name} in {text}");
    }
    assert_eq!(schema["properties"]["fields"]["uniqueItems"], json!(true));
}

#[test]
fn the_footer_analyses_a_loaded_page_by_itself_unless_switched_off() {
    let user: UserSettings = serde_json::from_value(json!({})).expect("settings");
    assert!(user.ui.view_footer.analyze);
    let off: UserSettings =
        serde_json::from_value(json!({ "ui": { "viewFooter": { "analyze": false } } }))
            .expect("settings");
    assert!(!off.ui.view_footer.analyze);
    assert!(off.ui.view_footer.shown(), "the strip itself stays");
}

#[test]
fn the_footer_wraps_and_is_centred_by_default() {
    let user: UserSettings = serde_json::from_value(json!({})).expect("settings");
    assert_eq!(user.ui.view_footer.wrap, FooterWrap::Auto);
    assert_eq!(user.ui.view_footer.align, FooterAlign::Center);
}

#[test]
fn wrap_and_align_are_read_from_the_file() {
    let user: UserSettings = serde_json::from_value(json!({
        "ui": { "viewFooter": { "wrap": "never", "align": "start" } }
    }))
    .expect("settings");
    assert_eq!(user.ui.view_footer.wrap, FooterWrap::Never);
    assert_eq!(user.ui.view_footer.align, FooterAlign::Start);
    assert!(
        serde_json::from_value::<UserSettings>(json!({
            "ui": { "viewFooter": { "wrap": "always" } }
        }))
        .is_err()
    );
}

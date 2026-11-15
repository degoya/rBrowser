use rb_plugin_api::test_support::MemoryHost;

use super::*;

#[test]
fn without_settings_the_schema_defaults_apply() {
    assert_eq!(read(&mut MemoryHost::new()), Settings::default());
    assert_eq!(Settings::default().format, Format::Png);
}

#[test]
fn the_settings_are_read_and_bad_values_ignored() {
    let mut host = MemoryHost::new();
    host.settings
        .insert("format".to_owned(), "\"jpeg\"".to_owned());
    host.settings
        .insert("jpegQuality".to_owned(), "60".to_owned());
    assert_eq!(
        read(&mut host),
        Settings {
            format: Format::Jpeg,
            jpeg_quality: 60
        }
    );
    host.settings
        .insert("format".to_owned(), "\"gif\"".to_owned());
    host.settings
        .insert("jpegQuality".to_owned(), "500".to_owned());
    assert_eq!(read(&mut host), Settings::default());
}

#[test]
fn the_schema_holds_the_defaults_the_code_falls_back_to() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../settings.schema.json")).expect("schema");
    let properties = &schema["properties"];
    assert_eq!(properties["format"]["default"], "png");
    assert_eq!(
        properties["jpegQuality"]["default"],
        u64::from(DEFAULT_QUALITY)
    );
    assert_eq!(
        properties["redactBlock"]["default"],
        crate::model::DEFAULT_BLOCK
    );
    for (key, property) in properties.as_object().expect("properties") {
        assert_eq!(property["x-category"], "capture", "{key}");
        assert_eq!(property["x-section"], "screenshot-editor", "{key}");
        assert!(property["x-order"].is_u64(), "{key}");
    }
}

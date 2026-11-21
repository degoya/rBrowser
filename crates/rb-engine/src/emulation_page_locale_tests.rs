//! `dir` and `lang` of a view's emulation (RB-092-02).

use rb_config::TextDirection;
use serde_json::{Value, json};

use super::*;

fn resolve(own: Value, defaults: Value) -> Result<Emulation, EngineError> {
    let mut own = own;
    if let Some(object) = own.as_object_mut() {
        object.insert("id".into(), json!("b"));
        object.entry("width").or_insert(json!(800));
        object.entry("height").or_insert(json!(600));
    }
    let breakpoint: Breakpoint = serde_json::from_value(own).expect("a breakpoint");
    let defaults: EmulationFields = serde_json::from_value(defaults).expect("defaults");
    resolve_emulation(&breakpoint, &defaults, &[])
}

#[test]
fn the_breakpoint_s_dir_and_lang_win_over_the_defaults() {
    let emulation = resolve(
        json!({ "dir": "rtl", "lang": "ar" }),
        json!({ "dir": "ltr", "lang": "de" }),
    )
    .expect("resolves");
    assert_eq!(emulation.dir, Some(TextDirection::Rtl));
    assert_eq!(emulation.lang.as_deref(), Some("ar"));
    let defaults = resolve(json!({}), json!({ "dir": "auto", "lang": "he" })).expect("resolves");
    assert_eq!(defaults.dir, Some(TextDirection::Auto));
    assert_eq!(defaults.lang.as_deref(), Some("he"));
}

#[test]
fn without_dir_and_lang_the_page_keeps_its_own() {
    let emulation = resolve(json!({}), json!({})).expect("resolves");
    assert_eq!(emulation.dir, None);
    assert_eq!(emulation.lang, None);
    assert_eq!(Emulation::default().dir, None);
    let empty = resolve(json!({ "lang": "" }), json!({ "lang": "de" })).expect("resolves");
    assert_eq!(empty.lang, None, "an empty lang keeps the page's");
}

#[test]
fn a_lang_that_is_no_bcp_47_tag_is_refused() {
    for lang in ["ar\"><script>", "de DE", "-de", "x\u{7}"] {
        let error = resolve(json!({ "lang": lang }), json!({})).expect_err(lang);
        assert!(error.to_string().contains("lang"), "{error}");
    }
    assert!(resolve(json!({ "lang": "zh-Hant-TW" }), json!({})).is_ok());
}

#[test]
fn dir_and_lang_serialize_camel_case_and_only_when_set() {
    let emulation = Emulation {
        dir: Some(TextDirection::Rtl),
        lang: Some("ar".into()),
        ..Emulation::default()
    };
    let value = serde_json::to_value(&emulation).expect("serializes");
    assert_eq!(value["dir"], "rtl");
    assert_eq!(value["lang"], "ar");
    let plain = serde_json::to_value(Emulation::default()).expect("serializes");
    assert!(plain.get("dir").is_none() && plain.get("lang").is_none());
}

#[test]
fn dir_and_lang_are_chromium_s_only_firefox_and_webkit_mark_them() {
    use crate::emulation_extras::{DIR, LANG, not_emulable};
    use rb_config::EngineKind;
    let emulation = Emulation {
        dir: Some(TextDirection::Rtl),
        lang: Some("ar".into()),
        ..Emulation::default()
    };
    assert!(not_emulable(EngineKind::Chromium, &emulation, &[]).is_empty());
    assert_eq!(
        not_emulable(EngineKind::Firefox, &emulation, &[]),
        [DIR, LANG]
    );
    assert_eq!(
        not_emulable(EngineKind::Webkit, &emulation, &[]),
        [DIR, LANG]
    );
}

use serde_json::json;

use super::*;
use crate::{EmulationFields, EmulationUserSettings};

#[test]
fn the_direction_reads_and_writes_lowercase() {
    for (text, direction) in [
        ("ltr", TextDirection::Ltr),
        ("rtl", TextDirection::Rtl),
        ("auto", TextDirection::Auto),
    ] {
        let read: TextDirection = serde_json::from_value(json!(text)).expect("reads");
        assert_eq!(read, direction);
        assert_eq!(
            serde_json::to_value(direction).expect("writes"),
            json!(text)
        );
    }
    assert!(serde_json::from_value::<TextDirection>(json!("RTL")).is_err());
}

#[test]
fn a_toggle_goes_to_rtl_and_back_to_ltr() {
    assert_eq!(TextDirection::toggled(None), TextDirection::Rtl);
    assert_eq!(
        TextDirection::toggled(Some(TextDirection::Ltr)),
        TextDirection::Rtl
    );
    assert_eq!(
        TextDirection::toggled(Some(TextDirection::Auto)),
        TextDirection::Rtl
    );
    assert_eq!(
        TextDirection::toggled(Some(TextDirection::Rtl)),
        TextDirection::Ltr
    );
}

#[test]
fn a_breakpoint_carries_dir_and_lang_among_its_emulation_fields() {
    let fields: EmulationFields =
        serde_json::from_value(json!({ "dir": "rtl", "lang": "ar" })).expect("reads");
    assert_eq!(fields.dir, Some(TextDirection::Rtl));
    assert_eq!(fields.lang.as_deref(), Some("ar"));
    let back = serde_json::to_value(&fields).expect("writes");
    assert_eq!(back, json!({ "dir": "rtl", "lang": "ar" }));
    let none = serde_json::to_value(EmulationFields::default()).expect("writes");
    assert_eq!(none, json!({}), "absent fields stay out of the file");
}

#[test]
fn the_pseudo_locale_defaults_are_the_job_file_s() {
    let settings: PseudoLocaleSettings = serde_json::from_value(json!({})).expect("defaults");
    assert_eq!(settings, PseudoLocaleSettings::default());
    assert!((settings.factor - 1.4).abs() < f64::EPSILON);
    assert!(settings.accents);
    assert!(!settings.rtl);
}

#[test]
fn the_user_settings_carry_it_under_emulation_pseudo_locale() {
    let user: EmulationUserSettings = serde_json::from_value(json!({
        "pseudoLocale": { "factor": 2, "accents": false, "rtl": true }
    }))
    .expect("reads");
    assert!((user.pseudo_locale.factor - 2.0).abs() < f64::EPSILON);
    assert!(!user.pseudo_locale.accents);
    assert!(user.pseudo_locale.rtl);
    let empty: EmulationUserSettings = serde_json::from_value(json!({})).expect("reads");
    assert_eq!(empty.pseudo_locale, PseudoLocaleSettings::default());
}

#[test]
fn a_factor_is_held_to_its_range() {
    assert!((clamp_pseudo_factor(0.5) - 1.0).abs() < f64::EPSILON);
    assert!((clamp_pseudo_factor(9.0) - 3.0).abs() < f64::EPSILON);
    assert!((clamp_pseudo_factor(1.7) - 1.7).abs() < f64::EPSILON);
    assert!((clamp_pseudo_factor(f64::NAN) - DEFAULT_PSEUDO_FACTOR).abs() < f64::EPSILON);
}

use std::collections::BTreeMap;

use serde::Deserialize;

use super::*;

#[test]
fn keys_follow_area_dot_thing() {
    for good in [
        "toolbar.reload",
        "config.error.duplicate_id",
        "settings.key.sync.leaderOnlySubmit.label",
        "plugin.seo-essentials.title",
    ] {
        assert!(is_well_formed_key(good), "{good}");
    }
    for bad in [
        "",
        "toolbar",
        "Toolbar.reload",
        "toolbar..reload",
        "toolbar.re load",
    ] {
        assert!(!is_well_formed_key(bad), "{bad}");
    }
}

/// A key segment the shell's catalogue would walk into an object's prototype is no key: the
/// window nests a plugin's keys into objects (RB-080-82, security review c20).
#[test]
fn a_key_never_names_a_prototype_segment() {
    for bad in [
        "plugin.demo.settings.constructor.prototype.x",
        "plugin.demo.constructor",
        "plugin.demo.prototype.label",
        "constructor.thing",
        "prototype.thing",
    ] {
        assert!(!is_well_formed_key(bad), "{bad}");
    }
    let locale =
        br#"{"plugin":{"demo-proto":{"settings":{"constructor":{"prototype":{"x":"polluted"}}}}}}"#;
    assert!(register_plugin_locale("demo-proto", "en", locale).is_err());
    // A segment that only contains the word stays a key.
    assert!(is_well_formed_key("plugin.demo.commands.constructors"));
}

#[test]
fn the_paths_are_fixed() {
    assert_eq!(CATALOGUE_PATH, "locales/en.json");
    assert_eq!(PARTS_DIR, "locales/parts");
    assert_eq!(part_path("RB-010-05"), "locales/parts/RB-010-05.json");
}

#[test]
fn the_compiled_catalogue_is_valid() {
    let parsed = Catalogue::from_json(CATALOGUE_JSON);
    assert!(parsed.is_ok(), "locales/en.json: {parsed:?}");
    assert_eq!(core_catalogue().len(), parsed.map(|c| c.len()).unwrap_or(0));
}

#[test]
fn a_missing_key_shows_itself() {
    assert_eq!(
        t("toolbar.no_such_key_anywhere"),
        "toolbar.no_such_key_anywhere"
    );
    assert_eq!(
        t_with("grid.no_such_key_anywhere", &[("count", "3")]),
        "grid.no_such_key_anywhere"
    );
    assert_eq!(
        t!("grid.no_such_key_anywhere", count = 3),
        "grid.no_such_key_anywhere"
    );
}

#[derive(Deserialize)]
struct Cases {
    cases: Vec<Case>,
    invalid: Vec<String>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    locale: String,
    message: String,
    args: BTreeMap<String, String>,
    expected: String,
}

/// The vectors the shell's formatter is held to as well (`locales/tests/format-cases.json`).
#[test]
fn the_shared_format_cases_hold() {
    let cases: Cases =
        serde_json::from_str(include_str!("../../../locales/tests/format-cases.json"))
            .expect("the format cases parse");
    assert!(!cases.cases.is_empty());
    for case in &cases.cases {
        let style = match case.locale.as_str() {
            PSEUDO_LOCALE => message::Style::Pseudo,
            _ => message::Style::Plain,
        };
        let args: Vec<(&str, &str)> = case
            .args
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        assert_eq!(
            message::format(&case.message, &args, style),
            case.expected,
            "{}",
            case.name
        );
    }
    for bad in &cases.invalid {
        assert!(validate_message(bad).is_err(), "{bad:?} should be refused");
    }
}

/// Plugin locales and the language switch share the global state; one test holds them all so a
/// threaded test run cannot interleave them.
#[test]
fn plugin_texts_are_found_and_the_pseudo_locale_applies() {
    let json = br#"{"plugin": {"demo-lookup": {"panel": {"title": "Demo", "views": "{count, plural, one {# view} other {# views}}"}}}}"#;
    assert_eq!(register_plugin_locale("demo-lookup", "en", json), Ok(2));
    assert_eq!(t("plugin.demo-lookup.panel.title"), "Demo");
    assert_eq!(t!("plugin.demo-lookup.panel.views", count = 1), "1 view");
    assert_eq!(t!("plugin.demo-lookup.panel.views", count = 2), "2 views");
    assert_eq!(
        plugin_texts("demo-lookup").get("plugin.demo-lookup.panel.title"),
        Some(&"Demo".to_owned())
    );
    assert_eq!(
        plugin_text("demo-lookup", "plugin.demo-lookup.panel.title").as_deref(),
        Some("Demo")
    );
    assert_eq!(
        plugin_text("demo-lookup", "plugin.demo-lookup.panel.nope"),
        None
    );
    assert_eq!(plugin_text("other", "plugin.demo-lookup.panel.title"), None);
    assert_eq!(
        t("plugin.demo-lookup.panel.nope"),
        "plugin.demo-lookup.panel.nope"
    );

    assert_eq!(locale(), DEFAULT_LOCALE);
    assert_eq!(available_locales(), ["en"]);
    set_locale(PSEUDO_LOCALE).expect("the pseudo-locale is known");
    assert_eq!(locale(), PSEUDO_LOCALE);
    assert_eq!(t("plugin.demo-lookup.panel.title"), "[D\u{e9}m\u{f3}]");
    assert_eq!(
        t("plugin.demo-lookup.panel.nope"),
        "plugin.demo-lookup.panel.nope",
        "a missing key shows itself, unpadded"
    );
    assert_eq!(
        set_locale("de"),
        Err(I18nError::UnknownLocale("de".to_owned()))
    );
    assert_eq!(locale(), PSEUDO_LOCALE, "a refused switch changes nothing");
    set_locale(DEFAULT_LOCALE).expect("en is known");
    assert_eq!(t("plugin.demo-lookup.panel.title"), "Demo");

    unregister_plugin_locales("demo-lookup");
    assert_eq!(
        t("plugin.demo-lookup.panel.title"),
        "plugin.demo-lookup.panel.title"
    );
    assert!(plugin_texts("demo-lookup").is_empty());
}

#[test]
fn a_plugin_locale_is_checked_before_it_is_kept() {
    assert_eq!(
        register_plugin_locale("demo-refused", "english", b"{}"),
        Err(I18nError::UnknownLocale("english".to_owned()))
    );
    assert!(matches!(
        register_plugin_locale("demo-refused", "en", br#"{"toolbar": {"reload": "X"}}"#),
        Err(I18nError::ForeignKey { .. })
    ));
    assert!(plugin_texts("demo-refused").is_empty());
}

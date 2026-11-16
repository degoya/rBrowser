use super::*;

fn valid(text: &str) -> String {
    match check(text) {
        Checked::Valid(code) => code.to_string(),
        other => panic!("{text}: {other:?}"),
    }
}

fn suggestion(text: &str) -> Option<String> {
    match check(text) {
        Checked::Invalid { suggestion } => suggestion,
        other => panic!("{text} is not invalid: {other:?}"),
    }
}

#[test]
fn language_region_and_script_codes_are_valid_in_any_case() {
    assert_eq!(valid("en"), "en");
    assert_eq!(valid("de-DE"), "de-DE");
    assert_eq!(valid("DE-de"), "de-DE");
    assert_eq!(valid("en-gb"), "en-GB");
    assert_eq!(valid("zh-Hant"), "zh-Hant");
    assert_eq!(valid("zh-hant-tw"), "zh-Hant-TW");
    assert_eq!(valid(" fr "), "fr");
    assert_eq!(check("x-default"), Checked::XDefault);
    assert_eq!(check("X-Default"), Checked::XDefault);
}

#[test]
fn the_typical_mistakes_get_their_correction() {
    assert_eq!(suggestion("en-UK").as_deref(), Some("en-GB"));
    assert_eq!(suggestion("en-uk").as_deref(), Some("en-GB"));
    assert_eq!(suggestion("de_DE").as_deref(), Some("de-DE"));
    assert_eq!(suggestion("en_UK").as_deref(), Some("en-GB"));
    assert_eq!(suggestion("jp").as_deref(), Some("ja"));
    assert_eq!(suggestion("cz-CZ").as_deref(), Some("cs-CZ"));
    assert_eq!(suggestion("x_default").as_deref(), Some("x-default"));
}

#[test]
fn unknown_codes_are_invalid_without_a_guess() {
    assert_eq!(suggestion("xx"), None);
    assert_eq!(suggestion("en-XX"), None);
    assert_eq!(suggestion("es-419"), None);
    assert_eq!(suggestion("english"), None);
    assert_eq!(suggestion(""), None);
    assert_eq!(suggestion("en-GB-extra"), None);
    assert_eq!(suggestion("GB"), None);
}

#[test]
fn every_table_entry_is_well_formed_and_sorted() {
    for pair in LANGUAGES.windows(2) {
        assert!(pair[0] < pair[1], "{pair:?}");
    }
    for pair in REGIONS.windows(2) {
        assert!(pair[0] < pair[1], "{pair:?}");
    }
    assert!(
        LANGUAGES
            .iter()
            .all(|code| code.len() == 2 && code.chars().all(|c| c.is_ascii_lowercase()))
    );
    assert!(
        REGIONS
            .iter()
            .all(|code| code.len() == 2 && code.chars().all(|c| c.is_ascii_uppercase()))
    );
    assert_eq!(REGIONS.len(), 249);
}

#[test]
fn languages_match_by_primary_tag_and_region_when_both_name_one() {
    let code = |text| match check(text) {
        Checked::Valid(code) => code,
        other => panic!("{other:?}"),
    };
    assert!(code("de").matches_lang("de-AT"));
    assert!(code("de-AT").matches_lang("de"));
    assert!(code("de-AT").matches_lang("DE_at"));
    assert!(!code("de-AT").matches_lang("de-CH"));
    assert!(!code("fr").matches_lang("en"));
}

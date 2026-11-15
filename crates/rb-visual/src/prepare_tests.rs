use std::collections::BTreeSet;

use serde_json::json;

use super::*;

fn list() -> BuiltinList {
    serde_json::from_str(BUILTIN_JSON).expect("the shipped list parses")
}

#[test]
fn the_builtin_list_is_sound() {
    let list = list();
    assert_eq!(list.version, 1);
    let sources: BTreeSet<&str> = list.sources.iter().map(|s| s.id.as_str()).collect();
    for source in &list.sources {
        assert!(
            ALLOWED_LICENCES.contains(&source.licence.as_str()),
            "{} has the licence {}",
            source.id,
            source.licence
        );
    }
    let mut groups = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for group in &list.groups {
        assert!(groups.insert(group.id.as_str()), "group {} twice", group.id);
        assert!(sources.contains(group.source.as_str()), "{}", group.id);
        assert!(
            ["consent", "chat"].contains(&group.kind.as_str()),
            "{}",
            group.id
        );
        assert!(!group.selectors.is_empty(), "{} is empty", group.id);
        for selector in &group.selectors {
            assert_eq!(selector.trim(), selector, "{selector:?} is padded");
            assert!(!selector.is_empty() && selector.len() <= MAX_SELECTOR_LEN);
            assert!(
                !selector.contains(['{', '}', ';']),
                "{selector:?} is more than a selector"
            );
            assert!(seen.insert(selector.as_str()), "{selector} twice");
        }
    }
    assert_eq!(BUILTIN.len(), seen.len());
    assert!(BUILTIN.len() < MAX_SELECTORS - MAX_PROJECT_SELECTORS);
}

#[test]
fn the_builtin_list_covers_the_named_managers_and_the_old_sitemap_defaults() {
    let ids: BTreeSet<String> = list().groups.into_iter().map(|group| group.id).collect();
    for id in [
        "onetrust",
        "cookiebot",
        "usercentrics",
        "didomi",
        "borlabs",
        "klaro",
        "cookieyes",
        "complianz",
        "intercom",
    ] {
        assert!(ids.contains(id), "{id} missing");
    }
    // `sitemap.hideSelectors`' defaults before RB-080-12.
    for old in [
        "#onetrust-banner-sdk",
        "#onetrust-consent-sdk",
        "#CybotCookiebotDialog",
        "#usercentrics-root",
        "#cookie-law-info-bar",
        ".cc-window",
        ".cookie-banner",
        "[aria-label='cookieconsent']",
    ] {
        assert!(BUILTIN.iter().any(|selector| selector == old), "{old}");
    }
}

#[test]
fn the_defaults_hide_the_builtin_list_and_freeze() {
    let plan = CleanSettings::default().plan(&[], CleanOverrides::default());
    assert!(plan.hide && plan.freeze);
    assert_eq!(plan.selectors, *BUILTIN);
    assert_eq!(plan.settle(), Duration::from_millis(100));
}

#[test]
fn the_project_list_adds_to_the_builtin_one_without_duplicates() {
    let settings = CleanSettings {
        hide_selectors: vec![
            " #own-promo ".to_owned(),
            "#onetrust-banner-sdk".to_owned(),
            String::new(),
            "#own-promo".to_owned(),
        ],
        ..CleanSettings::default()
    };
    let extra = vec![".old-sitemap-banner".to_owned()];
    let plan = settings.plan(&extra, CleanOverrides::default());
    assert_eq!(plan.selectors.len(), BUILTIN.len() + 2);
    assert_eq!(plan.selectors[BUILTIN.len()], "#own-promo");
    assert_eq!(plan.selectors[BUILTIN.len() + 1], ".old-sitemap-banner");
}

#[test]
fn without_the_builtin_list_only_the_projects_count() {
    let settings = CleanSettings {
        use_builtin_list: false,
        hide_selectors: vec!["#own".to_owned()],
        ..CleanSettings::default()
    };
    let plan = settings.plan(&[], CleanOverrides::default());
    assert_eq!(plan.selectors, ["#own"]);
    let none = CleanSettings {
        use_builtin_list: false,
        ..CleanSettings::default()
    };
    assert!(!none.plan(&[], CleanOverrides::default()).hide);
}

#[test]
fn the_call_overrides_the_settings_both_ways() {
    let off = CleanOverrides {
        hide_banners: Some(false),
        freeze_animations: Some(false),
    };
    let plan = CleanSettings::default().plan(&[], off);
    assert!(plan.is_noop());
    assert!(plan.selectors.is_empty());
    assert_eq!(plan.settle(), Duration::ZERO);
    let settings = CleanSettings {
        hide_banners: false,
        freeze_animations: false,
        ..CleanSettings::default()
    };
    let on = CleanOverrides {
        hide_banners: Some(true),
        freeze_animations: None,
    };
    let plan = settings.plan(&[], on);
    assert!(plan.hide && !plan.freeze);
}

#[test]
fn overrides_fall_back_per_field() {
    let call = CleanOverrides {
        hide_banners: Some(false),
        freeze_animations: None,
    };
    let session = CleanOverrides {
        hide_banners: Some(true),
        freeze_animations: Some(false),
    };
    assert_eq!(
        call.or(session),
        CleanOverrides {
            hide_banners: Some(false),
            freeze_animations: Some(false),
        }
    );
}

#[test]
fn the_selectors_are_bounded() {
    let settings = CleanSettings {
        hide_selectors: (0..400).map(|n| format!("#s{n}")).collect(),
        settle_ms: u32::MAX,
        ..CleanSettings::default()
    };
    let plan = settings.plan(
        &["x".repeat(MAX_SELECTOR_LEN + 1)],
        CleanOverrides::default(),
    );
    assert_eq!(plan.selectors.len(), MAX_SELECTORS);
    assert_eq!(plan.settle_ms, MAX_SETTLE_MS);
}

#[test]
fn parse_selectors_takes_a_list_of_strings() {
    assert_eq!(
        parse_selectors(&json!([" #a ", ".b"])).expect("a list"),
        ["#a", ".b"]
    );
    assert!(parse_selectors(&json!("#a")).is_err());
    assert!(parse_selectors(&json!([1])).is_err());
    assert!(parse_selectors(&json!([""])).is_err());
    let many: Vec<String> = (0..=MAX_PROJECT_SELECTORS)
        .map(|n| format!("#s{n}"))
        .collect();
    assert!(parse_selectors(&json!(many)).is_err());
}

#[test]
fn the_selectors_go_into_the_script_as_data() {
    let plan = CleanPlan {
        hide: true,
        selectors: vec!["a\"); alert(1); (\"".to_owned()],
        freeze: false,
        settle_ms: 0,
    };
    let expression = plan.prepare_expression();
    assert!(expression.starts_with("// The capture preparation"));
    let (_, call) = expression
        .rsplit_once("\nglobalThis.__rbCaptureClean.prepare(")
        .expect("the call last");
    let options: serde_json::Value =
        serde_json::from_str(call.strip_suffix(')').expect("closed")).expect("JSON data");
    assert_eq!(
        options,
        json!({ "hide": true, "selectors": ["a\"); alert(1); (\""], "freeze": false })
    );
    assert!(undo_expression(7).ends_with("globalThis.__rbCaptureClean.undo(7)"));
}

#[test]
fn reduced_motion_joins_the_views_media() {
    let media = json!({
        "media": "print",
        "features": [
            { "name": "prefers-color-scheme", "value": "dark" },
            { "name": "prefers-reduced-motion", "value": "no-preference" }
        ]
    });
    assert_eq!(
        reduced_motion_media(&media),
        json!({
            "media": "print",
            "features": [
                { "name": "prefers-color-scheme", "value": "dark" },
                { "name": "prefers-reduced-motion", "value": "reduce" }
            ]
        })
    );
    assert_eq!(
        reduced_motion_media(&json!({})),
        json!({ "media": "", "features": [{ "name": "prefers-reduced-motion", "value": "reduce" }] })
    );
}

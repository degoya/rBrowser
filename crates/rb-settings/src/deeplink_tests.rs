use super::*;
use crate::test_support::def;

fn registry() -> SettingsRegistry {
    let mut registry = SettingsRegistry::new();
    registry
        .register(def("sync.scroll", Category::Sync, "scroll", 1))
        .expect("registers");
    registry
        .register(def("breakpoints", Category::Breakpoints, "breakpoints", 1))
        .expect("registers");
    registry
}

#[test]
fn a_key_link_resolves_to_category_section_and_key() {
    assert_eq!(
        registry().resolve_deep_link("rbrowser://settings/sync.scroll"),
        Ok(DeepLinkTarget::Key {
            category: Category::Sync,
            section: "scroll".into(),
            key: "sync.scroll".into(),
        })
    );
}

#[test]
fn the_link_is_built_from_the_key() {
    let link = deep_link("sync.scroll");
    assert_eq!(link, "rbrowser://settings/sync.scroll");
    assert!(registry().resolve_deep_link(&link).is_ok());
}

#[test]
fn the_scheme_is_case_insensitive_and_a_trailing_slash_is_dropped() {
    assert!(matches!(
        registry().resolve_deep_link("RBrowser://Settings/sync.scroll/"),
        Ok(DeepLinkTarget::Key { .. })
    ));
}

#[test]
fn a_category_and_the_root_open_too() {
    let registry = registry();
    assert_eq!(
        registry.resolve_deep_link("rbrowser://settings/emulation"),
        Ok(DeepLinkTarget::Category(Category::Emulation))
    );
    assert_eq!(
        registry.resolve_deep_link("rbrowser://settings/"),
        Ok(DeepLinkTarget::Root)
    );
    assert_eq!(
        registry.resolve_deep_link("rbrowser://settings"),
        Ok(DeepLinkTarget::Root)
    );
}

#[test]
fn a_key_wins_over_a_category_of_the_same_name() {
    assert!(matches!(
        registry().resolve_deep_link("rbrowser://settings/breakpoints"),
        Ok(DeepLinkTarget::Key {
            category: Category::Breakpoints,
            ..
        })
    ));
}

#[test]
fn unknown_and_malformed_links_are_refused() {
    let registry = registry();
    assert_eq!(
        registry.resolve_deep_link("rbrowser://settings/sync.nope"),
        Err(SettingsError::UnknownKey("sync.nope".into()))
    );
    for link in [
        "https://settings/sync.scroll",
        "rbrowser://settingsx/sync.scroll",
        "rbrowser://settings/sync.scroll?x=1",
        "rbrowser://settings/sync%2Escroll",
        "rbrowser://settings/../sync",
        "rbrowser://settings/sync/scroll",
    ] {
        assert!(
            matches!(
                registry.resolve_deep_link(link),
                Err(SettingsError::Invalid { .. } | SettingsError::UnknownKey(_))
            ),
            "accepted {link}"
        );
    }
}

use rb_settings::Category;

use super::*;

fn registry() -> SettingsRegistry {
    let (registry, issues) = crate::control::settings::settings_registry();
    assert!(issues.is_empty(), "the schemas read: {issues:?}");
    registry
}

#[test]
fn a_key_opens_its_field_and_wins_over_a_category_of_its_name() {
    let registry = registry();
    assert_eq!(
        resolve(&registry, "sync.scroll"),
        Some(Target::Key("sync.scroll"))
    );
    // `breakpoints` is a key and a category: the key wins, as in the deep links.
    assert_eq!(
        resolve(&registry, "breakpoints"),
        Some(Target::Key("breakpoints"))
    );
}

#[test]
fn a_category_opens_the_category() {
    let registry = registry();
    assert_eq!(
        resolve(&registry, "capture"),
        Some(Target::Category(Category::Capture))
    );
    // `emulation` is a category and the prefix of `emulation.*`: the category wins.
    assert_eq!(
        resolve(&registry, "emulation"),
        Some(Target::Category(Category::Emulation))
    );
}

#[test]
fn a_section_id_opens_its_section() {
    let registry = registry();
    assert_eq!(
        resolve(&registry, "sitemap"),
        Some(Target::Section {
            category: Category::Capture,
            section: "sitemap"
        })
    );
    assert_eq!(
        resolve(&registry, "env-compare"),
        Some(Target::Section {
            category: Category::Capture,
            section: "env-compare"
        })
    );
    // A section id in two categories opens the first in navigation order.
    assert_eq!(
        resolve(&registry, "trust"),
        Some(Target::Section {
            category: Category::Project,
            section: "trust"
        })
    );
}

#[test]
fn a_key_prefix_opens_the_section_or_category_its_keys_share() {
    let registry = registry();
    assert_eq!(
        resolve(&registry, "visual"),
        Some(Target::Section {
            category: Category::Capture,
            section: "visual-regression"
        })
    );
    assert_eq!(
        resolve(&registry, "pageApi"),
        Some(Target::Section {
            category: Category::Sync,
            section: "page-api"
        })
    );
    // `settings.*` sits in two sections of General: the category.
    assert_eq!(
        resolve(&registry, "settings"),
        Some(Target::Category(Category::General))
    );
    // `wsl.*` sits in Reload and in Automation: no one place.
    assert_eq!(resolve(&registry, "wsl"), None);
    // A prefix is whole segments: `sync.scr` names no key.
    assert_eq!(resolve(&registry, "sync.scr"), None);
}

#[test]
fn anything_else_opens_nothing() {
    let registry = registry();
    assert_eq!(resolve(&registry, "no.such"), None);
    assert_eq!(resolve(&registry, ""), None);
    assert_eq!(resolve(&registry, "\"><script>"), None);
}

#[test]
fn the_route_carries_the_section_for_an_area() {
    let registry = registry();
    assert_eq!(route_of(&registry, "sync.scroll"), "settings/sync.scroll");
    assert_eq!(route_of(&registry, "capture"), "settings/capture");
    assert_eq!(
        route_of(&registry, "sitemap"),
        "settings/capture?section=sitemap"
    );
    assert_eq!(
        route_of(&registry, "visual"),
        "settings/capture?section=visual-regression"
    );
    assert_eq!(route_of(&registry, "no.such"), "settings");
}

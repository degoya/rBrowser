use super::*;

#[test]
fn eighteen_categories_in_navigation_order() {
    let ids: Vec<&str> = Category::ALL.iter().map(|c| c.id()).collect();
    assert_eq!(
        ids,
        [
            "general",
            "project",
            "breakpoints",
            "layout",
            "sync",
            "emulation",
            "engines",
            "devtools",
            "reload",
            "network",
            "capture",
            "checks",
            "automation",
            "security",
            "plugins",
            "shortcuts",
            "advanced",
            "about",
        ]
    );
}

#[test]
fn serialized_form_is_the_id() {
    for category in Category::ALL {
        let json = serde_json::to_value(category).expect("serializes");
        assert_eq!(json, serde_json::Value::from(category.id()));
        assert_eq!(Category::from_id(category.id()), Some(category));
    }
}

#[test]
fn every_category_has_a_section_and_ids_are_unique_per_category() {
    for category in Category::ALL {
        let ids: Vec<&str> = category.sections().map(|s| s.id).collect();
        assert!(!ids.is_empty(), "{category:?} has no section");
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "{category:?} repeats a section");
    }
}

#[test]
fn sections_are_grouped_in_category_order() {
    let positions: Vec<usize> = SECTIONS
        .iter()
        .map(|s| Category::ALL.iter().position(|c| *c == s.category))
        .map(|p| p.expect("known category"))
        .collect();
    assert!(positions.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn keys_follow_the_naming_convention() {
    assert_eq!(Category::Sync.title_key(), "settings.category.sync");
    let scroll = section(Category::Sync, "scroll").expect("sync has scroll");
    assert_eq!(scroll.title_key(), "settings.section.sync.scroll");
    assert!(section(Category::Sync, "nope").is_none());
}

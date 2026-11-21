use super::Vocabulary;

#[test]
fn the_pinned_release_loads_with_its_classes_and_properties() {
    let vocabulary = Vocabulary::load();
    assert_eq!(vocabulary.version, "30.1");
    assert!(vocabulary.is_type("Article") && vocabulary.is_type("Organization"));
    assert!(!vocabulary.is_type("Articel"));
    let headline = vocabulary.property("headline").expect("headline");
    assert!(
        headline
            .domains
            .iter()
            .any(|domain| domain == "CreativeWork")
    );
    assert_eq!(headline.ranges, ["Text"]);
    assert!(vocabulary.property("headlne").is_none());
}

#[test]
fn a_type_knows_every_type_above_it() {
    let vocabulary = Vocabulary::load();
    let above = vocabulary.ancestors("NewsArticle");
    for name in ["NewsArticle", "Article", "CreativeWork", "Thing"] {
        assert!(above.contains(name), "{name}");
    }
    assert!(vocabulary.is_a("NewsArticle", "CreativeWork"));
    assert!(!vocabulary.is_a("Person", "CreativeWork"));
}

#[test]
fn data_types_and_enumerations_are_told_apart() {
    let vocabulary = Vocabulary::load();
    assert_eq!(vocabulary.data_type_of("URL").as_deref(), Some("Text"));
    assert_eq!(
        vocabulary.data_type_of("Integer").as_deref(),
        Some("Number")
    );
    assert_eq!(vocabulary.data_type_of("Person"), None);
    assert!(vocabulary.is_enumeration("ItemAvailability"));
    assert!(!vocabulary.is_enumeration("Organization"));
    assert_eq!(vocabulary.enumerations_of("InStock"), ["ItemAvailability"]);
    assert!(vocabulary.enumerations_of("Lamp").is_empty());
}

#[test]
fn a_property_is_allowed_on_its_domains_and_the_types_below_them() {
    let vocabulary = Vocabulary::load();
    let headline = vocabulary.property("headline").expect("headline");
    assert!(vocabulary.allows(&["NewsArticle".to_owned()], headline));
    assert!(!vocabulary.allows(&["Person".to_owned()], headline));
    assert!(vocabulary.allows(&["Person".to_owned(), "Article".to_owned()], headline));
    let name = vocabulary.property("name").expect("name");
    assert!(vocabulary.allows(&["Offer".to_owned()], name));
}

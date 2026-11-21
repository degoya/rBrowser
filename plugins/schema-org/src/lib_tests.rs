use super::short_name;

#[test]
fn a_schema_org_iri_or_compact_name_gives_its_short_name() {
    assert_eq!(short_name("https://schema.org/Product"), Some("Product"));
    assert_eq!(short_name(" http://www.schema.org/Offer/ "), Some("Offer"));
    assert_eq!(short_name("schema:name"), Some("name"));
    assert_eq!(short_name("headline"), Some("headline"));
}

#[test]
fn another_vocabulary_has_no_short_name() {
    assert_eq!(short_name("https://example.org/Thing"), None);
    assert_eq!(short_name("og:title"), None);
}

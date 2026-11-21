use super::table;

#[test]
fn every_required_or_recommended_type_is_a_known_type() {
    let table = table();
    assert_eq!(table.version, 1);
    for name in table.required.keys().chain(table.recommended.keys()) {
        assert!(table.types.contains(name), "{name} is listed but unknown");
    }
}

#[test]
fn the_table_names_the_needs_of_the_common_rich_results() {
    let table = table();
    let product = table.required.get("Product").expect("Product");
    assert_eq!(product.all, ["name"]);
    assert_eq!(product.any, [["offers", "review", "aggregateRating"]]);
    let article = table.recommended.get("Article").expect("Article");
    assert!(article.iter().any(|property| property == "datePublished"));
}

#[test]
fn a_recommended_property_is_never_also_required() {
    let table = table();
    for (name, recommended) in &table.recommended {
        let Some(required) = table.required.get(name) else {
            continue;
        };
        for property in recommended {
            assert!(
                !required.all.contains(property)
                    && !required.any.iter().flatten().any(|other| other == property),
                "{name}.{property} is required and recommended"
            );
        }
    }
}

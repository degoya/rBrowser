use crate::testing::{check, serve, short};

const URL: &str = "https://shop.test/";

fn found(head: &str, body: &str) -> Vec<String> {
    let html = format!("<html><head>{head}</head><body>{body}</body></html>");
    let mut host = serve(URL, &html, &[]);
    short(&check(&mut host, URL, None), "microdata")
}

#[test]
fn the_table_parses_and_knows_its_required_types() {
    let table = super::table();
    assert_eq!(table.version, 1);
    for name in table.required.keys() {
        assert!(table.types.contains(name), "{name} is required but unknown");
    }
}

#[test]
fn a_complete_product_and_breadcrumb_have_no_finding() {
    let body = r#"<div itemscope itemtype="https://schema.org/Product"><h1 itemprop="name">Lamp</h1>
<div itemprop="offers" itemscope itemtype="https://schema.org/Offer"><meta itemprop="price" content="9"></div></div>
<ol vocab="https://schema.org/" typeof="BreadcrumbList"><li property="itemListElement" typeof="ListItem">
<a property="item" href="/"><span property="name">Home</span></a><meta property="position" content="1"></li></ol>"#;
    assert!(found("", body).is_empty(), "{:?}", found("", body));
}

#[test]
fn missing_types_unknown_types_and_required_properties_are_named() {
    let body = r#"<div itemscope><span itemprop="name">x</span></div>
<div itemscope itemtype="https://schema.org/Prodct"></div>
<div itemscope itemtype="https://schema.org/Product"><span itemprop="sku">1</span></div>
<div itemscope itemtype="https://schema.org/FAQPage"><div itemprop="mainEntity" itemscope itemtype="https://schema.org/Question"><span itemprop="name">Q?</span></div></div>"#;
    assert_eq!(
        found("", body),
        [
            "microdata.type_missing microdata.type_missing",
            "microdata.type_unknown microdata.type_unknown",
            "microdata.required_missing microdata.required_missing",
            "microdata.required_missing microdata.required_any",
            "microdata.required_missing microdata.required_missing",
        ]
    );
}

#[test]
fn a_value_that_contradicts_the_json_ld_of_the_same_entity_is_named() {
    let head = r#"<script type="application/ld+json">{"@context": "https://schema.org", "@type": "Product", "name": "Lamp", "sku": "B-2"}</script>"#;
    let body = r#"<div itemscope itemtype="https://schema.org/Product"><span itemprop="name">Lamp</span>
<span itemprop="sku">A-1</span><div itemprop="offers" itemscope itemtype="https://schema.org/Offer"></div></div>"#;
    assert_eq!(
        found(head, body),
        ["microdata.jsonld_conflict microdata.jsonld_conflict"]
    );
}

#[test]
fn entities_with_other_urls_are_not_compared() {
    let head = r#"<script type="application/ld+json">{"@type": "Product", "url": "https://shop.test/b", "name": "B", "offers": {"@type": "Offer"}}</script>"#;
    let body = r#"<div itemscope itemtype="https://schema.org/Product"><a itemprop="url" href="https://shop.test/a">A</a>
<span itemprop="name">A</span><div itemprop="offers" itemscope itemtype="https://schema.org/Offer"></div></div>"#;
    assert!(found(head, body).is_empty());
}

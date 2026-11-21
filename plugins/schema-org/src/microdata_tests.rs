use super::*;

fn parse(body: &str) -> Vec<Item> {
    items(&Html::parse_document(&format!(
        "<html><body>{body}</body></html>"
    )))
}

#[test]
fn microdata_items_nest_and_read_each_element_s_value() {
    let items = parse(
        r#"<div itemscope itemtype="https://schema.org/Product">
  <h1 itemprop="name">Lamp <small>LED</small></h1>
  <img itemprop="image" src="/lamp.jpg" alt="">
  <a itemprop="url" href="https://shop.test/lamp">Lamp</a>
  <div itemprop="offers" itemscope itemtype="http://schema.org/Offer">
    <meta itemprop="price" content="19.99"><span itemprop="name">not the product's</span>
  </div>
  <section><time itemprop="releaseDate" datetime="2026-01-02">January</time></section>
</div>"#,
    );
    assert_eq!(items.len(), 1);
    let product = &items[0];
    assert_eq!(product.syntax, Syntax::Microdata);
    assert_eq!(product.types, ["Product"]);
    assert_eq!(product.text("name"), Some("Lamp LED"));
    assert_eq!(product.text("image"), Some("/lamp.jpg"));
    assert_eq!(product.text("url"), Some("https://shop.test/lamp"));
    assert_eq!(product.text("releaseDate"), Some("2026-01-02"));
    assert_eq!(product.path, "html > body > div");
    let Some([Value::Item(offer)]) = product.props.get("offers").map(Vec::as_slice) else {
        panic!("one nested offer: {:?}", product.props.get("offers"));
    };
    assert_eq!(offer.types, ["Offer"]);
    assert_eq!(offer.text("price"), Some("19.99"));
}

#[test]
fn rdfa_lite_resolves_its_vocabulary_and_prefix() {
    let items = parse(
        r#"<div vocab="https://schema.org/" typeof="BreadcrumbList">
  <span property="itemListElement" typeof="ListItem"><a property="item" href="/a"><span property="name">A</span></a><meta property="position" content="1"></span>
</div>
<p typeof="schema:Organization"><span property="schema:name">ACME</span></p>
<p typeof="Thing">no vocabulary</p>"#,
    );
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].syntax, Syntax::Rdfa);
    assert_eq!(items[0].types, ["BreadcrumbList"]);
    let Some([Value::Item(entry)]) = items[0].props.get("itemListElement").map(Vec::as_slice)
    else {
        panic!("one list item");
    };
    assert_eq!(entry.text("position"), Some("1"));
    assert_eq!(items[1].types, ["Organization"]);
    assert_eq!(items[1].text("name"), Some("ACME"));
    assert!(items[2].types.is_empty());
    assert_eq!(items[2].other_types, ["Thing"]);
}

#[test]
fn an_item_without_type_and_one_of_another_vocabulary_are_kept_apart() {
    let items = parse(
        r#"<div itemscope><span itemprop="name">x</span></div>
<div itemscope itemtype="https://example.org/Thing"></div>"#,
    );
    assert_eq!(items.len(), 2);
    assert!(items[0].types.is_empty() && items[0].other_types.is_empty());
    assert_eq!(items[1].other_types, ["https://example.org/Thing"]);
}

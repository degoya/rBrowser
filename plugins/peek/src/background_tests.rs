use serde_json::json;

use super::*;
use crate::color::parse;

fn layer(color: &str, image: bool) -> Layer {
    Layer {
        color: parse(color).expect("a colour"),
        image,
    }
}

#[test]
fn the_first_opaque_ancestor_is_the_background() {
    let layers = [
        layer("transparent", false),
        layer("rgb(255, 255, 255)", false),
        layer("rgb(0, 0, 0)", false),
    ];
    assert_eq!(
        resolve(&layers),
        Resolved {
            color: parse("#fff").expect("white"),
            uncertain: false,
        }
    );
}

#[test]
fn translucent_layers_are_blended_over_what_lies_below() {
    // Half black over white: #808080.
    let layers = [layer("rgba(0, 0, 0, 0.5)", false), layer("#fff", false)];
    let resolved = resolve(&layers);
    assert_eq!(resolved.color.hex(), "#808080");
    assert!(!resolved.uncertain);
}

#[test]
fn without_an_opaque_layer_the_canvas_is_white() {
    let resolved = resolve(&[
        layer("transparent", false),
        layer("rgba(255, 0, 0, 0.5)", false),
    ]);
    assert_eq!(resolved.color.hex(), "#ff8080");
    assert!(!resolved.uncertain);
    assert_eq!(resolve(&[]).color, Rgba::WHITE);
}

#[test]
fn an_image_or_gradient_on_the_way_makes_it_uncertain() {
    let resolved = resolve(&[
        layer("transparent", false),
        layer("transparent", true),
        layer("#000", false),
    ]);
    assert!(resolved.uncertain);
    assert_eq!(resolved.color.hex(), "#000000");
    // An image behind the first opaque layer is covered: certain.
    let covered = resolve(&[layer("#fff", false), layer("#000", true)]);
    assert!(!covered.uncertain);
}

#[test]
fn layers_are_read_from_the_page_data_and_bad_ones_dropped() {
    let data = json!([
        { "color": "rgba(0, 0, 0, 0)", "image": false },
        { "color": "oklch(1 0 0)" },
        { "color": "rgb(10, 20, 30)", "image": true },
        "nonsense",
    ]);
    let (layers, unknown) = layers_from_json(&data);
    assert_eq!(layers.len(), 2);
    assert!(unknown);
    assert!(layers[1].image);
    let (layers, unknown) = layers_from_json(&json!(null));
    assert!(layers.is_empty());
    assert!(!unknown);
}

#[test]
fn at_most_max_layers_are_read() {
    let many: Vec<_> = (0..500)
        .map(|_| json!({ "color": "transparent" }))
        .collect();
    let (layers, _) = layers_from_json(&json!(many));
    assert_eq!(layers.len(), MAX_LAYERS);
}

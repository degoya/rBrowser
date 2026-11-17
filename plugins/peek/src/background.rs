//! The background a text sits on, resolved over its ancestors (RB-060-04): the page script sends
//! the background of the element and of every ancestor up to the root, innermost first; the
//! first opaque one ends the walk, the translucent ones before it are blended over it, and the
//! canvas below everything is white. A background image or gradient on the way makes the result
//! **uncertain**: the colour under the text cannot be known from styles alone.
//!
//! The walk follows the DOM, not the painting order: an element positioned over another one is
//! not seen here - the tooltip says "uncertain" only for images.

use serde_json::Value;

use crate::color::{Rgba, parse};

/// The most layers read from the page (the DOM's depth beyond this is not followed).
pub const MAX_LAYERS: usize = 64;

/// The background of one element: its `background-color` and whether it has a
/// `background-image` (an image or a gradient).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    pub color: Rgba,
    pub image: bool,
}

/// The resolved background: opaque, and whether an image or gradient may change it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolved {
    pub color: Rgba,
    pub uncertain: bool,
}

/// Resolves `layers` (innermost first) to the opaque colour behind the text.
#[must_use]
pub fn resolve(layers: &[Layer]) -> Resolved {
    let mut uncertain = false;
    let mut seen = Vec::new();
    for layer in layers.iter().take(MAX_LAYERS) {
        // An image paints over its own element's colour, so it counts even on an opaque layer.
        uncertain |= layer.image;
        seen.push(layer.color);
        if layer.color.is_opaque() {
            break;
        }
    }
    let color = seen
        .iter()
        .rev()
        .fold(Rgba::WHITE, |below, layer| layer.over(below));
    Resolved { color, uncertain }
}

/// The layers of the page data's `backgrounds` (`[{"color", "image"}]`), and whether one was
/// dropped because its colour is no sRGB colour (the result is then uncertain too).
#[must_use]
pub fn layers_from_json(value: &Value) -> (Vec<Layer>, bool) {
    let mut unknown = false;
    let layers = value
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .take(MAX_LAYERS)
        .filter_map(|entry| {
            let color = entry.get("color").and_then(Value::as_str)?;
            let Some(color) = parse(color) else {
                unknown = true;
                return None;
            };
            let image = entry.get("image").and_then(Value::as_bool).unwrap_or(false);
            Some(Layer { color, image })
        })
        .collect();
    (layers, unknown)
}

#[cfg(test)]
#[path = "background_tests.rs"]
mod tests;

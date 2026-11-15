//! One element's inspection (RB-060-04): the page script's data - read from computed style in
//! the page, untrusted as anything the page shapes - checked and cut, the contrast rated
//! ([`crate::contrast`]) over the resolved background ([`crate::background`]), and what CDP adds
//! ([`crate::node`]) put in. The tooltip and the MCP tool `peek_inspect` show the same JSON.

use serde_json::{Map, Value, json};

use crate::background::{layers_from_json, resolve};
use crate::color::{parse, round2};
use crate::contrast::Contrast;
use crate::node::Extras;

pub const MAX_TAG: usize = 64;
pub const MAX_ID_CHARS: usize = 128;
pub const MAX_CLASSES: usize = 32;
pub const MAX_CLASS_CHARS: usize = 64;
pub const MAX_STYLE_CHARS: usize = 256;
pub const MAX_NAME_CHARS: usize = 200;
/// The largest length taken, in CSS px; beyond it the page lies.
pub const MAX_PX: f64 = 10_000_000.0;

fn text(value: &Value, max: usize) -> String {
    value
        .as_str()
        .unwrap_or_default()
        .chars()
        .take(max)
        .collect()
}

/// A length: finite, clamped, two decimals; anything else is zero.
fn px(value: &Value) -> f64 {
    value
        .as_f64()
        .filter(|number| number.is_finite())
        .map_or(0.0, |number| round2(number.clamp(-MAX_PX, MAX_PX)))
}

/// `[top, right, bottom, left]` as an object.
fn sides(value: &Value) -> Value {
    let side = |index: usize| px(value.get(index).unwrap_or(&Value::Null));
    json!({ "top": side(0), "right": side(1), "bottom": side(2), "left": side(3) })
}

fn sum_x(value: &Value) -> f64 {
    px(&value[1]) + px(&value[3])
}

fn sum_y(value: &Value) -> f64 {
    px(&value[0]) + px(&value[2])
}

fn size(page: &Value) -> Value {
    let width = px(&page["rect"]["width"]).max(0.0);
    let height = px(&page["rect"]["height"]).max(0.0);
    let dpr = page["dpr"]
        .as_f64()
        .filter(|dpr| dpr.is_finite() && *dpr > 0.0 && *dpr <= 16.0)
        .unwrap_or(1.0);
    json!({
        "width": width,
        "height": height,
        "dpr": dpr,
        "deviceWidth": round2(width * dpr),
        "deviceHeight": round2(height * dpr),
    })
}

fn box_model(page: &Value) -> Value {
    let model = &page["box"];
    let (border, padding) = (&model["border"], &model["padding"]);
    let width = px(&page["rect"]["width"]) - sum_x(border) - sum_x(padding);
    let height = px(&page["rect"]["height"]) - sum_y(border) - sum_y(padding);
    json!({
        "margin": sides(&model["margin"]),
        "border": sides(border),
        "padding": sides(padding),
        "content": { "width": round2(width.max(0.0)), "height": round2(height.max(0.0)) },
    })
}

/// The inspection of the element the page script described in `page`.
#[must_use]
pub fn build(page: &Value, extras: &Extras) -> Value {
    let font = &page["font"];
    let font_size = px(&font["size"]).max(0.0);
    let weight = font["weight"].as_u64().unwrap_or(400).clamp(1, 1000);
    let foreground = page["color"].as_str().and_then(parse);
    let (layers, unknown) = layers_from_json(&page["backgrounds"]);
    let background = resolve(&layers);
    let contrast =
        foreground.map(|color| Contrast::of(color, background.color, font_size, weight as u32));

    let classes: Vec<String> = page["classes"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .take(MAX_CLASSES)
        .map(|class| text(class, MAX_CLASS_CHARS))
        .collect();

    let (role, name, source) = match (&extras.role, &extras.name) {
        (Some(role), name) => (role.clone(), name.clone().unwrap_or_default(), "cdp"),
        (None, _) => (
            text(&page["role"], MAX_NAME_CHARS),
            text(&page["name"], MAX_NAME_CHARS),
            "page",
        ),
    };
    let rendered = extras.rendered_fonts.as_ref().map_or(Value::Null, |fonts| {
        Value::Array(fonts.iter().map(|font| font.to_json()).collect())
    });

    let mut inspection = Map::new();
    inspection.insert("tag".into(), json!(text(&page["tag"], MAX_TAG)));
    inspection.insert("id".into(), json!(text(&page["id"], MAX_ID_CHARS)));
    inspection.insert("classes".into(), json!(classes));
    inspection.insert("size".into(), size(page));
    inspection.insert("box".into(), box_model(page));
    inspection.insert(
        "colors".into(),
        json!({
            "foreground": contrast.as_ref().map(|contrast| contrast.foreground.hex()),
            "background": contrast.as_ref().map_or_else(|| background.color.hex(), |contrast| contrast.background.hex()),
            "backgroundUncertain": background.uncertain || unknown,
        }),
    );
    inspection.insert(
        "contrast".into(),
        contrast.as_ref().map_or(Value::Null, Contrast::to_json),
    );
    inspection.insert(
        "font".into(),
        json!({
            "declared": text(&font["family"], MAX_STYLE_CHARS),
            "rendered": rendered,
            "size": font_size,
            "weight": weight,
            "style": text(&font["style"], MAX_STYLE_CHARS),
            "lineHeight": text(&font["lineHeight"], MAX_STYLE_CHARS),
            "letterSpacing": text(&font["letterSpacing"], MAX_STYLE_CHARS),
        }),
    );
    inspection.insert("role".into(), json!(role));
    inspection.insert("name".into(), json!(name));
    inspection.insert("accessibility".into(), json!(source));
    Value::Object(inspection)
}

#[cfg(test)]
#[path = "inspect_tests.rs"]
mod tests;

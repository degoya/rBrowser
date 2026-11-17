use serde_json::{Value, json};

use super::*;
use crate::node::{Extras, RenderedFont};

fn page_data() -> Value {
    json!({
        "tag": "button",
        "id": "send",
        "classes": ["btn", "btn-primary"],
        "rect": { "x": 10, "y": 20, "width": 120.5, "height": 40 },
        "dpr": 2,
        "box": { "margin": [4, 8, 4, 8], "border": [1, 1, 1, 1], "padding": [6, 12, 6, 12] },
        "color": "rgb(119, 119, 119)",
        "backgrounds": [
            { "color": "rgba(0, 0, 0, 0)", "image": false },
            { "color": "rgb(255, 255, 255)", "image": false },
        ],
        "font": { "family": "\"Inter\", sans-serif", "size": 16, "weight": 400,
                  "lineHeight": "24px", "letterSpacing": "normal", "style": "normal" },
        "role": "button",
        "name": "Send",
        "selector": "#send",
    })
}

#[test]
fn the_fixture_button_gives_every_field() {
    let inspection = build(&page_data(), &Extras::default());
    assert_eq!(inspection["tag"], "button");
    assert_eq!(inspection["id"], "send");
    assert_eq!(inspection["classes"], json!(["btn", "btn-primary"]));
    assert_eq!(
        inspection["size"],
        json!({ "width": 120.5, "height": 40.0, "dpr": 2.0, "deviceWidth": 241.0, "deviceHeight": 80.0 })
    );
    assert_eq!(
        inspection["box"]["padding"],
        json!({ "top": 6.0, "right": 12.0, "bottom": 6.0, "left": 12.0 })
    );
    // 120.5 - 2 x 1 border - 2 x 12 padding; 40 - 2 - 12.
    assert_eq!(
        inspection["box"]["content"],
        json!({ "width": 94.5, "height": 26.0 })
    );
    assert_eq!(inspection["colors"]["foreground"], "#777777");
    assert_eq!(inspection["colors"]["background"], "#ffffff");
    assert_eq!(inspection["colors"]["backgroundUncertain"], false);
    assert_eq!(inspection["contrast"]["ratio"], 4.48);
    assert_eq!(inspection["contrast"]["level"], "aa_large");
    assert_eq!(inspection["font"]["declared"], "\"Inter\", sans-serif");
    assert_eq!(inspection["font"]["size"], 16.0);
    assert_eq!(inspection["font"]["weight"], 400);
    assert_eq!(inspection["font"]["lineHeight"], "24px");
    assert_eq!(inspection["font"]["rendered"], Value::Null);
    assert_eq!(inspection["role"], "button");
    assert_eq!(inspection["name"], "Send");
    assert_eq!(inspection["accessibility"], "page");
}

#[test]
fn cdp_wins_for_role_name_and_rendered_font() {
    let extras = Extras {
        rendered_fonts: Some(vec![RenderedFont {
            family: "Arial".to_owned(),
            post_script_name: "ArialMT".to_owned(),
            custom: false,
            glyphs: 4,
        }]),
        role: Some("link".to_owned()),
        name: Some("Home".to_owned()),
    };
    let inspection = build(&page_data(), &extras);
    assert_eq!(inspection["role"], "link");
    assert_eq!(inspection["name"], "Home");
    assert_eq!(inspection["accessibility"], "cdp");
    assert_eq!(inspection["font"]["rendered"][0]["family"], "Arial");
}

#[test]
fn large_bold_text_is_rated_as_large() {
    let mut data = page_data();
    data["font"]["size"] = json!(18.6667);
    data["font"]["weight"] = json!(700);
    let inspection = build(&data, &Extras::default());
    assert_eq!(inspection["contrast"]["largeText"], true);
    assert_eq!(inspection["contrast"]["level"], "aa");
}

#[test]
fn a_gradient_or_an_unknown_colour_makes_the_background_uncertain() {
    let mut data = page_data();
    data["backgrounds"][0]["image"] = json!(true);
    assert_eq!(
        build(&data, &Extras::default())["colors"]["backgroundUncertain"],
        true
    );
    let mut data = page_data();
    data["backgrounds"][0]["color"] = json!("oklch(0.5 0.1 100)");
    assert_eq!(
        build(&data, &Extras::default())["colors"]["backgroundUncertain"],
        true
    );
}

#[test]
fn an_unknown_text_colour_leaves_the_contrast_out() {
    let mut data = page_data();
    data["color"] = json!("lab(50 0 0)");
    let inspection = build(&data, &Extras::default());
    assert_eq!(inspection["contrast"], Value::Null);
    assert_eq!(inspection["colors"]["foreground"], Value::Null);
}

#[test]
fn hostile_page_data_is_cut_and_bad_numbers_are_zero() {
    let data = json!({
        "tag": "x".repeat(500),
        "classes": (0..100).map(|index| format!("c{index}{}", "y".repeat(200))).collect::<Vec<_>>(),
        "rect": { "width": "wide", "height": f64::MAX },
        "dpr": -3,
        "box": { "margin": [1, 2] },
        "font": { "size": "big", "weight": 99999 },
    });
    let inspection = build(&data, &Extras::default());
    assert_eq!(inspection["tag"].as_str().map(str::len), Some(MAX_TAG));
    assert_eq!(
        inspection["classes"].as_array().map(Vec::len),
        Some(MAX_CLASSES)
    );
    assert_eq!(
        inspection["classes"][0]
            .as_str()
            .map(|class| class.chars().count()),
        Some(MAX_CLASS_CHARS)
    );
    assert_eq!(inspection["size"]["width"], 0.0);
    assert_eq!(inspection["size"]["height"], MAX_PX);
    assert_eq!(inspection["size"]["dpr"], 1.0);
    assert_eq!(inspection["box"]["margin"]["top"], 1.0);
    assert_eq!(inspection["box"]["margin"]["bottom"], 0.0);
    assert_eq!(inspection["font"]["size"], 0.0);
    assert_eq!(inspection["font"]["weight"], 1000);
    assert_eq!(inspection["id"], "");
}

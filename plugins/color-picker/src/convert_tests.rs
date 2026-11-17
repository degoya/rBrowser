use super::*;
use crate::color::{Rgba, parse};

fn rgb(r: f64, g: f64, b: f64) -> Rgba {
    Rgba { r, g, b, a: 1.0 }
}

#[test]
fn the_fixture_blue_in_every_format() {
    let blue = parse("#1e88e5").expect("hex");
    let formats = Formats::of(blue);
    assert_eq!(formats.hex, "#1e88e5");
    assert_eq!(formats.rgb, "rgb(30, 136, 229)");
    assert_eq!(formats.hsl, "hsl(208, 79.3%, 50.8%)");
    assert_eq!(formats.oklch, "oklch(61.79% 0.16678 250.87)");
    // Every format reads back as the same 8-bit colour.
    for text in [&formats.rgb, &formats.hsl, &formats.oklch] {
        assert_eq!(
            parse(text).map(Rgba::hex).as_deref(),
            Some("#1e88e5"),
            "{text}"
        );
    }
}

#[test]
fn oklch_of_known_colours() {
    // CSS Color 4: red is oklch(62.796% 0.25768 29.2339).
    assert_eq!(
        oklch_text(rgb(255.0, 0.0, 0.0)),
        "oklch(62.8% 0.25768 29.23)"
    );
    assert_eq!(oklch_text(rgb(255.0, 255.0, 255.0)), "oklch(100% 0 0)");
    assert_eq!(oklch_text(rgb(0.0, 0.0, 0.0)), "oklch(0% 0 0)");
    // A grey has no hue.
    assert!(oklch_text(rgb(118.0, 118.0, 118.0)).ends_with(" 0 0)"));
}

#[test]
fn translucent_colours_carry_their_alpha() {
    let half = Rgba {
        r: 255.0,
        g: 0.0,
        b: 0.0,
        a: 0.5,
    };
    let formats = Formats::of(half);
    assert_eq!(formats.hex, "#ff000080");
    assert_eq!(formats.rgb, "rgba(255, 0, 0, 0.5)");
    assert_eq!(formats.hsl, "hsla(0, 100%, 50%, 0.5)");
    assert!(formats.oklch.ends_with(" / 0.5)"), "{}", formats.oklch);
}

#[test]
fn every_byte_colour_survives_the_round_trip() {
    for r in (0..=255).step_by(15) {
        for g in (0..=255).step_by(17) {
            for b in (0..=255).step_by(51) {
                let color = rgb(f64::from(r), f64::from(g), f64::from(b));
                let formats = Formats::of(color);
                for text in [&formats.hsl, &formats.oklch] {
                    assert_eq!(
                        parse(text).map(Rgba::hex),
                        Some(color.hex()),
                        "{text} for {}",
                        color.hex()
                    );
                }
            }
        }
    }
}

#[test]
fn the_default_format_is_read_from_its_name() {
    assert_eq!(Format::from_name("hex"), Format::Hex);
    assert_eq!(Format::from_name("rgb"), Format::Rgb);
    assert_eq!(Format::from_name("hsl"), Format::Hsl);
    assert_eq!(Format::from_name("oklch"), Format::Oklch);
    assert_eq!(Format::from_name("cmyk"), Format::Hex);
    let formats = Formats::of(rgb(0.0, 0.0, 0.0));
    assert_eq!(formats.get(Format::Rgb), "rgb(0, 0, 0)");
    assert_eq!(formats.to_json()["hex"], "#000000");
}

#[test]
fn oklab_distance_tells_near_colours_apart() {
    let a = oklab(rgb(30.0, 136.0, 229.0));
    let b = oklab(rgb(31.0, 136.0, 229.0));
    let c = oklab(rgb(229.0, 57.0, 53.0));
    assert!(distance(a, b) < 0.01);
    assert!(distance(a, c) > 0.2);
}

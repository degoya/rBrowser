use super::*;

fn rgb(r: f64, g: f64, b: f64) -> Rgba {
    Rgba { r, g, b, a: 1.0 }
}

#[test]
fn the_computed_forms_of_chromium_parse() {
    assert_eq!(parse("rgb(119, 119, 119)"), Some(rgb(119.0, 119.0, 119.0)));
    assert_eq!(
        parse("rgba(0, 0, 0, 0.5)"),
        Some(Rgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.5
        })
    );
    assert_eq!(
        parse("rgb(10 20 30 / 25%)"),
        Some(Rgba {
            r: 10.0,
            g: 20.0,
            b: 30.0,
            a: 0.25
        })
    );
    assert_eq!(parse("  RGB(1,2,3)  "), Some(rgb(1.0, 2.0, 3.0)));
    assert_eq!(parse("transparent"), Some(Rgba::TRANSPARENT));
}

#[test]
fn hex_and_srgb_parse() {
    assert_eq!(parse("#777"), Some(rgb(119.0, 119.0, 119.0)));
    assert_eq!(parse("#ffffff"), Some(rgb(255.0, 255.0, 255.0)));
    assert_eq!(
        parse("#00000080"),
        Some(Rgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 128.0 / 255.0
        })
    );
    assert_eq!(parse("color(srgb 1 0 0.5)"), Some(rgb(255.0, 0.0, 127.5)));
    assert_eq!(
        parse("color(srgb 0 0 0 / 0.5)"),
        Some(Rgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.5
        })
    );
}

#[test]
fn what_is_no_srgb_colour_is_refused() {
    for text in [
        "",
        "red",
        "oklch(0.5 0.1 120)",
        "rgb(1, 2)",
        "rgb(1, 2, 3, 4, 5)",
        "rgb(a, b, c)",
        "#12",
        "#gggggg",
        "color(display-p3 1 0 0)",
        "rgb(1, 2, 3",
    ] {
        assert_eq!(parse(text), None, "{text:?}");
    }
    assert_eq!(parse(&"9".repeat(500)), None);
}

#[test]
fn channels_are_clamped() {
    assert_eq!(parse("rgb(300, -5, 20)"), Some(rgb(255.0, 0.0, 20.0)));
    assert_eq!(parse("rgba(0, 0, 0, 7)").map(|c| c.a), Some(1.0));
}

#[test]
fn source_over_blends_by_alpha() {
    let black_half = Rgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.5,
    };
    let blended = black_half.over(rgb(255.0, 255.0, 255.0));
    assert!((blended.r - 127.5).abs() < 1e-9);
    assert!((blended.a - 1.0).abs() < 1e-9);
    assert_eq!(
        rgb(1.0, 2.0, 3.0).over(Rgba::TRANSPARENT),
        rgb(1.0, 2.0, 3.0)
    );
    assert_eq!(Rgba::TRANSPARENT.over(Rgba::TRANSPARENT), Rgba::TRANSPARENT);
}

#[test]
fn luminance_and_contrast_follow_wcag() {
    let white = rgb(255.0, 255.0, 255.0);
    let black = rgb(0.0, 0.0, 0.0);
    assert!((white.luminance() - 1.0).abs() < 1e-12);
    assert!(black.luminance().abs() < 1e-12);
    assert!((contrast_ratio(white, black) - 21.0).abs() < 1e-9);
    assert!((contrast_ratio(black, white) - 21.0).abs() < 1e-9);
    // The job's fixture pair: #777 on #fff is 4.48:1.
    let grey = rgb(119.0, 119.0, 119.0);
    assert_eq!(round2(contrast_ratio(grey, white)), 4.48);
    // Two more known pairs.
    assert_eq!(
        round2(contrast_ratio(rgb(118.0, 118.0, 118.0), white)),
        4.54
    );
    assert_eq!(round2(contrast_ratio(rgb(0.0, 0.0, 255.0), white)), 8.59);
}

#[test]
fn hex_text_of_a_colour() {
    assert_eq!(rgb(119.0, 119.0, 119.0).hex(), "#777777");
    assert_eq!(
        Rgba {
            r: 255.0,
            g: 0.0,
            b: 16.0,
            a: 0.5
        }
        .hex(),
        "#ff001080"
    );
    assert_eq!(rgb(127.5, 0.0, 0.0).hex(), "#800000");
}

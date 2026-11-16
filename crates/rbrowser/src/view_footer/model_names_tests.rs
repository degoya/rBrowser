//! The class name in the footer from `2xl` on (RB-090-50): every name the analysis gives a
//! boundary reaches the strip, those that begin with a digit and the project's own too.

use std::collections::BTreeSet;

use rb_config::FooterField;
use rb_ipc::{BoundaryKind, DetectedBreakpoint};

use super::*;

/// The boundaries of a UnoCSS page named after its config, `ultra` declared but used by no query
/// yet (`count` 0).
fn named() -> Vec<DetectedBreakpoint> {
    [
        (768, "sm", 2),
        (1024, "md", 3),
        (1280, "lg", 2),
        (1600, "xl", 2),
        (1920, "2xl", 2),
        (2000, "3xl", 1),
        (2560, "ultra", 0),
    ]
    .into_iter()
    .map(|(width, label, count)| DetectedBreakpoint {
        width,
        kind: BoundaryKind::Min,
        label: Some(label.to_owned()),
        count,
        source: Some(format!("uno.config.ts: {label}")),
        heuristic: false,
    })
    .collect()
}

/// The strip of a view `width` px wide with the default fields (range and class).
fn strip(width: u32, detected: &[DetectedBreakpoint]) -> String {
    let fields = BTreeSet::from([FooterField::Range, FooterField::Framework]);
    text(&FooterInput {
        breakpoint: Some("w1920"),
        width,
        height: 1000,
        dpr: 1.0,
        config_widths: &[375, 768, 1024, 1280, 1600, 1920, 2000, 2320],
        detected,
        fields: &fields,
    })
    .text
}

#[test]
fn every_wide_view_names_its_class_2xl_and_3xl_too() {
    let detected = named();
    assert_eq!(strip(1280, &detected), "1280-1599px | lg");
    assert_eq!(strip(1600, &detected), "1600-1919px | xl");
    assert_eq!(strip(1920, &detected), "1920-1999px | 2xl");
    assert_eq!(strip(2000, &detected), "2000-2559px | 3xl");
    assert_eq!(strip(2320, &detected), "2000-2559px | 3xl");
    assert_eq!(strip(3000, &detected), "2560-\u{221e}px | ultra");
    assert_eq!(strip(375, &detected), "0-767px");
}

#[test]
fn a_name_that_begins_with_a_digit_is_kept_as_it_is() {
    for name in ["2xl", "3xl", "4k", "wide-1"] {
        let detected = [DetectedBreakpoint {
            width: 1536,
            kind: BoundaryKind::Min,
            label: Some(name.to_owned()),
            count: 1,
            source: None,
            heuristic: false,
        }];
        assert_eq!(framework_at(1700, &detected).as_deref(), Some(name));
        assert_eq!(strip(1700, &detected), format!("1536-\u{221e}px | {name}"));
    }
}

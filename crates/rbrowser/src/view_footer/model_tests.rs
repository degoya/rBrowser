use std::collections::BTreeSet;

use rb_config::FooterField;
use rb_ipc::{BoundaryKind, DetectedBreakpoint};

use super::*;

const WIDTHS: [u32; 4] = [1440, 360, 768, 1024];

fn boundary(width: u32, kind: BoundaryKind, label: Option<&str>) -> DetectedBreakpoint {
    DetectedBreakpoint {
        width,
        kind,
        label: label.map(str::to_owned),
        count: 1,
        source: None,
        heuristic: false,
    }
}

fn min(width: u32) -> DetectedBreakpoint {
    boundary(width, BoundaryKind::Min, None)
}

fn max(width: u32) -> DetectedBreakpoint {
    boundary(width, BoundaryKind::Max, None)
}

fn all_fields() -> BTreeSet<FooterField> {
    BTreeSet::from([
        FooterField::Breakpoint,
        FooterField::Range,
        FooterField::Size,
        FooterField::Queries,
        FooterField::Framework,
    ])
}

fn input<'a>(
    width: u32,
    detected: &'a [DetectedBreakpoint],
    fields: &'a BTreeSet<FooterField>,
) -> FooterInput<'a> {
    FooterInput {
        breakpoint: Some("md"),
        width,
        height: 1024,
        dpr: 2.0,
        config_widths: &WIDTHS,
        detected,
        fields,
    }
}

#[test]
fn a_breakpoint_ranges_from_its_width_to_the_next_one() {
    assert_eq!(
        range(768, &WIDTHS),
        Range {
            from: 768,
            to: Some(1023)
        }
    );
    assert_eq!(
        range(1024, &WIDTHS),
        Range {
            from: 1024,
            to: Some(1439)
        }
    );
}

#[test]
fn the_smallest_breakpoint_ranges_from_zero() {
    // The owner's reference look: `xs | 0-767px | 360px`.
    assert_eq!(
        range(360, &WIDTHS),
        Range {
            from: 0,
            to: Some(767)
        }
    );
}

#[test]
fn the_largest_breakpoint_has_no_upper_end() {
    assert_eq!(
        range(1440, &WIDTHS),
        Range {
            from: 1440,
            to: None
        }
    );
    assert_eq!(
        range(2560, &WIDTHS),
        Range {
            from: 1440,
            to: None
        }
    );
}

#[test]
fn a_free_size_takes_the_range_of_its_config_neighbours() {
    assert_eq!(
        range(800, &WIDTHS),
        Range {
            from: 768,
            to: Some(1023)
        }
    );
    assert_eq!(
        range(500, &WIDTHS),
        Range {
            from: 0,
            to: Some(767)
        }
    );
    assert_eq!(
        range(200, &WIDTHS),
        Range {
            from: 0,
            to: Some(359)
        }
    );
}

#[test]
fn a_range_without_config_widths_is_everything() {
    assert_eq!(range(800, &[]), Range { from: 0, to: None });
    // Repeated widths count once.
    assert_eq!(
        range(768, &[768, 768, 1024]),
        Range {
            from: 0,
            to: Some(1023)
        }
    );
}

#[test]
fn the_range_reads_as_px() {
    assert_eq!(
        Range {
            from: 768,
            to: Some(1023)
        }
        .text(),
        "768-1023px"
    );
    assert_eq!(
        Range {
            from: 1440,
            to: None
        }
        .text(),
        "1440-\u{221e}px"
    );
}

#[test]
fn the_queries_that_hold_at_a_width_are_the_tightest_first() {
    let detected = [
        min(640),
        max(1023),
        min(768),
        max(1279),
        min(1024),
        max(767),
    ];
    assert_eq!(
        queries_at(768, &detected),
        [
            "min-width 768px",
            "max-width 1023px",
            "min-width 640px",
            "max-width 1279px"
        ]
    );
    // A boundary holds at its own width, on both sides.
    assert_eq!(
        queries_at(767, &detected),
        [
            "min-width 640px",
            "max-width 767px",
            "max-width 1023px",
            "max-width 1279px"
        ]
    );
    assert!(queries_at(2000, &[max(1023)]).is_empty());
    assert!(queries_at(320, &[]).is_empty());
}

#[test]
fn queries_of_one_kind_fill_both_places() {
    let detected = [min(640), min(768), min(1024)];
    assert_eq!(
        queries_at(1100, &detected),
        ["min-width 1024px", "min-width 768px", "min-width 640px"]
    );
    // The same boundary found twice is named once.
    assert_eq!(queries_at(700, &[min(640), min(640)]), ["min-width 640px"]);
}

#[test]
fn the_framework_names_the_tightest_boundary_that_holds() {
    let detected = [
        boundary(640, BoundaryKind::Min, Some("sm")),
        boundary(768, BoundaryKind::Min, Some("md")),
        boundary(1024, BoundaryKind::Min, Some("lg")),
    ];
    assert_eq!(framework_at(900, &detected).as_deref(), Some("md"));
    assert_eq!(framework_at(1024, &detected).as_deref(), Some("lg"));
    assert_eq!(framework_at(360, &detected), None);
    assert_eq!(framework_at(900, &[min(768)]), None);
}

#[test]
fn a_tighter_unnamed_boundary_leaves_the_width_without_a_framework_name() {
    // 800 is the page's own boundary: at 900 the width is past `md`'s step, in no named one.
    let detected = [boundary(768, BoundaryKind::Min, Some("md")), min(800)];
    assert_eq!(framework_at(900, &detected), None);
    assert_eq!(framework_at(790, &detected).as_deref(), Some("md"));
}

#[test]
fn a_boundary_found_twice_keeps_its_name() {
    // The unnamed find comes first: the name of the second is not lost.
    let detected = [min(768), boundary(768, BoundaryKind::Min, Some("md"))];
    assert_eq!(queries_at(800, &detected), ["min-width 768px"]);
    assert_eq!(framework_at(800, &detected).as_deref(), Some("md"));
}

#[test]
fn another_document_is_told_from_a_jump_inside_the_page() {
    assert_eq!(
        document_of("https://a.test/x?q=1#top"),
        "https://a.test/x?q=1"
    );
    assert_eq!(document_of("https://a.test/x"), "https://a.test/x");
    assert_eq!(document_of("#only"), "");
}

#[test]
fn a_window_grows_and_shrinks_by_the_footer() {
    assert_eq!(window_height(800, 0, 18), 818);
    assert_eq!(window_height(818, 18, 0), 800);
    // A second line, and back to one (RB-080-53).
    assert_eq!(window_height(818, 18, 36), 836);
    assert_eq!(window_height(836, 36, 18), 818);
    assert_eq!(window_height(800, 36, 36), 800);
    // Never a side of 0: CEF takes such a size for empty.
    assert_eq!(window_height(10, 18, 0), 1);
    assert_eq!(window_height(i32::MAX, 0, 18), i32::MAX);
}

#[test]
fn a_footer_takes_a_line_height_per_line() {
    let footer = |lines: &[&str]| CellFooter {
        lines: lines.iter().map(|line| (*line).to_owned()).collect(),
        ..CellFooter::default()
    };
    assert_eq!(footer(&[]).height(), FOOTER_HEIGHT);
    assert_eq!(footer(&["a"]).height(), FOOTER_HEIGHT);
    assert_eq!(footer(&["a", "b"]).height(), 2 * FOOTER_HEIGHT);
    // Folded in a view too low for a line: no height, nothing shown.
    let folded = CellFooter {
        folded: true,
        ..footer(&["a"])
    };
    assert_eq!(folded.height(), 0);
    assert!(folded.shown().is_none());
    assert!(footer(&["a"]).shown().is_some());
}

#[test]
fn the_footer_reads_like_the_reference() {
    let detected = [min(640), min(768), max(1023), max(1279)];
    let fields = all_fields();
    let footer = text(&input(768, &detected, &fields));
    assert_eq!(
        footer.text,
        "md | 768-1023px | 768 \u{d7} 1024 @2x | min-width 768px | max-width 1023px"
    );
    assert_eq!(
        footer.tooltip,
        "min-width 768px, max-width 1023px, min-width 640px, max-width 1279px"
    );
}

#[test]
fn a_field_switched_off_or_without_a_value_is_left_out() {
    let fields = BTreeSet::from([FooterField::Range, FooterField::Framework]);
    assert_eq!(text(&input(768, &[], &fields)).text, "768-1023px");
    assert_eq!(text(&input(768, &[], &fields)).tooltip, "");
    assert_eq!(text(&input(768, &[], &BTreeSet::new())).text, "");
}

#[test]
fn without_an_analysis_the_queries_name_the_ends_of_the_range() {
    // RB-080-47: `min-width` and `max-width` are always there, with their labels.
    let fields = BTreeSet::from([FooterField::Queries]);
    let footer = text(&input(768, &[], &fields));
    assert_eq!(footer.text, "min-width 768px | max-width 1023px");
    assert_eq!(footer.tooltip, "");
    // A free size: the ends of its neighbours' range.
    assert_eq!(
        text(&input(800, &[], &fields)).text,
        "min-width 768px | max-width 1023px"
    );
    // The smallest has no lower end, the largest no upper one.
    assert_eq!(text(&input(360, &[], &fields)).text, "max-width 767px");
    assert_eq!(text(&input(1440, &[], &fields)).text, "min-width 1440px");
    // One breakpoint alone stands for every width: nothing to name.
    let mut alone = input(768, &[], &fields);
    alone.config_widths = &[768];
    assert_eq!(text(&alone).text, "");
}

#[test]
fn a_page_without_a_query_that_holds_gets_the_step_its_boundaries_leave() {
    // `max-width 1023px` alone: from 1024 on the page's last step holds.
    let fields = BTreeSet::from([FooterField::Queries, FooterField::Range]);
    assert_eq!(
        text(&input(1440, &[max(1023)], &fields)).text,
        "1024-\u{221e}px | min-width 1024px"
    );
}

/// A real-world page (RB-080-78): UnoCSS, mobile-first, only `min-width`.
fn mobile_first() -> Vec<DetectedBreakpoint> {
    vec![
        boundary(640, BoundaryKind::Min, Some("xs")),
        boundary(768, BoundaryKind::Min, Some("sm")),
        boundary(1024, BoundaryKind::Min, Some("md")),
        boundary(1280, BoundaryKind::Min, Some("lg")),
        boundary(1920, BoundaryKind::Min, Some("xl")),
    ]
}

#[test]
fn a_mobile_first_page_names_the_upper_end_of_its_step() {
    // Before: `min-width 1024px | min-width 768px` - twice min-width, no max-width.
    let detected = mobile_first();
    let fields = BTreeSet::from([FooterField::Queries]);
    assert_eq!(
        text(&input(1024, &detected, &fields)).text,
        "min-width 1024px | max-width 1279px"
    );
    assert_eq!(
        text(&input(1100, &detected, &fields)).text,
        "min-width 1024px | max-width 1279px"
    );
    // Below the first step and above the last one, one end each.
    assert_eq!(
        text(&input(375, &detected, &fields)).text,
        "max-width 639px"
    );
    assert_eq!(
        text(&input(2560, &detected, &fields)).text,
        "min-width 1920px"
    );
}

#[test]
fn the_range_is_the_pages_step_when_the_page_is_known() {
    // The config's breakpoint `lg` is 1024: the page's step is the same here, its class `md`.
    let detected = mobile_first();
    let fields = BTreeSet::from([FooterField::Range, FooterField::Framework]);
    assert_eq!(
        text(&input(1024, &detected, &fields)).text,
        "1024-1279px | md"
    );
    // A width between two config breakpoints: the page's step, not the config's.
    assert_eq!(text(&input(700, &detected, &fields)).text, "640-767px | xs");
    // Without an analysis: the config's range, and no class.
    assert_eq!(text(&input(1024, &[], &fields)).text, "1024-1439px");
}

#[test]
fn a_desktop_first_page_has_its_step_too() {
    let detected = [max(767), max(1023), max(1279)];
    assert_eq!(
        page_range(800, &detected),
        Some(Range {
            from: 768,
            to: Some(1023)
        })
    );
    assert_eq!(
        page_range(500, &detected),
        Some(Range {
            from: 0,
            to: Some(767)
        })
    );
    assert_eq!(page_range(800, &[]), None);
    // Both kinds: the tightest of either side.
    assert_eq!(
        page_range(900, &[min(768), max(1023), min(640), max(1279)]),
        Some(Range {
            from: 768,
            to: Some(1023)
        })
    );
}

#[test]
fn the_tooltip_names_every_query_that_holds_beside_the_range() {
    let detected = mobile_first();
    let fields = BTreeSet::from([FooterField::Range, FooterField::Framework]);
    assert_eq!(
        text(&input(1024, &detected, &fields)).tooltip,
        "min-width 1024px, min-width 768px, min-width 640px"
    );
}

#[test]
fn the_queries_of_the_page_come_before_the_range_each_with_its_label() {
    // The owner's reference: `min-width 360px | max-width 1920px`.
    let fields = BTreeSet::from([FooterField::Queries]);
    let footer = text(&input(1280, &[min(360), max(1920), min(640)], &fields));
    assert_eq!(footer.text, "min-width 640px | max-width 1920px");
    assert_eq!(
        footer.tooltip,
        "min-width 640px, max-width 1920px, min-width 360px"
    );
}

#[test]
fn the_framework_name_is_not_repeated_beside_the_same_breakpoint_id() {
    let detected = [boundary(768, BoundaryKind::Min, Some("md"))];
    let fields = BTreeSet::from([FooterField::Breakpoint, FooterField::Framework]);
    assert_eq!(text(&input(768, &detected, &fields)).text, "md");
    let mut other = input(768, &detected, &fields);
    other.breakpoint = Some("tablet");
    assert_eq!(text(&other).text, "tablet | md");
}

#[test]
fn a_breakpoint_takes_its_own_colour_or_one_of_the_palette() {
    assert_eq!(background(Some("#22c55e"), 0), 0x0022_c55e);
    assert_eq!(background(None, 0), PALETTE[0]);
    assert_eq!(background(None, PALETTE.len() + 2), PALETTE[2]);
    // A colour that is none (a hand-edited config) falls back to the palette.
    assert_eq!(background(Some("green"), 1), PALETTE[1]);
}

#[test]
fn the_text_is_dark_on_light_and_light_on_dark() {
    assert_eq!(foreground(0x00ff_ffff), DARK_TEXT);
    assert_eq!(foreground(0x00fa_cc15), DARK_TEXT);
    assert_eq!(foreground(0x0000_0000), LIGHT_TEXT);
    assert_eq!(foreground(0x001e_3a8a), LIGHT_TEXT);
}

#[test]
fn the_text_reads_on_every_ground() {
    // WCAG AA, 4.5 : 1 - on the palette and on whatever colour a breakpoint names (RB-080-47).
    let steps = || (0..=0xff_u32).step_by(0x11);
    let grounds = steps()
        .flat_map(|r| steps().flat_map(move |g| steps().map(move |b| (r << 16) | (g << 8) | b)))
        .chain(PALETTE);
    for color in grounds {
        let contrast = contrast(color, foreground(color));
        assert!(contrast >= 4.5, "{color:06x}: {contrast}");
    }
}

#[test]
fn the_view_gives_the_footer_its_height() {
    let rect = CellRect {
        x: 10,
        y: 20,
        width: 300,
        height: 200,
    };
    assert_eq!(
        above_footer(rect, FOOTER_HEIGHT),
        CellRect {
            height: 182,
            ..rect
        }
    );
    assert_eq!(above_footer(rect, 0), rect);
    // A view lower than the footer gives all it has.
    let low = CellRect { height: 10, ..rect };
    assert_eq!(above_footer(low, FOOTER_HEIGHT).height, 0);
}

use serde_json::json;

use super::*;

/// `Page.getLayoutMetrics` of a view `width` CSS px wide on a screen at `ratio`.
fn measured(width: f64, ratio: f64) -> Value {
    json!({
        "contentSize": { "width": width * ratio, "height": 40_000.0 * ratio },
        "cssContentSize": { "width": width, "height": 40_000.0 },
        "layoutViewport": { "clientWidth": (width * ratio).round() },
        "cssLayoutViewport": { "clientWidth": width }
    })
}

/// Sections back to back from 0 to the plan's height, each within one texture and within the
/// pixels of one shot, their rows the image's.
fn assert_sound(plan: &FullPagePlan, width: f64, ratio: f64) {
    let cap = ratio.max(1.0);
    let mut y = 0.0;
    for section in &plan.sections {
        assert_eq!(section.y, y, "no gap, no overlap: {plan:?}");
        assert!(section.height >= 1.0);
        assert!(
            section.height * cap <= MAX_DEVICE_PX,
            "a section over the texture: {plan:?}"
        );
        assert!((width * cap).ceil() * section.height * cap <= MAX_SHOT_PIXELS);
        // Whole rows counted from the top: no section's rounding adds up.
        let rows = ((y + section.height) * ratio).round() - (y * ratio).round();
        assert_eq!(f64::from(section.rows), rows, "{section:?}");
        y += section.height;
    }
    assert_eq!(y, plan.height, "{plan:?}");
    assert_eq!(
        plan.viewport, plan.sections[0].height,
        "one viewport, one section's"
    );
    assert!(f64::from(plan.rows()) <= MAX_IMAGE_ROWS);
}

#[test]
fn the_screens_ratio_counts_when_the_breakpoint_has_none() {
    // The owner's views: no `dpr`, Windows at 150 %: 1920 CSS px are 2880 device px.
    assert_eq!(device_ratio(&measured(1920.0, 1.5), 0.0), 1.5);
    assert_eq!(device_ratio(&measured(375.0, 1.5), 0.0), 1.5);
    // Nothing measured: the breakpoint's ratio, else 1.
    assert_eq!(device_ratio(&json!({}), 3.0), 3.0);
    assert_eq!(device_ratio(&json!({}), 0.0), 1.0);
    assert_eq!(device_ratio(&json!({}), f64::NAN), 1.0);
    // An empty page measures nothing.
    let empty = json!({
        "contentSize": { "width": 0.0, "height": 0.0 },
        "cssContentSize": { "width": 0.0, "height": 0.0 }
    });
    assert_eq!(device_ratio(&empty, 2.0), 2.0);
}

#[test]
fn a_measured_ratio_is_rounded_to_what_it_is_meant_to_be() {
    // Device pixels are whole: 393 x 2.625 = 1031.6 measures as 1031 or 1032.
    let rounded = json!({
        "contentSize": { "width": 1032.0 }, "cssContentSize": { "width": 393.0 }
    });
    assert_eq!(device_ratio(&rounded, 2.625), 2.625);
    let screen = json!({
        "contentSize": { "width": 2879.0 }, "cssContentSize": { "width": 1920.0 }
    });
    assert_eq!(device_ratio(&screen, 0.0), 1.5);
    // The browser's zoom on top of the breakpoint's ratio: 1.5 x 110 %.
    let zoomed = json!({
        "contentSize": { "width": 3168.0 }, "cssContentSize": { "width": 1920.0 }
    });
    assert!((device_ratio(&zoomed, 1.5) - 1.65).abs() < 1e-9);
    // A measurement out of all reason is not taken.
    let absurd = json!({
        "contentSize": { "width": 1e9 }, "cssContentSize": { "width": 1.0 }
    });
    assert_eq!(device_ratio(&absurd, 0.0), 1.0);
}

#[test]
fn a_page_within_one_texture_is_one_shot_at_the_pages_height() {
    let plan = plan(4000.5, 852.0, 393.0, 3.0, 1.0).expect("a plan");
    assert_eq!(plan.height, 4001.0);
    assert_eq!(plan.viewport, 4001.0, "the viewport as tall as the page");
    assert_eq!(plan.sections.len(), 1);
    assert_eq!(plan.rows(), 12_003);
    assert!(!plan.truncated);
    // A page shorter than the viewport is the viewport.
    let short = plan_of(100.0, 852.0, 393.0, 3.0);
    assert_eq!(short.height, 852.0);
}

fn plan_of(content: f64, viewport: f64, width: f64, ratio: f64) -> FullPagePlan {
    plan(content, viewport, width, ratio, 1.0).expect("a plan")
}

#[test]
fn every_section_stays_within_one_texture_at_ratio_0_1_1_5_and_3() {
    // `dpr` 0 is the screen's ratio, measured: 1.5 here (the owner's screen).
    for dpr in [0.0, 1.0, 1.5, 3.0] {
        let screen = if dpr > 0.0 { dpr } else { 1.5 };
        let ratio = device_ratio(&measured(375.0, screen), dpr);
        assert_eq!(ratio, screen);
        let plan = plan_of(30_000.0, 812.0, 375.0, ratio);
        assert_sound(&plan, 375.0, ratio);
        assert!(plan.sections.len() > 1, "dpr {dpr}: {plan:?}");
        for section in &plan.sections {
            assert!(f64::from(section.rows) <= MAX_DEVICE_PX, "dpr {dpr}");
        }
    }
}

#[test]
fn the_owners_long_page_is_four_sections_and_ends_where_the_page_ends() {
    let plan = plan_of(40_000.0, 1080.0, 1920.0, 1.5);
    assert_sound(&plan, 1920.0, 1.5);
    assert!(!plan.truncated);
    assert_eq!(plan.height, 40_000.0);
    let heights: Vec<f64> = plan.sections.iter().map(|s| s.height).collect();
    assert_eq!(heights, [10_666.0, 10_666.0, 10_666.0, 8002.0]);
    let rows: Vec<u32> = plan.sections.iter().map(|s| s.rows).collect();
    assert_eq!(rows, [15_999, 15_999, 15_999, 12_003]);
    assert_eq!(plan.rows(), 60_000);
    // One viewport for every section: a section's height, never the whole page's.
    assert_eq!(plan.viewport, 10_666.0);
}

#[test]
fn a_fractional_ratio_puts_every_seam_on_a_whole_row() {
    // At 1.5 an odd CSS height is half a row: the page is rounded up to whole rows.
    let plan = plan_of(4001.0, 852.0, 393.0, 1.5);
    assert_eq!(plan.height, 4002.0);
    assert_eq!(plan.rows(), 6003);
    for ratio in [1.25, 1.5, 1.75, 2.625] {
        let plan = plan_of(50_001.0, 852.0, 393.0, ratio);
        assert_sound(&plan, 393.0, ratio);
        for section in &plan.sections {
            let rows = section.height * ratio;
            assert_eq!(rows, rows.round(), "ratio {ratio}: {section:?}");
            let top = section.y * ratio;
            assert_eq!(top, top.round(), "ratio {ratio}: {section:?}");
        }
    }
}

#[test]
fn a_page_beyond_the_image_is_cut_and_says_so() {
    // The height: 65535 rows at most.
    let plan = plan_of(100_000.0, 812.0, 375.0, 1.5);
    assert_sound(&plan, 375.0, 1.5);
    assert!(plan.truncated);
    assert_eq!(plan.page, 100_000.0);
    assert_eq!(plan.height, 43_690.0);
    assert_eq!(plan.rows(), 65_535);
    // The area: 256 megapixels at most, 64 per section.
    let wide = plan_of(20_000.0, 2160.0, 3840.0, 3.0);
    assert_sound(&wide, 3840.0, 3.0);
    assert!(wide.truncated);
    let pixels = f64::from(wide.rows()) * 11_520.0;
    assert!(pixels <= MAX_IMAGE_PIXELS, "{pixels}");
}

#[test]
fn a_ratio_below_1_or_none_never_raises_the_limits() {
    // Security review, wave 090-w23: a checked-in `dpr: 0.001` made the cap 16.4 million.
    for ratio in [0.001, 0.5, f64::NAN, 0.0, -2.0] {
        let plan = plan_of(3e7, 852.0, 393.0, ratio);
        assert!(plan.truncated, "{ratio}");
        assert!(plan.height <= MAX_IMAGE_ROWS, "{ratio}: {}", plan.height);
        for section in &plan.sections {
            assert!(section.height <= SECTION_DEVICE_PX, "{ratio}");
        }
    }
}

#[test]
fn a_view_too_wide_for_one_row_has_no_plan() {
    assert_eq!(plan(5000.0, 852.0, 393.0, 1e6, 1.0), None);
    assert_eq!(plan(5000.0, 852.0, f64::INFINITY, 1.0, 1.0), None);
}

#[test]
fn a_shot_scaled_down_to_css_px_keeps_each_section_within_a_texture() {
    // The overview's scale 1 / dpr: painted at the ratio first, then scaled down.
    let plan = plan(40_000.0, 852.0, 393.0, 3.0, 1.0 / 3.0).expect("a plan");
    for section in &plan.sections {
        assert!(section.height * 3.0 <= MAX_DEVICE_PX, "{section:?}");
    }
    assert_eq!(plan.height, 40_000.0);
    assert_eq!(plan.rows(), 40_000, "one row per CSS px");
}

#[test]
fn rows_rounded_per_section_never_add_up_past_the_image() {
    // Code review, wave 099-w36: at 1.0937 (no whole-row step up to 32 CSS px) four sections of
    // 14629 px rounded to 16000 rows each and the last to 1536 - 65536 rows, one over the image.
    let plan = plan_of(1e6, 852.0, 393.0, 1.0937);
    assert!(plan.truncated);
    assert!(f64::from(plan.rows()) <= MAX_IMAGE_ROWS, "{}", plan.rows());
    assert_sound(&plan, 393.0, 1.0937);
}

#[test]
fn every_ratio_keeps_the_image_and_each_section_within_their_limits() {
    // A sweep over ratios with and without a whole-row step, narrow and wide views, pages that
    // fit and pages that are cut.
    let mut ratio = 1.0;
    while ratio <= 4.0 {
        for width in [375.0, 1920.0, 3840.0] {
            for content in [9_999.0, 47_123.0, 1e6] {
                let plan = plan_of(content, 852.0, width, ratio);
                let row = (width * ratio).ceil();
                let rows = f64::from(plan.rows());
                assert!(rows <= MAX_IMAGE_ROWS, "{ratio} {width} {content}: {rows}");
                assert!(rows * row <= MAX_IMAGE_PIXELS, "{ratio} {width} {content}");
                assert_eq!(rows, (plan.height * ratio).round(), "{ratio} {width}");
                for section in &plan.sections {
                    let rows = f64::from(section.rows);
                    assert!(rows <= MAX_DEVICE_PX, "{ratio} {width}: {section:?}");
                    assert!(
                        rows * row <= MAX_SHOT_PIXELS,
                        "{ratio} {width}: {section:?}"
                    );
                }
                assert_sound(&plan, width, ratio);
            }
        }
        ratio += 0.0137;
    }
}

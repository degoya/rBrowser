use serde_json::json;

use super::*;

#[test]
fn the_target_showing_a_marker_is_found_by_its_whole_url() {
    let targets = json!({ "targetInfos": [
        { "targetId": "GRID", "type": "page", "url": "about:blank#rb-view-1" },
        { "targetId": "R10", "type": "page", "url": "about:blank#rb-render-10" },
        { "targetId": "R1", "type": "page", "url": "about:blank#rb-render-1" },
    ] });
    assert_eq!(
        target_showing(&targets, "about:blank#rb-render-1", None).as_deref(),
        Some("R1")
    );
    assert_eq!(
        target_showing(&targets, "about:blank#rb-render-2", None),
        None
    );
    assert_eq!(
        target_showing(&json!({}), "about:blank#rb-render-1", None),
        None
    );
}

#[test]
fn a_popup_or_another_context_showing_the_marker_is_never_taken() {
    // A grid page that opened a window at the marker (security review, wave 090-w21).
    let marker = "about:blank#rb-render-1-00ff";
    let targets = json!({ "targetInfos": [
        { "targetId": "P1", "url": marker, "openerId": "GRID", "browserContextId": "CTX" },
        { "targetId": "X1", "url": marker, "browserContextId": "OTHER" },
        { "targetId": "N1", "url": marker },
        { "targetId": "R1", "url": marker, "openerId": "", "browserContextId": "CTX" },
    ] });
    assert_eq!(
        target_showing(&targets, marker, Some("CTX")).as_deref(),
        Some("R1")
    );
    // Without a context to hold it to, still never a popup.
    assert_eq!(
        target_showing(&targets, marker, None).as_deref(),
        Some("X1")
    );
}

#[test]
fn markers_are_random_and_unique_in_the_process_whichever_backend_asks() {
    let first = next_marker().expect("random");
    let second = next_marker().expect("random");
    assert!(first.starts_with(MARKER_PREFIX), "{first}");
    assert_ne!(first, second);
    // The number, then 128 random bits: a page cannot guess a render view's address.
    let (_, random) = first
        .strip_prefix(MARKER_PREFIX)
        .and_then(|rest| rest.split_once('-'))
        .expect("number and nonce");
    assert_eq!(random.len(), 32);
    assert!(random.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_ne!(first.rsplit('-').next(), second.rsplit('-').next());
}

#[test]
fn whether_a_target_is_still_listed() {
    let targets = json!({ "targetInfos": [{ "targetId": "R1", "url": "https://site.test/" }] });
    assert!(lists(&targets, "R1"));
    assert!(!lists(&targets, "R2"));
}

#[test]
fn a_render_views_size_is_its_breakpoints_viewport_never_empty() {
    let view = RenderView::new("about:blank#rb-render-1", "default", 0, 667, 0.0);
    assert_eq!((view.width, view.height), (1, 667));
    let view = RenderView::new("about:blank#rb-render-2", "staging", 1280, 800, 1.0);
    assert_eq!(view.session, "staging");
    assert_eq!((view.width, view.height), (1280, 800));
}

#[test]
fn a_render_views_scale_is_its_breakpoints_pixel_ratio_at_least_one() {
    let scale =
        |dpr: f64| RenderView::new("about:blank#rb-render-1", "default", 744, 1133, dpr).scale;
    // The iPad mini's surface is painted at 2, as its emulation asks (RB-090-90).
    assert_eq!(scale(2.0), 2.0);
    assert_eq!(scale(1.5), 1.5);
    // None, below 1 or nonsense: 1; above the breakpoints' highest ratio: that.
    assert_eq!(scale(0.0), 1.0);
    assert_eq!(scale(0.5), 1.0);
    assert_eq!(scale(f64::NAN), 1.0);
    assert_eq!(scale(f64::INFINITY), 1.0);
    assert_eq!(scale(40.0), MAX_SCALE);
}

/// What a recording `painted` callback was called with.
type Calls = Arc<std::sync::Mutex<Vec<Result<(), String>>>>;

/// A `painted` callback that records what it was called with.
fn recorder() -> (Calls, RenderViewMade) {
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = calls.clone();
    let painted: RenderViewMade = Box::new(move |result| {
        crate::cdp::lock(&sink).push(result);
    });
    (calls, painted)
}

#[test]
fn a_view_size_is_the_breakpoints_until_a_resize() {
    let size = ViewSize::new(375, 0);
    assert_eq!(size.get(), (375, 1), "never 0");
    let (_, painted) = recorder();
    size.resize(375, 4001, painted);
    assert_eq!(size.get(), (375, 4001));
}

#[test]
fn the_first_frame_as_tall_as_the_new_size_answers_the_resize() {
    // RB-090-88: the full-page shot waits for a frame of the grown view, not one painted before.
    let size = ViewSize::new(375, 667);
    let (calls, painted) = recorder();
    size.resize(375, 4001, painted);
    size.painted(375, 667);
    assert!(
        crate::cdp::lock(&calls).is_empty(),
        "a frame of the old size"
    );
    size.painted(375, 4001);
    assert_eq!(*crate::cdp::lock(&calls), [Ok(())]);
    size.painted(375, 4001);
    assert_eq!(crate::cdp::lock(&calls).len(), 1, "answered once");
}

#[test]
fn a_frame_in_device_pixels_is_measured_in_dip() {
    // At a scale factor of 1.5 a frame of the old size (800 DIP, 1200 px) is taller in pixels
    // than the new size in DIP (1100): it must not answer the resize (code review, wave 090-w23).
    let size = ViewSize::new(375, 800);
    let (calls, painted) = recorder();
    size.resize(375, 1100, painted);
    size.painted(563, 1200);
    assert!(crate::cdp::lock(&calls).is_empty(), "the old size at 1.5");
    // 1100 DIP at 1.5, both sides rounded to whole pixels.
    size.painted(563, 1650);
    assert_eq!(*crate::cdp::lock(&calls), [Ok(())]);
    // At 2 and 3 alike.
    for (scale, old) in [(2, 1600), (3, 2400)] {
        let (calls, painted) = recorder();
        size.resize(375, 1100, painted);
        size.painted(375 * scale, old);
        assert!(
            crate::cdp::lock(&calls).is_empty(),
            "the old size at {scale}"
        );
        size.painted(375 * scale, 1100 * scale);
        assert_eq!(*crate::cdp::lock(&calls), [Ok(())], "at {scale}");
    }
}

#[test]
fn a_frame_without_width_answers_nothing() {
    let size = ViewSize::new(375, 667);
    let (calls, painted) = recorder();
    size.resize(375, 4001, painted);
    size.painted(0, 100_000);
    assert!(crate::cdp::lock(&calls).is_empty());
}

#[test]
fn a_resize_before_the_last_one_painted_answers_that_one() {
    let size = ViewSize::new(375, 667);
    let (first, painted) = recorder();
    size.resize(375, 4001, painted);
    let (second, painted) = recorder();
    size.resize(375, 667, painted);
    assert_eq!(crate::cdp::lock(&first).len(), 1);
    assert!(crate::cdp::lock(&first)[0].is_err());
    size.painted(375, 667);
    assert_eq!(*crate::cdp::lock(&second), [Ok(())]);
}

#[test]
fn a_closed_view_answers_its_waiting_resize() {
    let size = ViewSize::new(375, 667);
    let (calls, painted) = recorder();
    size.resize(375, 4001, painted);
    size.closed();
    assert_eq!(crate::cdp::lock(&calls).len(), 1);
    assert!(crate::cdp::lock(&calls)[0].is_err());
}

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
    let view = RenderView::new("about:blank#rb-render-1", "default", 0, 667);
    assert_eq!((view.width, view.height), (1, 667));
    let view = RenderView::new("about:blank#rb-render-2", "staging", 1280, 800);
    assert_eq!(view.session, "staging");
    assert_eq!((view.width, view.height), (1280, 800));
}

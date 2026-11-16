use serde_json::json;

use super::*;

#[test]
fn devtools_pages_and_the_dock_marker_are_hidden_by_their_url() {
    assert!(is_hidden(
        "A",
        "devtools://devtools/bundled/inspector.html?ws=127.0.0.1:1/devtools/x"
    ));
    assert!(is_hidden("B", "DevTools://devtools/devtools_app.html"));
    assert!(is_hidden("C", DOCK_MARKER_URL));
    assert!(!is_hidden("D", "https://example.com/devtools://"));
    assert!(!is_hidden("E", "about:blank#rb-view-1"));
}

#[test]
fn a_registered_target_is_hidden_whatever_it_shows() {
    assert!(!is_hidden("hidden-test-dock", "about:blank"));
    register("hidden-test-dock");
    assert!(is_hidden("hidden-test-dock", "about:blank"));
    assert!(is_hidden_target(
        &json!({ "targetId": "hidden-test-dock", "type": "page", "url": "https://a.test/" })
    ));
    assert!(!is_hidden_target(
        &json!({ "targetId": "hidden-test-page", "type": "page", "url": "https://a.test/" })
    ));
}

#[test]
fn the_shell_the_settings_window_and_the_analysers_target_are_hidden() {
    assert!(is_hidden("S", crate::shell::SHELL_URL));
    assert!(is_hidden("P", crate::shell::PAGES_URL));
    assert!(is_hidden("A", ANALYZER_MARKER_URL));
    assert!(!is_hidden("V", "https://site.test/"));
}

#[test]
fn a_forgotten_target_is_judged_by_its_url_again() {
    register("hidden-test-closed");
    forget("hidden-test-closed");
    assert!(!is_hidden("hidden-test-closed", "https://a.test/"));
    assert!(filter().is_hidden("X", "rbrowser://ui/index.html"));
}

/// F4 (RB-050-05): a sitemap render view is hidden by its marker URL from its creation on, so a
/// CDP proxy client never sees it between `createTarget` and the registration of its id.
#[test]
fn a_render_view_is_hidden_by_its_marker_before_its_id_is_known() {
    let marker = format!("{}3", crate::sitemap::backend::MARKER_PREFIX);
    assert!(is_hidden("hidden-test-render", &marker));
    assert!(is_hidden_target(
        &json!({ "targetId": "hidden-test-render", "type": "page", "url": marker })
    ));
}

/// An extension's service worker, background or options page is never an outer client's
/// target (RB-080-06): Playwright sees the views only.
#[test]
fn chrome_extension_targets_are_hidden_by_their_url() {
    assert!(is_hidden(
        "X1",
        "chrome-extension://nhdogjmejiglipccpnnnanhbledajbpd/service-worker.js"
    ));
    assert!(is_hidden_target(&json!({
        "targetId": "X2",
        "type": "service_worker",
        "url": "chrome-extension://abcdefghijklmnopabcdefghijklmnop/background.js"
    })));
    assert!(!is_hidden("X3", "https://example.com/chrome-extension://x"));
}

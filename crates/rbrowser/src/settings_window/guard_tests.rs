use super::*;

const KNOWN: KnownBrowsers = KnownBrowsers {
    shell: 1,
    pages: 7,
    palette: 11,
    panels: 13,
    panels_dock: 21,
    history: 15,
    status_bar: 17,
    board: 19,
};

fn from(browser: i32, main_frame: bool, url: &str) -> bool {
    query_allowed(
        QueryOrigin {
            browser,
            main_frame,
            url,
        },
        KNOWN,
    )
}

#[test]
fn the_shell_and_the_pages_window_may_query_from_their_main_frame() {
    assert!(from(1, true, "rbrowser://ui/index.html"));
    assert!(from(
        7,
        true,
        "rbrowser://ui/settings.html#/settings/sync.scroll"
    ));
    assert!(from(7, true, "RBROWSER://UI/settings.html"));
}

#[test]
fn any_other_browser_is_refused() {
    assert!(!from(2, true, "rbrowser://ui/index.html"));
    assert!(!from(0, true, "rbrowser://ui/settings.html"));
}

#[test]
fn a_sub_frame_is_refused() {
    assert!(!from(1, false, "rbrowser://ui/index.html"));
    assert!(!from(7, false, "rbrowser://ui/settings.html"));
}

#[test]
fn a_page_outside_the_shell_origin_is_refused() {
    assert!(!from(7, true, "https://example.test/"));
    assert!(!from(7, true, "rbrowser://settings/sync.scroll"));
    assert!(!from(1, true, "rbrowser://uix/index.html"));
    assert!(!from(7, true, ""));
}

#[test]
fn browser_id_zero_never_matches_a_missing_window() {
    let none = KnownBrowsers::default();
    assert!(!query_allowed(
        QueryOrigin {
            browser: 0,
            main_frame: true,
            url: "rbrowser://ui/settings.html",
        },
        none,
    ));
}

#[test]
fn the_palette_queries_under_the_same_checks() {
    assert!(from(11, true, "rbrowser://ui/settings.html#/palette"));
    assert!(!from(11, false, "rbrowser://ui/settings.html#/palette"));
    assert!(!from(11, true, "https://example.test/"));
    // The findings window is gone (RB-080-69): its old id is no browser that may query.
    assert!(!from(
        9,
        true,
        "rbrowser://ui/settings.html#/panels/findings"
    ));
}

#[test]
fn the_panels_window_may_query_but_never_a_plugin_panel_in_it() {
    assert!(from(13, true, "rbrowser://ui/settings.html#/panels"));
    assert!(!from(
        13,
        false,
        "rbrowser://plugin/hello-panel/panel/index.html"
    ));
    assert!(!from(
        13,
        true,
        "rbrowser://plugin/hello-panel/panel/index.html"
    ));
}

#[test]
fn the_status_bar_and_its_history_query_under_the_same_checks() {
    assert!(from(17, true, "rbrowser://ui/status-bar.html"));
    assert!(from(
        15,
        true,
        "rbrowser://ui/settings.html#/status-history"
    ));
    assert!(!from(17, false, "rbrowser://ui/status-bar.html"));
    assert!(!from(17, true, "https://example.test/"));
}

const BOARD_PAGE: &str = "rbrowser://ui/canvas-board.html";

fn board_sends(request: &str) -> bool {
    request_allowed(
        QueryOrigin {
            browser: 19,
            main_frame: true,
            url: BOARD_PAGE,
        },
        KNOWN,
        request,
    )
}

#[test]
fn the_canvas_board_sends_only_its_own_requests() {
    assert!(from(19, true, BOARD_PAGE));
    assert!(board_sends(r#"{ "type": "getState" }"#));
    assert!(board_sends(r#"{ "type": "getSettings" }"#));
    // RB-090-26: its labels follow Windows' text size like every page of the shell.
    assert!(board_sends(r#"{ "type": "getTextScale" }"#));
    assert!(board_sends(r#"{ "type": "setActiveView", "view": 2 }"#));
    assert!(board_sends(
        r#"{ "type": "canvasDrag", "view": 2, "dx": 4, "dy": 0, "phase": "move" }"#
    ));
    assert!(board_sends(
        r#"{ "type": "canvasPan", "dx": 4, "dy": 0, "phase": "cancel" }"#
    ));
    assert!(board_sends(
        r#"{ "type": "canvasStep", "view": 2, "dx": 1, "dy": 0 }"#
    ));
    assert!(board_sends(r#"{ "type": "canvasSizePick", "view": 2 }"#));
    assert!(board_sends(r#"{ "type": "canvasEnginePick", "view": 2 }"#));
    assert!(!board_sends(
        r#"{ "type": "getSettings", "includeSchema": true }"#
    ));
    assert!(!board_sends(
        r#"{ "type": "runCommand", "id": "app.quit" }"#
    ));
    // RB-080-46: the board's menus are the core's native ones - the board names the place and
    // runs no command, the view commands of RB-080-41 included.
    assert!(board_sends(
        r#"{ "type": "canvasMenu", "view": 2, "x": 10, "y": 4 }"#
    ));
    assert!(board_sends(r#"{ "type": "canvasMenu", "x": 10, "y": 4 }"#));
    assert!(board_sends(
        r#"{ "type": "canvasZoom", "x": 10, "y": 4, "delta": -100, "phase": "end" }"#
    ));
    // RB-090-55: the board says which canvas it painted; the views follow.
    assert!(board_sends(r#"{ "type": "canvasPainted", "seq": 3 }"#));
    assert!(!board_sends(
        r#"{ "type": "addViewMenu", "x": 10, "y": 4 }"#
    ));
    for refused in [
        r#"{ "type": "runCommand", "id": "view.remove", "args": { "view": 2 } }"#,
        r#"{ "type": "runCommand", "id": "view.add", "args": { "breakpoint": "all" } }"#,
        r#"{ "type": "runCommand", "id": "layout.zoom_to_fit" }"#,
        r#"{ "type": "runCommand", "id": "view.navigate" }"#,
        r#"{ "type": "runCommand", "id": "settings.open" }"#,
    ] {
        assert!(!board_sends(refused), "{refused}");
    }
    assert!(board_sends(
        r#"{ "type": "canvasSizePick", "view": 2, "force": true }"#
    ));
    assert!(!board_sends(
        r#"{ "type": "setSetting", "key": "mcp.enabled", "value": true, "scope": "user" }"#
    ));
    assert!(!board_sends(
        r#"{ "type": "navigate", "url": "https://example.test/" }"#
    ));
    assert!(!board_sends("not json"));
}

#[test]
fn the_board_is_refused_off_its_page_and_the_shell_is_not_narrowed() {
    assert!(!from(19, false, BOARD_PAGE));
    assert!(!from(19, true, "https://example.test/"));
    let shell = QueryOrigin {
        browser: 1,
        main_frame: true,
        url: "rbrowser://ui/index.html",
    };
    assert!(request_allowed(
        shell,
        KNOWN,
        r#"{ "type": "runCommand", "id": "app.quit" }"#
    ));
    assert!(request_allowed(
        shell,
        KNOWN,
        r#"{ "type": "canvasStep", "view": 2, "dx": 1, "dy": 0 }"#
    ));
}

#[test]
fn the_docked_panels_query_like_the_panels_window_and_no_more() {
    // RB-080-57: the same page in the main window, the same checks.
    assert!(from(
        21,
        true,
        "rbrowser://ui/settings.html#/panels/hello-panel.main"
    ));
    assert!(!from(
        21,
        false,
        "rbrowser://plugin/hello-panel/panel/index.html"
    ));
    assert!(!from(
        21,
        true,
        "rbrowser://plugin/hello-panel/panel/index.html"
    ));
    assert!(!from(21, true, "https://example.test/"));
    // Not docked: its id is 0 and matches nothing.
    let undocked = KnownBrowsers {
        panels_dock: 0,
        ..KNOWN
    };
    assert!(!query_allowed(
        QueryOrigin {
            browser: 21,
            main_frame: true,
            url: "rbrowser://ui/settings.html#/panels",
        },
        undocked,
    ));
}

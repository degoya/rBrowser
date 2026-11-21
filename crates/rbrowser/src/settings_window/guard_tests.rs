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
    gallery: 23,
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
    // RB-090-69: a click on a label's piece; the core looks up what it does.
    assert!(board_sends(
        r#"{ "type": "canvasPiece", "view": 2, "piece": "engineStatus" }"#
    ));
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
    // RB-092-03: the address field of a decoupled view's label; the command checks the view and
    // the address itself.
    assert!(board_sends(
        r#"{ "type": "runCommand", "id": "view.edit_address", "args": { "view": 2, "url": "/a" } }"#
    ));
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

fn sends_from(browser: i32, request: &str) -> bool {
    request_allowed(
        QueryOrigin {
            browser,
            main_frame: true,
            url: "rbrowser://ui/settings.html#/settings/plugins",
        },
        KNOWN,
        request,
    )
}

#[test]
fn only_the_pages_window_sends_the_plugin_catalogues_requests() {
    // RB-091-01, security review L3: downloading, installing and removing plugins belongs to
    // Settings > Plugins > Catalog - and, RB-091-04, to the first-run wizard of the same window.
    for op in [
        "catalog",
        "catalogPrepare",
        "catalogInstall",
        "catalogRemove",
        "available",
        "installChosen",
        "removeChosen",
        "applyRecommended",
    ] {
        let request = format!(r#"{{"type":"pluginPanel","request":{{"op":"{op}"}}}}"#);
        assert!(is_catalogue_request(&request), "{op}");
        assert!(
            sends_from(KNOWN.pages, &request),
            "{op} from the pages window"
        );
        for other in [KNOWN.shell, KNOWN.palette, KNOWN.panels, KNOWN.panels_dock] {
            assert!(!sends_from(other, &request), "{op} from browser {other}");
        }
    }
    let installed = r#"{"type":"pluginPanel","request":{"op":"installed"}}"#;
    assert!(!is_catalogue_request(installed));
    assert!(sends_from(KNOWN.panels, installed));
}

#[test]
fn only_the_pages_window_sends_the_setup_wizards_requests() {
    // RB-091-05: the wizard writes project configs; the shell, the palette, the panels and the
    // status bar never send its requests.
    for op in [
        r#"{"op":"info"}"#,
        r#"{"op":"write","requestId":1,"proposalId":1}"#,
        r#"{"op":"close"}"#,
    ] {
        let request = format!(r#"{{"type":"setup","request":{op}}}"#);
        assert!(
            sends_from(KNOWN.pages, &request),
            "{op} from the pages window"
        );
        for other in [
            KNOWN.shell,
            KNOWN.palette,
            KNOWN.panels,
            KNOWN.panels_dock,
            KNOWN.status_bar,
        ] {
            assert!(!sends_from(other, &request), "{op} from browser {other}");
        }
    }
}

#[test]
fn only_the_pages_window_sends_the_mcp_setups_requests() {
    // RB-091-09: the MCP setup writes client files; nothing but the wizard and the settings in
    // the pages window sends its requests.
    for op in [
        r#"{"op":"plan","requestId":1}"#,
        r#"{"op":"write","requestId":2,"client":"cursor","project":true,"user":true}"#,
        r#"{"op":"check","requestId":3}"#,
    ] {
        let request = format!(r#"{{"type":"mcpSetup","request":{op}}}"#);
        assert!(
            sends_from(KNOWN.pages, &request),
            "{op} from the pages window"
        );
        for other in [
            KNOWN.shell,
            KNOWN.palette,
            KNOWN.panels,
            KNOWN.panels_dock,
            KNOWN.status_bar,
            KNOWN.history,
            KNOWN.board,
            KNOWN.gallery,
        ] {
            assert!(!sends_from(other, &request), "{op} from browser {other}");
        }
    }
}

#[test]
fn the_gallery_window_reads_its_run_and_opens_links_and_nothing_else() {
    // RB-090-86: the sitemap gallery page reads the theme, the text size and its run, and opens
    // a page's address through `app.open_link`; it writes nothing and runs no other command.
    for request in [
        r#"{"type":"getState"}"#,
        r#"{"type":"getSettings"}"#,
        r#"{"type":"getTextScale"}"#,
        r#"{"type":"getSitemapGallery","runId":"2026-11-11_10-00-00"}"#,
        r#"{"type":"getSitemapGallery"}"#,
        r#"{"type":"runCommand","id":"app.open_link","args":{"url":"http://localhost:3000/"}}"#,
    ] {
        assert!(gallery_may_send(request), "{request}");
        assert!(sends_from(KNOWN.gallery, request), "{request}");
    }
    for request in [
        r#"{"type":"getSettings","includeSchema":true}"#,
        r#"{"type":"setSetting","key":"ui.theme","value":"dark","scope":"user"}"#,
        r#"{"type":"runCommand","id":"sitemap.capture","args":{}}"#,
        r#"{"type":"runCommand","id":"app.open_link_x","args":{}}"#,
        r#"{"type":"navigate","url":"http://localhost:3000/"}"#,
        r#"{"type":"pickPath","kind":"file"}"#,
        r#"{"type":"pluginPanel","request":{"op":"installed"}}"#,
        r#"{"type":"listCommands"}"#,
        r#"{"type":"getTrust"}"#,
        r#"{"type":"setup","request":{"op":"info"}}"#,
        "not json",
    ] {
        assert!(!gallery_may_send(request), "{request}");
        assert!(!sends_from(KNOWN.gallery, request), "{request}");
    }
    // The others are not narrowed by it.
    assert!(sends_from(KNOWN.pages, r#"{"type":"listCommands"}"#));
    assert!(sends_from(KNOWN.pages, r#"{"type":"getSitemapGallery"}"#));
}

#[test]
fn the_gallery_window_queries_from_its_main_frame_on_the_shells_pages_only() {
    let url = "rbrowser://ui/settings.html#/sitemap-gallery?run=2026-11-11_10-00-00";
    assert!(from(23, true, url));
    assert!(!from(23, false, url));
    assert!(!from(
        23,
        true,
        "file:///C:/work/.rbrowser/out/sitemap-capture/r1/index.html"
    ));
    let closed = KnownBrowsers {
        gallery: 0,
        ..KNOWN
    };
    assert!(!query_allowed(
        QueryOrigin {
            browser: 23,
            main_frame: true,
            url,
        },
        closed,
    ));
}

// Security review w27: the board's one command is checked for its shape at the guard - a view id
// and, optionally, the typed address as a bounded string; nothing else reaches the command.
#[test]
fn the_board_edits_an_address_only_with_a_view_and_a_bounded_url() {
    let edit = |args: &str| {
        board_sends(&format!(
            r#"{{ "type": "runCommand", "id": "view.edit_address", "args": {args} }}"#
        ))
    };
    let longest = "a".repeat(crate::open_link::address::MAX_URL_CHARS);
    assert!(edit(r#"{ "view": 2, "url": "/a" }"#));
    assert!(edit(r#"{ "view": 2 }"#));
    assert!(edit(&format!(
        r#"{{ "view": 4294967295, "url": "{longest}" }}"#
    )));
    let too_long = format!(r#"{{ "view": 2, "url": "{longest}a" }}"#);
    for refused in [
        "null",
        "[]",
        r#""view""#,
        "{}",
        r#"{ "url": "/a" }"#,
        r#"{ "view": "two", "url": "/a" }"#,
        r#"{ "view": -1 }"#,
        r#"{ "view": 2.5 }"#,
        r#"{ "view": 4294967296 }"#,
        r#"{ "view": null, "url": "/a" }"#,
        r#"{ "view": 2, "url": null }"#,
        r#"{ "view": 2, "url": 7 }"#,
        r#"{ "view": 2, "url": ["/a"] }"#,
        r#"{ "view": 2, "url": "/a", "target": "all" }"#,
        too_long.as_str(),
    ] {
        assert!(!edit(refused), "{refused}");
    }
    assert!(!board_sends(
        r#"{ "type": "runCommand", "id": "view.edit_address" }"#
    ));
}

// The canvas zoom badge's click (RB-090-28) sets one view's page zoom back: only that command,
// only `{ target: <view id> }`.
#[test]
fn the_board_resets_one_views_page_zoom_and_nothing_more() {
    let reset = |args: &str| {
        board_sends(&format!(
            r#"{{ "type": "runCommand", "id": "view.page_zoom.reset", "args": {args} }}"#
        ))
    };
    assert!(reset(r#"{ "target": 3 }"#));
    for refused in [
        "{}",
        "null",
        r#"{ "target": "all" }"#,
        r#"{ "target": -1 }"#,
        r#"{ "target": 3, "zoom": 2 }"#,
    ] {
        assert!(!reset(refused), "{refused}");
    }
    assert!(!board_sends(
        r#"{ "type": "runCommand", "id": "view.page_zoom.set", "args": { "target": 3 } }"#
    ));
}

#[test]
fn only_the_pages_window_decides_on_a_bug_reports_preview() {
    // RB-093-04: Save or Cancel of the preview is the person's in the pages window; the status
    // bar may still ask for a saved report's Markdown (its copy button), the board and the
    // gallery for nothing of it.
    let decide = r#"{"type":"bugReport","request":{"op":"decide","previewId":1,"save":true}}"#;
    let markdown = r#"{"type":"bugReport","request":{"op":"getMarkdown","report":"r"}}"#;
    assert!(sends_from(KNOWN.pages, decide));
    for browser in [
        KNOWN.shell,
        KNOWN.palette,
        KNOWN.panels,
        KNOWN.panels_dock,
        KNOWN.history,
        KNOWN.status_bar,
        KNOWN.board,
        KNOWN.gallery,
    ] {
        assert!(!sends_from(browser, decide), "browser {browser}");
    }
    assert!(sends_from(KNOWN.status_bar, markdown));
    assert!(sends_from(KNOWN.pages, markdown));
    assert!(!sends_from(KNOWN.board, markdown));
    assert!(!sends_from(KNOWN.gallery, markdown));
}

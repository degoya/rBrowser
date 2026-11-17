use super::*;

fn ui_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(dir.path().join("assets")).expect("assets");
    std::fs::write(dir.path().join("index.html"), "<!doctype html>").expect("index");
    std::fs::write(dir.path().join("assets/app-1a2b.js"), "export {}").expect("js");
    dir
}

#[test]
fn shell_files_are_served_with_their_type() {
    let dir = ui_dir();
    assert_eq!(
        resolve(dir.path(), "rbrowser://ui/assets/app-1a2b.js?v=1"),
        ShellResource::File(
            dir.path().join("assets").join("app-1a2b.js"),
            "text/javascript"
        )
    );
    assert_eq!(
        resolve(dir.path(), "rbrowser://ui/"),
        ShellResource::File(dir.path().join("index.html"), "text/html")
    );
}

#[test]
fn without_a_build_the_entry_page_is_the_placeholder() {
    let empty = tempfile::tempdir().expect("temp dir");
    assert_eq!(resolve(empty.path(), SHELL_URL), ShellResource::Placeholder);
    assert_eq!(
        resolve(empty.path(), "rbrowser://ui/assets/app.js"),
        ShellResource::NotFound
    );
    let page = placeholder_html();
    assert!(
        page.contains(&rb_i18n::t("shell.placeholder.hint")),
        "{page}"
    );
}

#[test]
fn the_placeholder_escapes_every_html_character_of_its_texts() {
    assert_eq!(escape("<b>&'\""), "&lt;b&gt;&amp;&#39;&quot;");
}

#[test]
fn paths_that_leave_the_folder_are_refused() {
    let dir = ui_dir();
    for url in [
        "rbrowser://ui/../secret.txt",
        "rbrowser://ui/%2e%2e/secret.txt",
        "rbrowser://ui/assets/..%2f..%2fsecret.txt",
        "rbrowser://ui/assets\\..\\index.html",
        "rbrowser://ui/C:/Windows/win.ini",
        "rbrowser://ui//index.html",
        "rbrowser://ui/%zz",
        "https://ui/index.html",
        "rbrowser://other/index.html",
    ] {
        assert_eq!(resolve(dir.path(), url), ShellResource::NotFound, "{url}");
    }
}

#[test]
fn shell_urls_are_recognised_by_origin() {
    assert!(is_shell_url("rbrowser://ui/index.html"));
    assert!(is_shell_url("RBROWSER://UI/index.html"));
    assert!(!is_shell_url("rbrowser://uix/index.html"));
    assert!(!is_shell_url("https://example.com/rbrowser://ui/"));
}

#[test]
fn unknown_extensions_are_binary() {
    assert_eq!(mime_type(Path::new("a.woff2")), "font/woff2");
    assert_eq!(mime_type(Path::new("A.HTML")), "text/html");
    assert_eq!(mime_type(Path::new("blob")), "application/octet-stream");
}

#[test]
fn the_pages_window_routes_by_hash() {
    assert_eq!(
        page_url(&format!("{ROUTE_SETTINGS}/sync.scroll")),
        "rbrowser://ui/settings.html#/settings/sync.scroll"
    );
    assert_eq!(page_url(ROUTE_TRUST), "rbrowser://ui/settings.html#/trust");
    assert!(is_shell_url(&page_url(ROUTE_ANALYZER)));
}

#[test]
fn only_the_shell_loads_shell_pages_and_a_popup_never() {
    let page = "https://shop.test/";
    for main in [true, false] {
        assert!(navigation_refused(PageOwner::Popup, main, SHELL_URL));
        assert!(navigation_refused(PageOwner::View, main, SHELL_URL));
        assert!(!navigation_refused(PageOwner::Popup, main, page));
        assert!(!navigation_refused(PageOwner::View, main, page));
    }
    assert!(navigation_refused(PageOwner::Shell, true, page));
    assert!(!navigation_refused(PageOwner::Shell, false, page));
    assert!(!navigation_refused(PageOwner::Shell, true, SHELL_URL));
}

#[test]
fn no_grid_view_popup_or_shell_loads_a_report() {
    let report = "rbrowser://report/0123456789abcdef0123456789abcdef/mobile.html";
    for main in [true, false] {
        assert!(navigation_refused(PageOwner::View, main, report));
        assert!(navigation_refused(PageOwner::Popup, main, report));
    }
    assert!(navigation_refused(PageOwner::Shell, true, report));
}

#[test]
fn shell_console_lines_reach_the_log_at_their_level() {
    use tracing::Level;
    // cef_log_severity_t: DEFAULT 0, VERBOSE 1, INFO 2, WARNING 3, ERROR 4, FATAL 5.
    assert_eq!(console_level(4), Level::ERROR);
    assert_eq!(console_level(5), Level::ERROR);
    assert_eq!(console_level(3), Level::WARN);
    assert_eq!(console_level(2), Level::DEBUG);
    assert_eq!(console_level(0), Level::DEBUG);
    assert_eq!(console_level(1), Level::TRACE);
}

#[test]
fn a_long_console_line_is_cut() {
    assert_eq!(console_text("short"), "short");
    let long = "x".repeat(CONSOLE_MAX_CHARS + 50);
    let cut = console_text(&long);
    assert_eq!(cut.chars().count(), CONSOLE_MAX_CHARS + 3);
    assert!(cut.ends_with("..."));
    // A cut never splits a character.
    let wide = "\u{20ac}".repeat(CONSOLE_MAX_CHARS + 1);
    assert!(console_text(&wide).starts_with('\u{20ac}'));
}

// RB-080-50: Ctrl + wheel zoomed the shell's pages (Chromium keeps the level per host).
#[test]
fn any_zoom_level_but_zero_counts_as_zoomed() {
    assert!(!is_zoomed(0.0));
    assert!(!is_zoomed(-0.0));
    for level in [1.0, -1.0, 0.5, 7.0, f64::NAN, f64::INFINITY] {
        assert!(is_zoomed(level), "{level}");
    }
}

// Review of RB-080-50: the zoom is refused where Chromium takes it, the shell's keyboard handler.
#[test]
fn ctrl_with_plus_minus_or_zero_is_a_zoom_key_and_nothing_else_is() {
    const SHIFT: u32 = 1 << 1;
    const CTRL: u32 = 1 << 2;
    const ALT: u32 = 1 << 3;
    for key in [0xBB, 0x6B, 0xBD, 0x6D, 0x30, 0x60] {
        assert!(is_zoom_key(CTRL, key), "{key:#x}");
        assert!(!is_zoom_key(0, key), "{key:#x} alone");
        assert!(!is_zoom_key(CTRL | ALT, key), "{key:#x} with AltGr");
    }
    // Ctrl+Shift+= is Plus on a US keyboard; Ctrl+Shift+0 is not Chromium's reset.
    assert!(is_zoom_key(CTRL | SHIFT, 0xBB));
    assert!(!is_zoom_key(CTRL | SHIFT, 0x30));
    assert!(!is_zoom_key(CTRL, 0x41), "Ctrl+A");
    assert!(!is_zoom_key(CTRL, 0x31), "Ctrl+1");
}

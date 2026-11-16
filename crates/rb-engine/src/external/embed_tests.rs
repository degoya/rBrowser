use super::*;

fn rect(x: i32, y: i32, width: u32, height: u32) -> WindowRect {
    WindowRect {
        x,
        y,
        width,
        height,
    }
}

/// A normal Firefox top-level window: overlapped with a caption, a thick frame, the boxes and
/// the system menu, and the app-window and window-edge extended styles.
const FIREFOX: WindowStyle = WindowStyle {
    style: WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | 0x1000_0000,
    ex_style: WS_EX_APPWINDOW | WS_EX_WINDOWEDGE | 0x0000_0010,
};

#[test]
fn the_embedded_style_is_a_frameless_tool_popup() {
    let embedded = embedded_style(FIREFOX);
    assert_ne!(embedded.style & WS_POPUP, 0, "a popup");
    for frame in [
        WS_CAPTION,
        WS_SYSMENU,
        WS_THICKFRAME,
        WS_MINIMIZEBOX,
        WS_MAXIMIZEBOX,
    ] {
        assert_eq!(embedded.style & frame, 0, "no frame bit {frame:#x}");
    }
    // Unrelated bits (WS_VISIBLE here) stay.
    assert_ne!(embedded.style & 0x1000_0000, 0);
    assert_ne!(embedded.ex_style & WS_EX_TOOLWINDOW, 0, "off the taskbar");
    assert_eq!(embedded.ex_style & WS_EX_APPWINDOW, 0);
    assert_eq!(embedded.ex_style & WS_EX_WINDOWEDGE, 0);
    assert_ne!(embedded.ex_style & 0x0000_0010, 0, "unrelated ex bits stay");
    assert!(is_embedded(embedded));
    assert!(!is_embedded(FIREFOX));
    // Idempotent: embedding twice is the same style.
    assert_eq!(embedded_style(embedded), embedded);
}

#[test]
fn a_cell_inside_the_area_needs_no_region() {
    let area = rect(100, 100, 1600, 900);
    let cell = rect(120, 140, 400, 300);
    assert_eq!(
        place_in_cell(cell, area, true, 0),
        EmbedPlace::Shown {
            rect: cell,
            region: None
        }
    );
}

#[test]
fn a_cell_partly_outside_the_area_is_clipped_by_a_region_in_window_coordinates() {
    let area = rect(100, 100, 1600, 900);
    // 50 px over the left edge and 100 px over the top edge of the area.
    let cell = rect(50, 0, 400, 300);
    assert_eq!(
        place_in_cell(cell, area, true, 0),
        EmbedPlace::Shown {
            rect: cell,
            region: Some(rect(50, 100, 350, 200)),
        }
    );
    // Over the bottom-right corner: the region ends at the area's edges.
    let cell = rect(1500, 900, 400, 300);
    assert_eq!(
        place_in_cell(cell, area, true, 0),
        EmbedPlace::Shown {
            rect: cell,
            region: Some(rect(0, 0, 200, 100)),
        }
    );
}

#[test]
fn a_cell_outside_the_area_or_not_shown_is_hidden() {
    let area = rect(100, 100, 1600, 900);
    assert_eq!(
        place_in_cell(rect(2000, 100, 400, 300), area, true, 0),
        EmbedPlace::Hidden
    );
    assert_eq!(
        place_in_cell(rect(100, 1000, 400, 300), area, true, 0),
        EmbedPlace::Hidden
    );
    assert_eq!(
        place_in_cell(rect(120, 140, 0, 300), area, true, 0),
        EmbedPlace::Hidden
    );
    assert_eq!(
        place_in_cell(rect(120, 140, 400, 300), area, false, 0),
        EmbedPlace::Hidden
    );
}

#[test]
fn the_only_free_window_is_taken() {
    let candidates = [
        Candidate {
            handle: 11,
            rect: rect(0, 0, 800, 600),
        },
        Candidate {
            handle: 12,
            rect: rect(32, 32, 800, 600),
        },
    ];
    assert_eq!(pick_window(&candidates, &[11], None), Some(12));
    assert_eq!(pick_window(&candidates, &[11, 12], None), None);
    assert_eq!(pick_window(&[], &[], None), None);
}

#[test]
fn among_several_free_windows_the_one_nearest_the_expected_place_is_taken() {
    let candidates = [
        Candidate {
            handle: 11,
            rect: rect(1008, 0, 800, 600),
        },
        Candidate {
            handle: 12,
            rect: rect(1040, 32, 800, 600),
        },
        Candidate {
            handle: 13,
            rect: rect(1072, 64, 800, 600),
        },
    ];
    // Firefox reports logical pixels; the screen is at 125 %: the nearest wins, not an exact one.
    let expected = Some(rect(1050, 40, 800, 600));
    assert_eq!(pick_window(&candidates, &[], expected), Some(12));
    assert_eq!(pick_window(&candidates, &[12], expected), Some(13));
    // Without an expectation several free windows are ambiguous: the first (the top-most).
    assert_eq!(pick_window(&candidates, &[], None), Some(11));
}

#[test]
fn a_native_window_hint_names_the_process_and_optionally_the_class() {
    let hint = NativeWindow {
        process_id: 4242,
        window_class: Some("MozillaWindowClass".to_owned()),
    };
    assert_eq!(hint.process_id, 4242);
    assert_eq!(hint.window_class.as_deref(), Some("MozillaWindowClass"));
}

/// WebKit names no window class, so a popup or an inspector of its process is a candidate too:
/// the window at the breakpoint's size wins over one at another size (RB-080-39).
#[test]
fn among_free_windows_of_unknown_class_the_one_of_the_expected_size_is_taken() {
    let candidates = [
        Candidate {
            handle: 21,
            rect: rect(1008, 0, 1200, 700),
        },
        Candidate {
            handle: 22,
            rect: rect(1012, 4, 390, 844),
        },
    ];
    let expected = Some(rect(1008, 0, 390, 844));
    assert_eq!(pick_window(&candidates, &[], expected), Some(22));
    assert_eq!(pick_window(&candidates, &[22], expected), Some(21));
}

/// The WebKit shell keeps a toolbar above its page (RB-080-40): the window goes up by the
/// toolbar's height and a region hides it, so the cell shows the page alone.
#[test]
fn a_window_with_a_toolbar_is_moved_up_and_the_toolbar_is_clipped_away() {
    let area = rect(100, 100, 1600, 900);
    let cell = rect(120, 140, 400, 300);
    assert_eq!(
        place_in_cell(cell, area, true, 36),
        EmbedPlace::Shown {
            rect: rect(120, 104, 400, 336),
            region: Some(rect(0, 36, 400, 300)),
        }
    );
    // Over the top edge of the area as well: the region is the visible part below the toolbar.
    let cell = rect(50, 0, 400, 300);
    assert_eq!(
        place_in_cell(cell, area, true, 36),
        EmbedPlace::Shown {
            rect: rect(50, -36, 400, 336),
            region: Some(rect(50, 136, 350, 200)),
        }
    );
}

#[test]
fn the_content_inset_is_the_top_of_the_largest_child_window() {
    // The Playwright shell: a toolbar across the top, the web view below it.
    let children = [rect(0, 0, 800, 36), rect(0, 36, 800, 564)];
    assert_eq!(content_inset(&children), 36);
    // A window whose page fills it (Firefox: the page is the only child that counts).
    assert_eq!(content_inset(&[rect(0, 0, 800, 600)]), 0);
    assert_eq!(content_inset(&[]), 0);
    // A child above the client area (scrolled, hidden) never gives a negative inset.
    assert_eq!(content_inset(&[rect(0, -10, 800, 600)]), 0);
}

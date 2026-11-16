use super::*;

#[test]
fn each_window_loads_its_route_of_the_pages() {
    assert_eq!(
        PageWindow::Palette.url(),
        "rbrowser://ui/settings.html#/palette"
    );
    for window in PageWindow::ALL {
        assert!(crate::shell::is_shell_url(&window.url()));
        assert!(window.title_key().ends_with(".window.title"));
    }
}

#[test]
fn the_palette_and_the_message_history_are_popups() {
    assert!(PageWindow::Palette.is_popup());
    assert!(PageWindow::History.is_popup());
    assert!(!PageWindow::Panels.is_popup());
    assert_eq!(
        PageWindow::History.url(),
        "rbrowser://ui/settings.html#/status-history"
    );
}

#[test]
fn the_palette_hangs_below_the_toolbar_centred_in_the_window() {
    let client = ScreenRect {
        x: 100,
        y: 50,
        width: 1600,
        height: 900,
    };
    let bounds = palette_bounds(client, 44);
    assert_eq!(bounds.y, 50 + 44);
    assert_eq!(bounds.width, PALETTE_SIZE.0);
    assert_eq!(bounds.height, PALETTE_SIZE.1);
    assert_eq!(
        bounds.x - client.x,
        client.width - (bounds.x - client.x) - bounds.width,
        "centred"
    );
}

#[test]
fn a_narrow_or_low_window_gets_a_smaller_palette_inside_it() {
    let client = ScreenRect {
        x: 0,
        y: 0,
        width: 400,
        height: 300,
    };
    let bounds = palette_bounds(client, 44);
    assert_eq!(bounds.x, MARGIN);
    assert_eq!(bounds.width, 400 - 2 * MARGIN);
    assert_eq!(bounds.y, 44);
    assert_eq!(bounds.height, 300 - 44 - MARGIN);
    // Never below the smallest usable size, whatever the window.
    let tiny = palette_bounds(
        ScreenRect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        },
        44,
    );
    assert_eq!((tiny.width, tiny.height), MIN_SIZE);
}

#[test]
fn the_history_stands_on_the_status_bar_at_the_right_edge() {
    let client = ScreenRect {
        x: 100,
        y: 50,
        width: 1600,
        height: 900,
    };
    let bounds = history_bounds(client, 44, 24);
    assert_eq!((bounds.width, bounds.height), HISTORY_SIZE);
    assert_eq!(bounds.y + bounds.height, 50 + 900 - 24, "on the bar");
    assert_eq!(
        bounds.x + bounds.width,
        100 + 1600 - MARGIN,
        "at the right edge"
    );
    // Without the bar it stands on the window's lower edge.
    let hidden = history_bounds(client, 44, 0);
    assert_eq!(hidden.y + hidden.height, 50 + 900);
}

#[test]
fn a_small_window_gets_a_smaller_history_that_never_covers_the_toolbar() {
    let client = ScreenRect {
        x: 0,
        y: 0,
        width: 400,
        height: 300,
    };
    let bounds = history_bounds(client, 44, 24);
    assert_eq!(bounds.width, 400 - 2 * MARGIN);
    assert_eq!(bounds.x, MARGIN);
    assert_eq!(bounds.height, 300 - 44 - 24 - MARGIN);
    assert!(bounds.y >= 44);
    let tiny = history_bounds(
        ScreenRect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        },
        44,
        24,
    );
    assert_eq!((tiny.width, tiny.height), MIN_SIZE);
}

#[test]
fn every_page_window_but_the_palette_can_be_resized() {
    assert!(PageWindow::Panels.resizable());
    assert!(!PageWindow::Palette.resizable());
}

#[test]
fn a_closing_window_leaves_its_slot_at_once_and_its_end_spares_the_next() {
    // RB-080-57: docking closes the panels window; an open right after makes a new one.
    let mut book = Generations::default();
    let first = book.open(PageWindow::Panels);
    assert!(book.is_current(PageWindow::Panels, first));
    assert!(book.close(PageWindow::Panels));
    assert!(!book.is_current(PageWindow::Panels, first));
    assert!(!book.close(PageWindow::Panels), "closed already");
    let second = book.open(PageWindow::Panels);
    assert_ne!(first, second);
    // The first window's end comes late: the second stays.
    assert!(!book.destroyed(PageWindow::Panels, first));
    assert!(book.is_current(PageWindow::Panels, second));
    // Its own end clears it.
    assert!(book.destroyed(PageWindow::Panels, second));
    assert!(!book.is_current(PageWindow::Panels, second));
    // Kinds are apart.
    let palette = book.open(PageWindow::Palette);
    assert!(!book.is_current(PageWindow::Panels, palette));
}

#[test]
fn a_browser_of_a_dock_already_removed_is_an_orphan() {
    // RB-080-57: the docked page's browser arrives after its dock went - no query rights.
    let mut book = Generations::default();
    let removed = book.open(());
    book.close(());
    let current = book.open(());
    assert!(!book.is_current((), removed));
    assert!(book.is_current((), current));
}

use super::*;

fn work(width: i32, height: i32) -> ScreenRect {
    ScreenRect {
        x: 0,
        y: 0,
        width,
        height,
    }
}

/// A Windows 10/11 frame: the title bar and the invisible resize borders, in DIP.
const FRAME: Insets = Insets {
    top: 31,
    left: 8,
    bottom: 8,
    right: 8,
};

fn input(viewport: (u32, u32), work_area: ScreenRect) -> PlaceInput {
    PlaceInput {
        viewport,
        work_area,
        insets: FRAME,
        remembered: None,
        fit_to_screen: true,
    }
}

#[test]
fn a_breakpoint_that_fits_opens_at_its_real_size_centred() {
    let placed = place(&input((1280, 800), work(2560, 1400)));
    assert_eq!(placed.client.width, 1280);
    assert_eq!(placed.client.height, 800);
    assert!((placed.zoom - 1.0).abs() < f64::EPSILON);
    assert!(!placed.exceeds_screen);
    // Centred in the work area with its frame.
    let left = placed.outer.x;
    let right = 2560 - (placed.outer.x + placed.outer.width);
    assert!((left - right).abs() <= 1, "{left} vs {right}");
    // The outer bounds are the client with the frame around it.
    assert_eq!(placed.outer.width, 1280 + 16);
    assert_eq!(placed.outer.height, 800 + 31 + 8);
    assert_eq!(placed.client.x, placed.outer.x + 8);
    assert_eq!(placed.client.y, placed.outer.y + 31);
}

#[test]
fn a_1920_breakpoint_on_a_150_percent_screen_is_limited_with_its_zoom() {
    // 2560 x 1440 physical pixels at 150 % are 1706 x 960 DIP, minus a 40 px taskbar.
    let area = dip_area(
        ScreenRect {
            x: 0,
            y: 0,
            width: 2560,
            height: 1380,
        },
        1.5,
    );
    assert_eq!(area.width, 1707);
    let placed = place(&input((1920, 1080), area));
    assert!(placed.exceeds_screen);
    assert_eq!(placed.client.width, area.width - 16);
    assert!(placed.zoom < 1.0);
    let expected = f64::from(area.width - 16) / 1920.0;
    assert!((placed.zoom - expected).abs() < 1e-9);
    // Inside the work area, frame included.
    assert!(placed.outer.x >= area.x);
    assert!(placed.outer.y >= area.y);
    assert!(placed.outer.x + placed.outer.width <= area.x + area.width);
    assert!(placed.outer.y + placed.outer.height <= area.y + area.height);
}

#[test]
fn a_tall_breakpoint_keeps_its_width_and_zoom_one_when_only_the_height_is_cut() {
    let placed = place(&input((393, 2000), work(1920, 1040)));
    assert_eq!(placed.client.width, 393);
    assert_eq!(placed.client.height, 1040 - 31 - 8);
    assert!((placed.zoom - 1.0).abs() < f64::EPSILON);
    assert!(placed.exceeds_screen);
}

#[test]
fn without_fit_to_screen_the_window_keeps_the_real_size_and_its_title_bar_visible() {
    let mut wanted = input((1920, 1080), work(1366, 728));
    wanted.fit_to_screen = false;
    let placed = place(&wanted);
    assert_eq!(placed.client.width, 1920);
    assert_eq!(placed.client.height, 1080);
    assert!((placed.zoom - 1.0).abs() < f64::EPSILON);
    assert!(placed.exceeds_screen);
    assert!(
        placed.outer.x >= 0 && placed.outer.y >= 0,
        "{:?}",
        placed.outer
    );
}

#[test]
fn remembered_bounds_come_back_inside_the_screen() {
    let mut wanted = input((1440, 900), work(1920, 1040));
    wanted.remembered = Some(ScreenRect {
        x: 300,
        y: 100,
        width: 1000,
        height: 700,
    });
    let placed = place(&wanted);
    assert_eq!(
        placed.client,
        ScreenRect {
            x: 300,
            y: 100,
            width: 1000,
            height: 700
        }
    );
    let expected = 1000.0 / 1440.0;
    assert!((placed.zoom - expected).abs() < 1e-9);
    assert!(!placed.exceeds_screen);
}

#[test]
fn remembered_bounds_off_screen_or_wider_than_the_page_are_pulled_back() {
    let mut wanted = input((800, 600), work(1920, 1040));
    wanted.remembered = Some(ScreenRect {
        x: 5000,
        y: -900,
        width: 1200,
        height: 600,
    });
    let placed = place(&wanted);
    // Never wider than the page: the rest would be an empty band.
    assert_eq!(placed.client.width, 800);
    assert!(placed.outer.x + placed.outer.width <= 1920);
    assert!(placed.outer.y >= 0);
}

#[test]
fn no_side_is_ever_zero() {
    let placed = place(&input((0, 0), work(0, 0)));
    assert!(placed.client.width >= 1 && placed.client.height >= 1);
    assert!(placed.outer.width >= 1 && placed.outer.height >= 1);
    assert!(placed.zoom > 0.0 && placed.zoom <= 1.0);
}

#[test]
fn the_frame_is_measured_from_the_window_and_its_client_area() {
    let outer = ScreenRect {
        x: 100,
        y: 100,
        width: 816,
        height: 639,
    };
    let client = ScreenRect {
        x: 108,
        y: 131,
        width: 800,
        height: 600,
    };
    assert_eq!(frame_insets(outer, client), FRAME);
    // Nonsense (a hidden window, a client outside the window) counts as no frame.
    let wrong = ScreenRect {
        x: 0,
        y: 0,
        width: 800,
        height: 600,
    };
    assert_eq!(frame_insets(outer, wrong), Insets::default());
}

#[test]
fn the_title_names_the_size_and_what_the_screen_allowed() {
    let fits = Placement {
        outer: work(10, 10),
        client: work(10, 10),
        zoom: 1.0,
        exceeds_screen: false,
    };
    assert_eq!(title_key(&fits, true), TITLE_KEY);
    let limited = Placement {
        zoom: 0.75,
        exceeds_screen: true,
        ..fits
    };
    assert_eq!(title_key(&limited, true), TITLE_ZOOMED_KEY);
    assert_eq!(percent(limited.zoom), 75);
    let cut = Placement {
        exceeds_screen: true,
        ..fits
    };
    assert_eq!(title_key(&cut, true), TITLE_CUT_KEY);
    assert_eq!(title_key(&cut, false), TITLE_LARGER_KEY);
}

#[test]
fn the_page_in_the_window_is_never_scaled_by_its_cell_zoom() {
    let emulation = Emulation {
        width: 1920,
        height: 1080,
        scale: Some(0.5),
        ..Emulation::default()
    };
    let window = CellRect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1050,
    };
    let metrics = metrics(&emulation, window);
    assert!((metrics.scale - 1.0).abs() < f64::EPSILON);
    assert_eq!(metrics.width, 1920);
    assert_eq!(metrics.height, 1050);
    // A window narrower than the page shows it scaled down to fit, never up.
    let narrow = CellRect {
        width: 960,
        ..window
    };
    let metrics = super::metrics(&emulation, narrow);
    assert!((metrics.scale - 0.5).abs() < 1e-9);
    assert_eq!(metrics.width, 1920);
}

#[test]
fn the_kept_bounds_are_the_pages_whatever_the_footer_does() {
    let client = ScreenRect {
        x: 40,
        y: 60,
        width: 768,
        height: 1042,
    };
    let page = taller(client, -18);
    assert_eq!(page.height, 1024);
    assert_eq!((page.x, page.y, page.width), (40, 60, 768));
    assert_eq!(taller(page, 18), client);
    assert_eq!(taller(page, 0), page);
    assert_eq!(
        taller(
            ScreenRect {
                height: 5,
                ..client
            },
            -18
        )
        .height,
        1
    );
}

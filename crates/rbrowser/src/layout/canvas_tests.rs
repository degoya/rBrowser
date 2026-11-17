use super::*;
use crate::layout::overlaps;

const LABEL: u32 = 20;

fn size(breakpoint: &str, width: u32, height: u32) -> Item {
    Item {
        breakpoint: breakpoint.to_owned(),
        width,
        height,
    }
}

fn item(breakpoint: &str, x: i32, y: i32) -> CanvasItem {
    CanvasItem {
        breakpoint: breakpoint.to_owned(),
        x,
        y,
        zoom: 0.5,
    }
}

fn area(width: u32, height: u32) -> CellRect {
    CellRect {
        x: 0,
        y: 44,
        width,
        height,
    }
}

/// Two views side by side and one below them, each at zoom 0.5.
fn board() -> (Vec<CanvasItem>, Vec<Item>) {
    (
        vec![item("a", 0, 20), item("b", 216, 20), item("c", 0, 456)],
        vec![
            size("a", 400, 800),
            size("b", 1200, 800),
            size("c", 800, 1000),
        ],
    )
}

/// With `layoutModes.showPartialViews` off a view is hidden unless it lies wholly inside.
#[test]
fn the_board_at_three_window_sizes() {
    let (items, sizes) = board();
    for (window, shown) in [
        ((1920, 1000), [true, true, true]),
        ((1280, 700), [true, true, false]),
        ((800, 600), [true, false, false]),
    ] {
        let area = area(window.0, window.1);
        let slots = arrange(&items, &sizes, area, Camera::default(), LABEL, false);
        assert_eq!(
            slots[0].outer(),
            CellRect {
                x: 0,
                y: 44,
                width: 200,
                height: 420
            }
        );
        assert_eq!(slots[1].label.x, 216);
        assert_eq!(
            slots[2].rect,
            CellRect {
                x: 0,
                y: 44 + 456,
                width: 400,
                height: 500
            }
        );
        let visible: Vec<bool> = slots.iter().map(|slot| slot.visible).collect();
        assert_eq!(visible, shown, "{window:?}");
        let on_screen: Vec<&Slot> = slots.iter().filter(|slot| slot.visible).collect();
        for (index, slot) in on_screen.iter().enumerate() {
            assert!(contains(area, slot.outer()));
            for other in &on_screen[index + 1..] {
                assert!(!overlaps(slot.outer(), other.outer()));
            }
        }
    }
}

/// RB-080-55: a view that reaches past an edge of the area is shown - the window clips it -
/// and keeps its full rectangle; only one whose view lies wholly outside is hidden.
#[test]
fn a_view_past_the_edge_is_shown_until_it_is_wholly_outside() {
    let sizes = vec![size("a", 400, 800)];
    let area = area(800, 600);
    let shown = |x: i32, y: i32, partial: bool| {
        let slots = arrange(
            &[item("a", x, y)],
            &sizes,
            area,
            Camera::default(),
            LABEL,
            partial,
        );
        (slots[0].visible, slots[0].rect)
    };
    // 200 x 400 on screen. Past the right, the bottom, the left and the top edge by a little.
    for (x, y) in [(700, 20), (0, 300), (-10, 20), (0, 10)] {
        let (visible, rect) = shown(x, y, true);
        assert!(visible, "{x}/{y} reaches past an edge and is shown");
        assert_eq!(
            (rect.width, rect.height),
            (200, 400),
            "the view keeps its size"
        );
        assert_eq!((rect.x, rect.y), (x, 44 + y), "and its place");
        assert!(
            !shown(x, y, false).0,
            "{x}/{y}: hidden with the setting off"
        );
    }
    // Wholly inside: shown either way.
    assert!(shown(0, 20, true).0 && shown(0, 20, false).0);
    // Wholly outside, or touching the edge without area inside.
    for (x, y) in [(800, 20), (-200, 20), (0, 600), (0, -400), (5000, 5000)] {
        assert!(!shown(x, y, true).0, "{x}/{y} is outside");
    }
    // Only its label band is inside (the view starts at the bottom edge): the board draws
    // the label, the view has nothing to show.
    assert!(shown(0, 599, true).0, "one DIP of the view is inside");
    assert!(!shown(0, 600, true).0);
}

#[test]
fn zoom_to_fit_shows_every_view() {
    let (items, sizes) = board();
    let rects: Vec<WorldRect> = items
        .iter()
        .zip(&sizes)
        .map(|(item, size)| outer(item, size, LABEL))
        .collect();
    for window in [(1920, 1000), (1280, 700), (800, 600)] {
        let area = area(window.0, window.1);
        let camera = fit(&rects, area, 8);
        assert!(camera.zoom <= 1.0);
        // Wholly inside: shown without partial views too.
        let slots = arrange(&items, &sizes, area, camera, LABEL, false);
        assert!(
            slots.iter().all(|slot| slot.visible),
            "{window:?}: {slots:?}"
        );
    }
    assert_eq!(fit(&[], area(800, 600), 8), Camera::default());
}

/// `snap: 8`: a free move lands on multiples of 8; within 8 DIP of a neighbour's edge it snaps
/// to that edge instead.
#[test]
fn snapping_to_the_grid_and_to_a_neighbours_edge() {
    let neighbour = outer(&item("a", 0, 20), &size("a", 400, 800), LABEL);
    let view = (200.0, 400.0);
    assert_eq!(snap((13.0, 29.0), view, &[], 8, LABEL), (16, 32));
    // 5 DIP right of the neighbour, its band 7 DIP below the neighbour's top: both edges snap.
    assert_eq!(snap((205.0, 27.0), view, &[neighbour], 8, LABEL), (200, 20));
    // 9 DIP away the grid wins.
    assert_eq!(
        snap((209.0, 900.0), view, &[neighbour], 8, LABEL),
        (208, 904)
    );
    // The bottom of the moved band onto the neighbour's top.
    let above = snap((600.0, -395.0), view, &[neighbour], 8, LABEL);
    assert_eq!(
        above.1 + 400,
        0,
        "the view ends where the neighbour's band starts"
    );
    assert_eq!(snap((13.4, 29.6), view, &[neighbour], 0, LABEL), (13, 30));
}

#[test]
fn a_minimap_click_centres_the_camera() {
    let camera = center_on(500.0, 300.0, area(800, 600), 0.5);
    assert_eq!(
        camera,
        Camera {
            x: -300.0,
            y: -300.0,
            zoom: 0.5
        }
    );
    let shown = viewport(area(800, 600), camera);
    assert_eq!(
        (shown.x + shown.width / 2.0, shown.y + shown.height / 2.0),
        (500.0, 300.0)
    );
}

#[test]
fn views_without_an_item_are_placed_after_the_others() {
    let items = vec![item("a", 0, 20)];
    let sizes = vec![
        size("a", 400, 800),
        size("x", 400, 800),
        size("y", 200, 400),
    ];
    let complete = complete(&items, &sizes, 16, LABEL);
    assert_eq!(complete[0], items[0]);
    assert_eq!((complete[1].x, complete[1].y), (216, 20));
    assert_eq!((complete[2].x, complete[2].y), (216 + 200 + 16, 20));
    assert_eq!(complete[1].zoom, default_item_zoom());
    let fresh = super::complete(&[], &sizes[..1], 16, LABEL);
    assert_eq!((fresh[0].x, fresh[0].y), (0, 20));
}

#[test]
fn a_camera_out_of_range_is_held() {
    let wild = Camera {
        x: f64::NAN,
        y: 4.0,
        zoom: 99.0,
    }
    .clamped();
    assert_eq!(
        wild,
        Camera {
            x: 0.0,
            y: 4.0,
            zoom: ZOOM_RANGE.1
        }
    );
}

/// RB-080-55: the overlay of a view past an edge covers only the area's part of it, and its
/// content reaches out of it by what is cut off on each side.
#[test]
fn the_overlay_of_a_view_past_the_edge_is_cut_to_the_area() {
    let area = area(800, 600);
    let rect = |x: i32, y: i32| CellRect {
        x,
        y,
        width: 200,
        height: 400,
    };
    let inside = clip(rect(10, 60), area).expect("inside");
    assert_eq!(inside.bounds, rect(10, 60));
    assert_eq!(
        (inside.top, inside.left, inside.bottom, inside.right),
        (0, 0, 0, 0)
    );

    let right = clip(rect(700, 60), area).expect("past the right edge");
    assert_eq!(
        right.bounds,
        CellRect {
            x: 700,
            y: 60,
            width: 100,
            height: 400
        }
    );
    assert_eq!((right.left, right.right), (0, 100));

    let top_left = clip(rect(-50, 0), area).expect("past the top and the left edge");
    assert_eq!(
        top_left.bounds,
        CellRect {
            x: 0,
            y: 44,
            width: 150,
            height: 356
        }
    );
    assert_eq!((top_left.top, top_left.left), (44, 50));

    let bottom = clip(rect(0, 500), area).expect("past the bottom edge");
    assert_eq!((bottom.bounds.height, bottom.bottom), (144, 256));

    assert_eq!(clip(rect(800, 60), area), None, "touching the edge only");
    assert_eq!(clip(rect(0, -400), area), None, "wholly above");
}

/// RB-080-55, review 8: the label of a view cut at the top stays at the area's top edge, as a
/// sticky header does, until the view's rest is no taller than it; the view keeps its place.
#[test]
fn the_label_of_a_view_cut_at_the_top_sticks_to_the_top_edge() {
    let sizes = vec![size("a", 400, 800)];
    let area = area(800, 600);
    let at = |y: i32, partial: bool| {
        arrange(
            &[item("a", 0, y)],
            &sizes,
            area,
            Camera::default(),
            LABEL,
            partial,
        )[0]
    };
    // 100 DIP of the view above the area.
    let slot = at(-100, true);
    assert!(slot.visible);
    assert_eq!(slot.rect.y, 44 - 100);
    assert_eq!(slot.label.y, 44, "the label at the top edge");
    let outer = slot.outer();
    assert_eq!(
        outer.y,
        slot.rect.y - LABEL as i32,
        "the place is the view's"
    );
    assert_eq!(outer.height, LABEL + 400);
    // Only the label band cut: pinned too.
    assert_eq!(at(10, true).label.y, 44);
    // 15 DIP of the view left, less than the label: it goes up with the view, which is hidden
    // (the label would cover what is left of it).
    let slot = at(-385, true);
    assert_eq!(slot.label.y, 44 - 385 + 400 - LABEL as i32);
    assert!(!slot.visible);
    // Wholly inside, or the setting off: the label stays above its view.
    assert_eq!(at(100, true).label.y, 44 + 100 - LABEL as i32);
    assert_eq!(at(-100, false).label.y, 44 - 100 - LABEL as i32);
}

/// RB-080-55, review 8: the overlay of a view cut at the top starts below the label the board
/// holds at the top edge; without board labels only the area cuts it.
#[test]
fn the_overlay_of_a_view_cut_at_the_top_starts_below_the_held_label() {
    let area = area(800, 600);
    // A 20 DIP label band over a 400 DIP view at -36.
    let outer = CellRect {
        x: 0,
        y: -56,
        width: 200,
        height: 420,
    };
    let cut = overlay_cut(outer, LABEL, Some(area)).expect("shown");
    assert_eq!(cut.bounds.y, 44 + LABEL as i32);
    assert_eq!(cut.top, 100);
    assert_eq!(cut.bounds.height, 300);
    assert_eq!(
        overlay_cut(outer, 0, Some(area)).map(|cut| cut.bounds.y),
        Some(44)
    );
    assert_eq!(
        overlay_cut(outer, LABEL, None).map(|cut| cut.bounds.height),
        Some(400),
        "before the area is known the whole view"
    );
}

#[test]
fn a_view_at_item_zoom_half_reaches_its_page_size_and_beyond() {
    // RB-080-94: the camera stopped at 1, so a view at the default item zoom 0.5 never came
    // above half of its page's size. On screen a view is size x item zoom x camera.
    let items = [item("a", 0, 0)];
    let sizes = [size("a", 1024, 640)];
    let area = area(4000, 3000);
    let at = |zoom: f64| {
        let camera = Camera {
            x: 0.0,
            y: -f64::from(LABEL),
            zoom,
        };
        arrange(&items, &sizes, area, camera, LABEL, true)[0].rect
    };
    assert_eq!(at(2.0).width, 1024, "100 % of the page");
    assert_eq!(at(4.0).width, 2048, "200 % of the page");
    assert!(
        Camera {
            zoom: 2.0,
            ..Camera::default()
        }
        .clamped()
        .zoom
            >= 2.0
    );
}

#[test]
fn the_cameras_limit_holds_the_largest_view_to_200_percent() {
    let half = [item("a", 0, 0), item("b", 0, 0)];
    assert!((camera_limit(&half) - 4.0).abs() < 1e-9, "200 % at 0.5");
    let mut mixed = half.to_vec();
    mixed[1].zoom = 1.0;
    assert!(
        (camera_limit(&mixed) - 2.0).abs() < 1e-9,
        "the largest view decides"
    );
    // No item: the camera's own range.
    assert!((camera_limit(&[]) - ZOOM_RANGE.1).abs() < 1e-9);
    let held = Camera {
        x: 1.0,
        y: 2.0,
        zoom: 9.0,
    }
    .within(2.0);
    assert_eq!((held.x, held.y), (1.0, 2.0));
    assert!((held.zoom - 2.0).abs() < 1e-9);
}

#[test]
fn fitting_never_zooms_past_one() {
    // A single small view is not blown up past the camera's 1 by fitting.
    let camera = fit(
        &[WorldRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
        }],
        area(1920, 1000),
        16,
    );
    assert!((camera.zoom - FIT_MAX).abs() < 1e-9);
}

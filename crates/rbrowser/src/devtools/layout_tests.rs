use super::*;

#[test]
fn each_docked_mode_has_its_side_and_the_window_none() {
    assert_eq!(side_of(DevtoolsMode::DockedBottom), Some(DockSide::Bottom));
    assert_eq!(side_of(DevtoolsMode::DockedRight), Some(DockSide::Right));
    assert_eq!(side_of(DevtoolsMode::Window), None);
}

#[test]
fn the_cycle_walks_bottom_right_window_and_back() {
    let mut mode = DevtoolsMode::DockedBottom;
    let mut seen = Vec::new();
    for _ in 0..4 {
        mode = next_mode(mode);
        seen.push(mode);
    }
    assert_eq!(
        seen,
        [
            DevtoolsMode::DockedRight,
            DevtoolsMode::Window,
            DevtoolsMode::DockedBottom,
            DevtoolsMode::DockedRight,
        ]
    );
}

#[test]
fn a_kept_size_is_held_between_the_minimum_and_the_maximum() {
    let sizes = DockSizes {
        bottom: 10,
        right: 99_999,
    };
    assert_eq!(sizes.size_for(DockSide::Bottom), MIN_DOCK);
    assert_eq!(sizes.size_for(DockSide::Right), MAX_DOCK);
    let defaults = DockSizes::default();
    assert_eq!(defaults.size_for(DockSide::Bottom), 320);
    assert_eq!(defaults.size_for(DockSide::Right), 480);
}

#[test]
fn the_dock_takes_what_it_wants_while_the_grid_keeps_its_share() {
    // 1000 DIP: the grid keeps 250, so the dock may take up to 750.
    assert_eq!(dock_extent(320, 1000), 320);
    assert_eq!(grid_extent(320, 1000), 680);
    assert_eq!(dock_extent(900, 1000), 750);
    assert_eq!(grid_extent(900, 1000), 250);
    // Never under the minimum while there is room for it.
    assert_eq!(dock_extent(40, 1000), MIN_DOCK);
}

#[test]
fn a_tiny_area_gives_the_grid_its_share_and_the_dock_the_rest() {
    assert_eq!(dock_extent(320, 100), 75);
    assert_eq!(grid_extent(320, 100), 25);
    assert_eq!(dock_extent(320, 0), 0);
    assert_eq!(grid_extent(320, 0), 0);
}

#[test]
fn the_sizes_read_from_json_fall_back_per_field() {
    let sizes: DockSizes = serde_json::from_str(r#"{ "right": 600 }"#).expect("parses");
    assert_eq!(
        sizes,
        DockSizes {
            bottom: 320,
            right: 600
        }
    );
}

fn want(horizontal: bool, wanted: u32) -> Option<DockWant> {
    Some(DockWant {
        horizontal,
        wanted,
        fixed: false,
    })
}

#[test]
fn one_dock_alone_is_sized_as_before() {
    // 1000 x 800 DIP: a bottom dock against the height, a side dock against the width.
    assert_eq!(shared_extents(1000, 800, None, want(false, 320)), (0, 320));
    assert_eq!(shared_extents(1000, 800, want(true, 900), None), (750, 0));
    assert_eq!(shared_extents(1000, 800, None, None), (0, 0));
    assert_eq!(
        shared_extents(1000, 800, want(true, 50), None),
        (MIN_DOCK, 0)
    );
}

#[test]
fn docks_on_different_axes_each_take_their_own_share() {
    // Panels left (width), DevTools below (height): neither takes from the other.
    assert_eq!(
        shared_extents(1000, 800, want(true, 700), want(false, 700)),
        (700, 600)
    );
}

#[test]
fn docks_on_one_axis_leave_the_views_a_quarter_of_the_whole_area() {
    // Both fit: as wanted.
    assert_eq!(
        shared_extents(1000, 800, want(true, 300), want(true, 400)),
        (300, 400)
    );
    // Too large together: the 750 DIP the views can give are shared by what each wants.
    let (panels, devtools) = shared_extents(1000, 800, want(true, 600), want(true, 900));
    assert_eq!((panels, devtools), (300, 450));
    assert!(1000 - panels - devtools >= 250, "the views keep a quarter");
    // Below the views, the same against the height.
    let (panels, devtools) = shared_extents(1000, 800, want(false, 4000), want(false, 4000));
    assert_eq!(panels + devtools, 600);
    assert_eq!(panels, 300);
}

/// Review point 7 of RB-080-61: the folded panels' strip (44 DIP) was left out of the shared
/// room, so DevTools beside it took the whole three quarters and the views kept less than a
/// quarter. The strip counts at its own size, below `MIN_DOCK`.
#[test]
fn a_folded_strip_counts_at_its_size_and_the_views_keep_their_quarter() {
    let strip = Some(DockWant {
        horizontal: true,
        wanted: 44,
        fixed: true,
    });
    // Alone: its own size, not MIN_DOCK.
    assert_eq!(shared_extents(1000, 800, strip, None), (44, 0));
    // Beside DevTools on the same axis that want everything: 1000 - 250 = 750 shared.
    let (panels, devtools) = shared_extents(1000, 800, strip, want(true, 900));
    assert_eq!((panels, devtools), (44, 706));
    assert!(1000 - panels - devtools >= 250);
    // Room enough: both as wanted.
    assert_eq!(shared_extents(1000, 800, strip, want(true, 400)), (44, 400));
    // DevTools below: each on its own axis.
    assert_eq!(
        shared_extents(1000, 800, strip, want(false, 320)),
        (44, 320)
    );
}

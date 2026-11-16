use super::*;

#[test]
fn only_a_report_windows_main_frame_is_answered() {
    // Ids far from any real browser's, so the shared set is not disturbed by other tests.
    register_browser(9_001);
    assert!(requester_allowed(Some(9_001), true));
    assert!(
        !requester_allowed(Some(9_001), false),
        "a frame in the report"
    );
    assert!(
        !requester_allowed(Some(9_002), true),
        "a grid view, a popup, the shell"
    );
    assert!(
        !requester_allowed(None, true),
        "a request without a browser"
    );
    forget_browser(9_001);
    assert!(!requester_allowed(Some(9_001), true), "a closed window");
}

#[test]
fn the_one_report_window_is_reused() {
    let open = [
        Open {
            id: 1,
            single: false,
        },
        Open {
            id: 2,
            single: true,
        },
    ];
    assert_eq!(choose(&open, Placement::Single), Target::Reuse(2));
    assert_eq!(choose(&[], Placement::Single), Target::New);
    assert_eq!(choose(&open[..1], Placement::Single), Target::New);
}

#[test]
fn a_window_per_report_opens_new_ones_up_to_the_cap() {
    let open: Vec<Open> = (1..u64::try_from(MAX_WINDOWS).expect("small"))
        .map(|id| Open { id, single: false })
        .collect();
    assert_eq!(choose(&open, Placement::PerReport), Target::New);
}

#[test]
fn past_the_cap_the_oldest_window_shows_the_report() {
    let open: Vec<Open> = (1..=u64::try_from(MAX_WINDOWS).expect("small"))
        .map(|id| Open {
            id,
            single: id == 3,
        })
        .collect();
    assert_eq!(choose(&open, Placement::PerReport), Target::Reuse(1));
    // The single window is reused before the cap matters.
    assert_eq!(choose(&open, Placement::Single), Target::Reuse(3));
    let none_single: Vec<Open> = open
        .iter()
        .map(|o| Open {
            id: o.id,
            single: false,
        })
        .collect();
    assert_eq!(choose(&none_single, Placement::Single), Target::Reuse(1));
}

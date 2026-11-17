use super::*;

fn rect(x: i32, y: i32, width: u32, height: u32) -> WindowRect {
    WindowRect {
        x,
        y,
        width,
        height,
    }
}

/// Two 1920 x 1040 work areas side by side; the main window fills the left one's left half.
fn two_monitors() -> Screen {
    Screen {
        main: rect(0, 0, 960, 1040),
        monitors: vec![rect(0, 0, 1920, 1040), rect(1920, 0, 1920, 1040)],
    }
}

#[test]
fn placements_parse_and_print() {
    for text in ["beside", "remember", "monitor:1", "monitor:12"] {
        let placement: Placement = text.parse().expect(text);
        assert_eq!(placement.to_string(), text);
    }
    for bad in [
        "",
        "left",
        "monitor:",
        "monitor:0",
        "monitor:100",
        "monitor:x",
    ] {
        assert!(bad.parse::<Placement>().is_err(), "{bad}");
    }
    let json: Placement = serde_json::from_str(r#""monitor:2""#).expect("parses");
    assert_eq!(json, Placement::Monitor(2));
    assert_eq!(
        serde_json::to_value(Placement::Beside).expect("serializes"),
        "beside"
    );
}

#[test]
fn beside_puts_the_first_window_right_of_the_main_window_and_cascades() {
    let screen = two_monitors();
    assert_eq!(
        place(Placement::Beside, &screen, (400, 800), 0, None),
        rect(960 + GAP, 0, 400, 800)
    );
    assert_eq!(
        place(Placement::Beside, &screen, (400, 800), 2, None),
        rect(960 + GAP + 2 * CASCADE_STEP, 2 * CASCADE_STEP, 400, 800)
    );
}

#[test]
fn beside_stays_on_the_screen_when_the_main_window_fills_it() {
    let screen = Screen {
        main: rect(0, 0, 1920, 1040),
        monitors: vec![rect(0, 0, 1920, 1040)],
    };
    let placed = place(Placement::Beside, &screen, (400, 800), 0, None);
    assert_eq!(placed, rect(1520, 0, 400, 800));
}

#[test]
fn a_window_larger_than_the_work_area_shrinks_into_it() {
    let screen = two_monitors();
    let placed = place(Placement::Monitor(2), &screen, (4000, 3000), 0, None);
    assert_eq!(placed, rect(1920, 0, 1920, 1040));
}

#[test]
fn monitor_n_uses_that_work_area_and_falls_back_to_beside() {
    let screen = two_monitors();
    assert_eq!(
        place(Placement::Monitor(2), &screen, (400, 800), 1, None),
        rect(1920 + 2 * CASCADE_STEP, 2 * CASCADE_STEP, 400, 800)
    );
    assert_eq!(
        place(Placement::Monitor(3), &screen, (400, 800), 0, None),
        place(Placement::Beside, &screen, (400, 800), 0, None)
    );
}

#[test]
fn remember_restores_a_window_that_is_still_on_a_monitor() {
    let screen = two_monitors();
    let last = rect(2500, 120, 500, 900);
    assert_eq!(
        place(Placement::Remember, &screen, (400, 800), 0, Some(last)),
        last
    );
    // The second monitor is gone: beside again.
    let one = Screen {
        main: screen.main,
        monitors: vec![screen.monitors[0]],
    };
    assert_eq!(
        place(Placement::Remember, &one, (400, 800), 0, Some(last)),
        place(Placement::Beside, &one, (400, 800), 0, None)
    );
    // Never remembered: beside.
    assert_eq!(
        place(Placement::Remember, &screen, (400, 800), 0, None),
        place(Placement::Beside, &screen, (400, 800), 0, None)
    );
}

#[test]
fn without_monitors_the_main_window_is_the_area() {
    let screen = Screen {
        main: rect(100, 100, 800, 600),
        monitors: Vec::new(),
    };
    let placed = place(Placement::Beside, &screen, (400, 300), 0, None);
    assert_eq!(placed, rect(500, 100, 400, 300));
}

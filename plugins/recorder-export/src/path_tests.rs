use super::*;
use crate::model::{Action, Locator, LocatorPreference, RecordedBreakpoint, Selector, Step};
use crate::steps::{Context, Guard, render};

fn point(x: i32, y: i32, t: u32) -> PathPoint {
    PathPoint { x, y, t }
}

fn way() -> Vec<PathPoint> {
    vec![point(12, 40, 0), point(60, 18, 48), point(96, 12, 120)]
}

fn hover(at_ms: u64, path: Vec<PathPoint>) -> Step {
    Step {
        id: 2,
        name: None,
        action: Action::Hover {
            target: Locator {
                selectors: vec![Selector::Css {
                    path: "#products".to_owned(),
                }],
                ..Locator::default()
            },
        },
        view: "desktop".to_owned(),
        only_in: Vec::new(),
        at_ms,
        path,
        disabled: false,
        pause_ms: None,
    }
}

fn navigate() -> Step {
    Step {
        id: 1,
        name: None,
        action: Action::Navigate {
            url: "http://localhost:3000/".to_owned(),
        },
        view: "desktop".to_owned(),
        only_in: Vec::new(),
        at_ms: 0,
        path: Vec::new(),
        disabled: false,
        pause_ms: None,
    }
}

fn breakpoints() -> Vec<RecordedBreakpoint> {
    vec![RecordedBreakpoint {
        id: "desktop".to_owned(),
        width: 1280,
        height: 800,
        device_scale_factor: 1.0,
    }]
}

fn flow(pace: Pace, steps: &[Step]) -> String {
    let mut context = Context::new(Guard::Breakpoint, LocatorPreference::Auto, None)
        .paced(pace)
        .sized(&breakpoints());
    render(steps, &mut context, 0).expect("renders")
}

#[test]
fn the_waits_follow_the_recorded_times_within_the_pause() {
    assert_eq!(waits(&way(), 1_000), vec![0, 48, 72]);
    // A pause shorter than the way: the way takes what there is, the rest moves at once.
    assert_eq!(waits(&way(), 60), vec![0, 48, 12]);
    assert_eq!(waits(&way(), 0), vec![0, 0, 0]);
}

#[test]
fn at_the_recorded_pace_the_way_moves_with_its_times_taken_from_the_pause() {
    let text = flow(Pace::Recorded, &[navigate(), hover(2_000, way())]);
    assert_eq!(
        text,
        "// Step 1: navigate\n\
         await step('Step 1: navigate', async () => {\n\
         \x20 await page.goto('http://localhost:3000/');\n\
         });\n\
         // Step 2: hover\n\
         await step('Step 2: hover', async () => {\n\
         \x20 await page.waitForTimeout(1880);\n\
         \x20 // Pointer path: 3 points, recorded at 1280x800.\n\
         \x20 if (view.width === 1280 && view.height === 800) {\n\
         \x20   for (const [x, y, wait] of [[12, 40, 0], [60, 18, 48], [96, 12, 72]]) {\n\
         \x20     if (wait > 0) await page.waitForTimeout(wait);\n\
         \x20     await page.mouse.move(x, y);\n\
         \x20   }\n\
         \x20 }\n\
         \x20 await page.locator('#products').hover();\n\
         });\n"
    );
}

#[test]
fn at_the_fast_pace_the_way_moves_through_its_points_without_waits() {
    let text = flow(Pace::Fast, &[navigate(), hover(2_000, way())]);
    assert!(
        text.contains(
            "  if (view.width === 1280 && view.height === 800) {\n\
             \x20   for (const [x, y] of [[12, 40], [60, 18], [96, 12]]) await page.mouse.move(x, y);\n\
             \x20 }\n\
             \x20 await page.locator('#products').hover();\n"
        ),
        "{text}"
    );
    assert!(!text.contains("waitForTimeout"), "{text}");
}

#[test]
fn the_spec_compares_the_viewport_and_a_view_of_another_size_skips_the_way() {
    let mut context = Context::new(Guard::Project, LocatorPreference::Auto, None)
        .paced(Pace::Fast)
        .sized(&breakpoints());
    let text = render(&[hover(0, way())], &mut context, 0).expect("renders");
    assert!(
        text.contains(
            "if (page.viewportSize()?.width === 1280 && page.viewportSize()?.height === 800) {"
        ),
        "{text}"
    );
}

#[test]
fn a_step_of_an_unknown_breakpoint_or_without_a_way_moves_straight_to_its_element() {
    let mut unknown = hover(2_000, way());
    unknown.view = "500x800".to_owned();
    for step in [unknown, hover(2_000, Vec::new())] {
        let text = flow(Pace::Recorded, &[navigate(), step]);
        assert!(!text.contains("mouse.move"), "{text}");
        assert!(text.contains("await page.waitForTimeout(2000);"), "{text}");
    }
}

#[test]
fn a_long_way_is_written_eight_points_a_line() {
    let long: Vec<PathPoint> = (0..10).map(|index| point(index * 10, 5, 0)).collect();
    let text = flow(Pace::Fast, &[hover(0, long)]);
    assert!(
        text.contains(
            "for (const [x, y] of [\n\
             \x20     [0, 5], [10, 5], [20, 5], [30, 5], [40, 5], [50, 5], [60, 5], [70, 5],\n\
             \x20     [80, 5], [90, 5],\n\
             \x20   ]) await page.mouse.move(x, y);\n"
        ),
        "{text}"
    );
}

#[test]
fn the_pauses_stay_the_one_rule_with_a_way() {
    // The way's time is part of the step's pause: the run budget does not change.
    let steps = [navigate(), hover(2_000, way())];
    assert_eq!(
        crate::steps::step_pauses(&steps, 0),
        vec![(1, 0), (2, 2_000)]
    );
}

#[test]
fn a_way_longer_than_a_recording_keeps_writes_only_its_last_points() {
    // The host caps a loaded way; a bare recording handed to the exporter is capped here too.
    let long: Vec<PathPoint> = (0..1_000).map(|index| point(index, 5, 0)).collect();
    let text = flow(Pace::Fast, &[hover(0, long)]);
    assert!(
        text.contains(&format!("Pointer path: {MAX_POINTS} points")),
        "{text}"
    );
    assert!(!text.contains("[935, 5]"), "{text}");
    assert!(text.contains("[936, 5]"), "{text}");
    assert!(text.contains("[999, 5]"), "{text}");
}

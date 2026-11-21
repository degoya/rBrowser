//! RB-090-87: every hover the agent marks is a step at once - a rest on a menu's entry of any
//! level, with or without a click after it - and the pointer's way goes with its step.

use crate::recorder::model::{Action, MAX_PATH_POINTS, PathPoint, Recording};
use crate::recorder::record::{HoverCapture, RecorderOptions};
use crate::recorder::testing::*;

fn kinds(recording: &Recording) -> Vec<String> {
    recording
        .steps
        .iter()
        .map(|step| match &step.action {
            Action::Hover { target } => format!("hover {}", target.css_path()),
            Action::Click { target, .. } => format!("click {}", target.css_path()),
            Action::Dblclick { target } => format!("dblclick {}", target.css_path()),
            other => format!("{other:?}"),
        })
        .collect()
}

fn products() -> crate::event::ElementSelector {
    role("link", "Products", "#nav > li:nth-child(2) > a")
}

fn software() -> crate::event::ElementSelector {
    role(
        "link",
        "Software",
        "#nav > li:nth-child(2) > div > ul > li:nth-child(1) > a",
    )
}

fn editor() -> crate::event::ElementSelector {
    role(
        "link",
        "Editor",
        "#nav > li:nth-child(2) > div > ul > li:nth-child(1) > ul > li:nth-child(1) > a",
    )
}

fn careers() -> crate::event::ElementSelector {
    role("link", "Careers", "#nav > li:nth-child(3) > a")
}

#[test]
fn a_recording_of_hovers_only_has_every_rest_on_both_levels_and_another_menu() {
    let mut recorder = started(RecorderOptions::default(), Some("http://localhost:3000/"));
    for (index, entry) in [products(), software(), editor(), careers()]
        .into_iter()
        .enumerate()
    {
        feed(
            &mut recorder,
            DESKTOP,
            &[MOBILE],
            300 * (index as u64 + 1),
            menu_hover(entry),
        );
    }
    let recording = recorder.stop().expect("stops");
    assert_eq!(
        kinds(&recording)[1..],
        [
            "hover #nav > li:nth-child(2) > a",
            "hover #nav > li:nth-child(2) > div > ul > li:nth-child(1) > a",
            "hover #nav > li:nth-child(2) > div > ul > li:nth-child(1) > ul > li:nth-child(1) > a",
            "hover #nav > li:nth-child(3) > a",
        ]
    );
    let times: Vec<u64> = recording.steps[1..].iter().map(|step| step.at_ms).collect();
    assert_eq!(times, [300, 600, 900, 1200]);
}

#[test]
fn the_rests_before_a_click_stay_in_order_and_a_repeated_entry_is_one_step() {
    let recording = {
        let mut recorder = started(RecorderOptions::default(), None);
        let events = [
            menu_hover(products()),
            hover(software()),
            menu_hover(software()),
            menu_hover(editor()),
            // The press's menu hover of an older agent, after the rest on the same entry.
            menu_hover(editor()),
            click(editor()),
        ];
        for (index, event) in events.into_iter().enumerate() {
            feed(&mut recorder, DESKTOP, &[MOBILE], 10 * index as u64, event);
        }
        recorder.stop().expect("stops")
    };
    assert_eq!(recording.steps.len(), 4, "{:?}", kinds(&recording));
    assert!(matches!(recording.steps[3].action, Action::Click { .. }));
    let ids: Vec<u32> = recording.steps.iter().map(|step| step.id).collect();
    assert_eq!(ids, [1, 2, 3, 4]);
}

#[test]
fn a_move_the_agent_did_not_mark_stays_out_and_off_keeps_no_hover() {
    let mut recorder = started(RecorderOptions::default(), None);
    feed(&mut recorder, DESKTOP, &[MOBILE], 0, hover(products()));
    feed(&mut recorder, DESKTOP, &[MOBILE], 10, hover(careers()));
    assert_eq!(recorder.step_count(), 0);
    let off = RecorderOptions {
        record_hover: HoverCapture::Off,
        ..RecorderOptions::default()
    };
    let mut recorder = started(off, None);
    feed(&mut recorder, DESKTOP, &[MOBILE], 0, menu_hover(products()));
    assert_eq!(recorder.step_count(), 0);
}

#[test]
fn the_way_goes_with_the_rest_and_with_the_click() {
    let mut recorder = started(RecorderOptions::default(), None);
    feed_way(&mut recorder, 0, menu_hover(products()), way(3), true);
    feed_way(&mut recorder, 400, click(software()), way(5), true);
    feed_way(&mut recorder, 800, click(careers()), Vec::new(), true);
    let recording = recorder.stop().expect("stops");
    assert_eq!(recording.steps[0].path, way(3));
    assert_eq!(recording.steps[1].path, way(5));
    assert!(recording.steps[2].path.is_empty());
}

#[test]
fn a_double_click_keeps_its_first_clicks_way() {
    let mut recorder = started(RecorderOptions::default(), None);
    feed_way(&mut recorder, 0, click(careers()), way(4), true);
    feed_way(&mut recorder, 50, click(careers()), Vec::new(), true);
    let dblclick = crate::event::SyncEvent::Dblclick(crate::event::MouseData {
        control: None,
        button: crate::event::MouseButton::Left,
        detail: 2,
        modifiers: 0,
        target: at(careers()),
    });
    feed_way(&mut recorder, 60, dblclick, Vec::new(), true);
    let recording = recorder.stop().expect("stops");
    assert_eq!(kinds(&recording), ["dblclick #nav > li:nth-child(3) > a"]);
    assert_eq!(recording.steps[0].path, way(4));
}

#[test]
fn a_way_keeps_its_last_points_and_none_comes_from_an_iframe() {
    let mut recorder = started(RecorderOptions::default(), None);
    feed_way(&mut recorder, 0, menu_hover(products()), way(100), true);
    feed_way(&mut recorder, 400, menu_hover(careers()), way(3), false);
    let recording = recorder.stop().expect("stops");
    let kept = &recording.steps[0].path;
    assert_eq!(kept.len(), MAX_PATH_POINTS);
    assert_eq!(kept.last(), way(100).last());
    assert_eq!(
        kept.first(),
        Some(&PathPoint {
            x: 360,
            y: 5,
            t: 720
        })
    );
    assert!(recording.steps[1].path.is_empty());
}

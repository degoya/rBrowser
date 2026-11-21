//! RB-090-93: a click on the label of a checkbox or radio stays the step; the control's `change`
//! is part of it, and a view whose replay of the click found no label drops out of `onlyIn`.

use crate::event::{ElementSelector, MouseButton, MouseData, SyncEvent};
use crate::recorder::model::{Action, Recording};
use crate::recorder::record::{Recorder, RecorderOptions};
use crate::recorder::testing::*;
use crate::replay::ReplayOutcome;

const LABEL_PATH: &str = "header > label";

/// The burger: a label whose only text is a visually hidden "Menu".
fn burger() -> ElementSelector {
    ElementSelector::Text {
        tag: "label".into(),
        text: "Menu".into(),
        nth: 0,
        visible_nth: None,
        within: None,
        path: LABEL_PATH.into(),
    }
}

/// The agent's click on a label of the checkbox or radio at `control`.
fn label_click(control: &str) -> SyncEvent {
    SyncEvent::Click(MouseData {
        button: MouseButton::Left,
        detail: 1,
        modifiers: 0,
        target: at(burger()),
        control: Some(control.into()),
    })
}

/// The `input` a checkbox fires: no text, the value `on`.
fn toggled(element: ElementSelector) -> SyncEvent {
    SyncEvent::Input(crate::event::InputData {
        input_type: String::new(),
        data: None,
        value: Some("on".into()),
        sensitive: false,
        target: at(element),
    })
}

fn record(events: Vec<SyncEvent>) -> Recording {
    let mut recorder = started(RecorderOptions::default(), None);
    for (i, event) in events.into_iter().enumerate() {
        feed(&mut recorder, DESKTOP, &[MOBILE], 10 * i as u64, event);
    }
    recorder.stop().expect("stops")
}

fn summary(recording: &Recording) -> Vec<String> {
    recording
        .steps
        .iter()
        .map(|step| match &step.action {
            Action::Click { target, .. } => format!("click {}", target.css_path()),
            Action::Check { target } => format!("check {}", target.css_path()),
            Action::Uncheck { target } => format!("uncheck {}", target.css_path()),
            Action::Fill { target, .. } => format!("fill {}", target.css_path()),
            other => format!("{other:?}"),
        })
        .collect()
}

#[test]
fn a_label_click_and_its_checkbox_change_are_one_click() {
    let toggle = role("checkbox", "Menu", "#nav-toggle");
    let recording = record(vec![
        label_click("#nav-toggle"),
        toggled(toggle.clone()),
        checked(toggle, true),
    ]);
    assert_eq!(summary(&recording), ["click header > label"]);
    assert!(recording.steps[0].only_in.is_empty());
}

#[test]
fn the_fill_of_an_unnamed_checkbox_goes_into_the_label_click_too() {
    let toggle = css("#agree");
    let recording = record(vec![
        label_click("#agree"),
        toggled(toggle.clone()),
        checked(toggle, true),
    ]);
    assert_eq!(summary(&recording), ["click header > label"]);
    let ids: Vec<u32> = recording.steps.iter().map(|step| step.id).collect();
    assert_eq!(ids, [1]);
}

#[test]
fn a_change_of_another_control_is_its_own_step() {
    let recording = record(vec![
        label_click("#nav-toggle"),
        checked(css("#newsletter"), true),
    ]);
    assert_eq!(
        summary(&recording),
        ["click header > label", "check #newsletter"]
    );
}

#[test]
fn a_click_without_a_control_keeps_the_change_as_a_check() {
    // A recording's events from an agent before RB-090-93: the label's click names no control.
    let recording = record(vec![click(burger()), checked(css("#nav-toggle"), true)]);
    assert_eq!(
        summary(&recording),
        ["click header > label", "check #nav-toggle"]
    );
}

#[test]
fn the_control_clicked_itself_after_its_label_is_a_check_again() {
    let toggle = css("#nav-toggle");
    let recording = record(vec![
        label_click("#nav-toggle"),
        checked(toggle.clone(), true),
        click(toggle.clone()),
        checked(toggle, false),
    ]);
    assert_eq!(
        summary(&recording),
        ["click header > label", "uncheck #nav-toggle"]
    );
}

#[test]
fn a_second_change_after_the_label_click_is_a_step() {
    // Only the change the label's click caused belongs to it.
    let toggle = css("#nav-toggle");
    let recording = record(vec![
        label_click("#nav-toggle"),
        checked(toggle.clone(), true),
        checked(toggle, false),
    ]);
    assert_eq!(
        summary(&recording),
        ["click header > label", "uncheck #nav-toggle"]
    );
}

fn label_step_views(outcome_after_change: bool, failed: &SyncEvent) -> Vec<String> {
    let mut recorder: Recorder = started(RecorderOptions::default(), None);
    feed(
        &mut recorder,
        DESKTOP,
        &[MOBILE],
        0,
        label_click("#nav-toggle"),
    );
    if !outcome_after_change {
        recorder.on_replayed(MOBILE, failed, &ReplayOutcome::NotVisible);
    }
    feed(
        &mut recorder,
        DESKTOP,
        &[MOBILE],
        6,
        checked(css("#nav-toggle"), true),
    );
    if outcome_after_change {
        recorder.on_replayed(MOBILE, failed, &ReplayOutcome::NotVisible);
    }
    let recording = recorder.stop().expect("stops");
    assert_eq!(summary(&recording), ["click header > label"]);
    recording.steps[0].only_in.clone()
}

#[test]
fn a_view_whose_replay_found_no_label_drops_out_of_the_click() {
    // The label is hidden at the wider breakpoint: its replay there ends `not_visible`, before or
    // after the change arrived.
    let label = label_click("#nav-toggle");
    assert_eq!(label_step_views(false, &label), ["desktop"]);
    assert_eq!(label_step_views(true, &label), ["desktop"]);
}

#[test]
fn a_view_whose_replay_of_the_change_failed_drops_out_too() {
    let change = checked(css("#nav-toggle"), true);
    assert_eq!(label_step_views(true, &change), ["desktop"]);
}

#[test]
fn a_later_change_of_the_control_after_a_label_click_that_changed_nothing_is_its_own_step() {
    // The page cancelled the label's toggle (or its radio was checked already): no change in
    // the click's task. Space on the focused control changes it later - a step of its own.
    let agree = css("#agree");
    let recording = record(vec![
        label_click("#agree"),
        key(" ", 0, Some(agree.clone())),
        checked(agree, true),
    ]);
    assert_eq!(
        summary(&recording),
        ["click header > label", "check #agree"]
    );
}

#[test]
fn any_other_event_after_the_label_click_ends_it() {
    let plan = css("#plan-a");
    let recording = record(vec![
        label_click("#plan-a"),
        hover(css("#pricing")),
        checked(plan, true),
    ]);
    assert_eq!(
        summary(&recording),
        ["click header > label", "check #plan-a"]
    );
}

#[test]
fn the_focus_and_input_of_the_control_keep_its_change_in_the_label_click() {
    // The engine focuses the control and fires its `input` before the `change`, in one task.
    let toggle = role("checkbox", "Menu", "#nav-toggle");
    let recording = record(vec![
        label_click("#nav-toggle"),
        SyncEvent::Focus(crate::event::TargetData {
            target: at(toggle.clone()),
        }),
        toggled(toggle.clone()),
        checked(toggle, true),
    ]);
    assert_eq!(summary(&recording), ["click header > label"]);
}

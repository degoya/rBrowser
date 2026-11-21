use super::*;
use crate::recorder::model::Action;
use crate::recorder::record::RecorderOptions;
use crate::recorder::testing::*;

fn value(step: &Step) -> Option<&str> {
    match &step.action {
        Action::Fill { value, .. } => Some(value),
        _ => None,
    }
}

#[test]
fn the_changed_steps_come_from_the_first_place_that_changed() {
    let mut recorder = started(RecorderOptions::default(), Some("http://localhost:3000/"));
    let (from, steps) = recorder.take_changed().expect("the start's navigation");
    assert_eq!((from, steps.len()), (0, 1));
    assert!(recorder.take_changed().is_none(), "nothing new");

    let name = css("#name");
    feed(
        &mut recorder,
        DESKTOP,
        &[MOBILE],
        100,
        input(name.clone(), "Ad", false),
    );
    let (from, steps) = recorder.take_changed().expect("a fill");
    assert_eq!((from, steps.len()), (1, 1));
    // The fill grows in place: the same step comes again, alone.
    feed(
        &mut recorder,
        DESKTOP,
        &[MOBILE],
        200,
        input(name, "Ada", false),
    );
    let (from, steps) = recorder.take_changed().expect("the fill grew");
    assert_eq!(from, 1);
    assert_eq!(steps.iter().map(value).collect::<Vec<_>>(), [Some("Ada")]);
    assert!(recorder.take_changed().is_none());
}

#[test]
fn a_step_changed_in_place_is_a_change_until_it_is_taken() {
    let mut recorder = started(RecorderOptions::default(), None);
    let name = css("#name");
    feed(
        &mut recorder,
        DESKTOP,
        &[MOBILE],
        100,
        input(name.clone(), "Ad", false),
    );
    recorder.take_changed();
    assert!(!recorder.changed());
    feed(
        &mut recorder,
        DESKTOP,
        &[MOBILE],
        200,
        input(name, "Ada", false),
    );
    assert_eq!(recorder.step_count(), 1, "the same step");
    assert!(recorder.changed(), "asking keeps it");
    assert!(recorder.changed());
    assert_eq!(recorder.take_changed().map(|(from, _)| from), Some(0));
    recorder.on_navigated(DESKTOP, "http://localhost:3000/a", START + 300);
    recorder.forget_changes();
    assert!(!recorder.changed());
}

#[test]
fn the_steps_from_a_place_are_the_snapshots() {
    let mut recorder = started(RecorderOptions::default(), Some("http://localhost:3000/"));
    feed(&mut recorder, DESKTOP, &[MOBILE], 100, click(css("#menu")));
    let all = recorder.snapshot().expect("records").steps;
    assert_eq!(recorder.steps_from(0), all);
    assert_eq!(recorder.steps_from(1), all[1..]);
    assert!(recorder.steps_from(9).is_empty(), "past the end");
}

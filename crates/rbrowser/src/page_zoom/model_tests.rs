use rb_config::PageZoomSettings;
use rb_engine::ViewId;

use super::*;

const A: ViewId = ViewId(1);
const B: ViewId = ViewId(2);
const C: ViewId = ViewId(3);

fn settings(wheel: bool, sync: bool) -> PageZoomSettings {
    PageZoomSettings {
        wheel,
        sync,
        ..PageZoomSettings::default()
    }
}

fn percents(sets: &[(ViewId, f64)]) -> Vec<(ViewId, u32)> {
    sets.iter()
        .map(|(view, level)| (*view, percent_of(*level)))
        .collect()
}

#[test]
fn a_level_is_chromiums_power_of_1_2() {
    assert_eq!(percent_of(0.0), 100);
    assert_eq!(percent_of(1.0), 120);
    assert_eq!(percent_of(-1.0), 83);
    assert_eq!(percent_of(level_of(1.1)), 110);
    assert_eq!(percent_of(level_of(0.25)), 25);
    assert!((factor_of(level_of(1.75)) - 1.75).abs() < 1e-9);
}

#[test]
fn only_a_level_away_from_zero_is_zoomed() {
    assert!(!is_zoomed(0.0));
    assert!(!is_zoomed(0.000_1), "Chromium's own epsilon");
    assert!(!is_zoomed(f64::NAN), "no browser to read");
    assert!(is_zoomed(level_of(1.1)));
    assert!(is_zoomed(level_of(0.9)));
}

#[test]
fn a_step_goes_to_chromiums_next_preset_and_stays_at_the_ends() {
    let up = |factor: f64| factor_of(step(level_of(factor), true));
    let down = |factor: f64| factor_of(step(level_of(factor), false));
    assert!((up(1.0) - 1.1).abs() < 1e-9);
    assert!((up(1.1) - 1.25).abs() < 1e-9);
    assert!((down(1.0) - 0.9).abs() < 1e-9);
    assert!((down(0.5) - 1.0 / 3.0).abs() < 1e-9);
    // Between two presets (Ctrl+wheel), the next one in that direction.
    assert!((up(1.2) - 1.25).abs() < 1e-9);
    assert!((down(1.2) - 1.1).abs() < 1e-9);
    // At the ends the level stays.
    assert!((up(5.0) - 5.0).abs() < 1e-9);
    assert!((down(0.25) - 0.25).abs() < 1e-9);
}

#[test]
fn the_browser_keys_map_to_the_three_actions() {
    const CTRL: u32 = 1 << 2;
    const SHIFT: u32 = 1 << 1;
    const ALT: u32 = 1 << 3;
    assert_eq!(key_action(CTRL, 0x30), Some(Action::Reset), "Ctrl+0");
    assert_eq!(key_action(CTRL, 0x60), Some(Action::Reset), "Ctrl+Num0");
    assert_eq!(key_action(CTRL, 0xBB), Some(Action::In), "Ctrl+Plus");
    assert_eq!(key_action(CTRL, 0x6B), Some(Action::In), "Ctrl+Num+");
    assert_eq!(
        key_action(CTRL | SHIFT, 0xBB),
        Some(Action::In),
        "Ctrl+Shift+="
    );
    assert_eq!(key_action(CTRL, 0xBD), Some(Action::Out), "Ctrl+Minus");
    assert_eq!(key_action(CTRL, 0x6D), Some(Action::Out), "Ctrl+Num-");
    assert_eq!(key_action(0, 0x30), None, "0 alone types");
    assert_eq!(
        key_action(CTRL | SHIFT, 0x30),
        None,
        "Ctrl+Shift+0 is rBrowser's"
    );
    assert_eq!(key_action(CTRL | ALT, 0x30), None, "AltGr");
    assert_eq!(key_action(CTRL, 0x31), None, "Ctrl+1");
}

#[test]
fn a_plan_without_sync_moves_each_target_from_its_own_level() {
    let mut levels = Levels::default();
    levels.record(&[(A, level_of(1.1)), (B, level_of(0.9))]);
    let sets = levels.plan(Action::In, &[A, B], None);
    assert_eq!(percents(&sets), vec![(A, 125), (B, 100)]);
    let reset = levels.plan(Action::Reset, &[A], None);
    assert_eq!(percents(&reset), vec![(A, 100)]);
    let set = levels.plan(Action::Set(level_of(1.5)), &[B], None);
    assert_eq!(percents(&set), vec![(B, 150)]);
}

#[test]
fn a_plan_with_sync_moves_every_view_from_the_anchors_level() {
    let mut levels = Levels::default();
    levels.record(&[(A, level_of(1.1)), (B, 0.0)]);
    let sets = levels.plan(Action::In, &[A], Some(&[A, B, C]));
    assert_eq!(percents(&sets), vec![(A, 125), (B, 125), (C, 125)]);
    let sets = levels.plan(Action::Out, &[B], Some(&[A, B, C]));
    assert_eq!(percents(&sets), vec![(A, 90), (B, 90), (C, 90)]);
}

#[test]
fn the_first_sight_of_a_view_is_taken_as_it_is() {
    // Chromium brings a site's zoom back from an earlier run: nothing to put back.
    let mut levels = Levels::default();
    let sets = levels.observe(A, level_of(1.25), &settings(false, true), &[A, B]);
    assert!(sets.is_empty());
    assert_eq!(percent_of(levels.get(A)), 125);
}

#[test]
fn a_wheel_zoom_is_put_back_while_the_wheel_is_off() {
    let mut levels = Levels::default();
    levels.record(&[(A, 0.0)]);
    let sets = levels.observe(A, level_of(1.1), &settings(false, false), &[A, B]);
    assert_eq!(percents(&sets), vec![(A, 100)]);
    assert_eq!(
        percent_of(levels.get(A)),
        100,
        "the model keeps the level it set"
    );
}

#[test]
fn a_wheel_zoom_reaches_every_view_with_sync() {
    let mut levels = Levels::default();
    levels.record(&[(A, 0.0), (B, 0.0), (C, level_of(1.1))]);
    let sets = levels.observe(A, level_of(1.1), &settings(true, true), &[A, B, C]);
    assert_eq!(percents(&sets), vec![(B, 110)], "C is there already");
    assert_eq!(percent_of(levels.get(B)), 110);
    // Without sync the change stays the view's own.
    let sets = levels.observe(A, level_of(1.25), &settings(true, false), &[A, B, C]);
    assert!(sets.is_empty());
    assert_eq!(percent_of(levels.get(A)), 125);
}

#[test]
fn an_unchanged_level_asks_for_nothing() {
    let mut levels = Levels::default();
    levels.record(&[(A, level_of(1.1))]);
    let sets = levels.observe(A, level_of(1.1) + 1e-6, &settings(false, true), &[A, B]);
    assert!(sets.is_empty());
}

#[test]
fn a_load_starts_at_100_percent_unless_the_zoom_is_kept() {
    let mut levels = Levels::default();
    levels.record(&[(A, level_of(1.1))]);
    let keep = settings(true, false);
    assert!(levels.loaded(A, level_of(1.1), &keep).is_empty());
    let fresh = PageZoomSettings {
        keep_on_reload: false,
        ..PageZoomSettings::default()
    };
    let sets = levels.loaded(A, level_of(1.1), &fresh);
    assert_eq!(percents(&sets), vec![(A, 100)]);
    assert!(levels.loaded(A, 0.0, &fresh).is_empty(), "at 100 % already");
}

#[test]
fn the_views_that_went_are_forgotten() {
    let mut levels = Levels::default();
    levels.record(&[(A, 1.0), (B, 1.0)]);
    levels.retain(&[B]);
    assert_eq!(levels.get(A), 0.0);
    assert_eq!(levels.get(B), 1.0);
}

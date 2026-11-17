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

// RB-090-69: a key a Firefox window's agent reports (DOM `key` and `code`, the sync protocol's
// modifier bits) means what the same key in a Chromium page does.

/// A row of the key table: modifiers, key, code, Firefox's keyCode, the action, the case.
type KeyRow = (
    u8,
    &'static str,
    &'static str,
    u32,
    Option<Action>,
    &'static str,
);
#[test]
fn a_page_reported_key_maps_like_the_browser_key() {
    use rb_sync::{MOD_ALT, MOD_CTRL, MOD_META, MOD_SHIFT};
    const CTRL: u8 = MOD_CTRL;
    const CTRL_SHIFT: u8 = MOD_CTRL | MOD_SHIFT;
    let (reset, zoom_in, zoom_out) = (Some(Action::Reset), Some(Action::In), Some(Action::Out));
    // (modifiers, key, code, keyCode as Firefox reports it, what it does, the case)
    let table: [KeyRow; 26] = [
        (CTRL, "0", "Digit0", 48, reset, "Ctrl+0"),
        (CTRL, "0", "Numpad0", 96, reset, "Ctrl+Num0"),
        (
            CTRL,
            "\u{e0}",
            "Digit0",
            48,
            reset,
            "AZERTY Ctrl+0: the key of the 0",
        ),
        (
            CTRL | MOD_META,
            "0",
            "Digit0",
            48,
            reset,
            "Meta beside Ctrl changes nothing",
        ),
        (CTRL, "=", "Equal", 61, zoom_in, "US Ctrl+="),
        (CTRL_SHIFT, "+", "Equal", 61, zoom_in, "US Ctrl+Shift+="),
        (CTRL, "+", "BracketRight", 171, zoom_in, "German Ctrl+Plus"),
        (
            CTRL_SHIFT,
            "*",
            "BracketRight",
            171,
            zoom_in,
            "German Ctrl+Shift+Plus",
        ),
        (CTRL, "+", "NumpadAdd", 107, zoom_in, "Ctrl+Num+"),
        (CTRL, "=", "Equal", 187, zoom_in, "a Chromium keyCode"),
        (CTRL, "-", "Minus", 173, zoom_out, "US Ctrl+Minus"),
        (
            CTRL_SHIFT,
            "_",
            "Minus",
            173,
            zoom_out,
            "US Ctrl+Shift+Minus",
        ),
        (CTRL, "-", "Slash", 173, zoom_out, "German Ctrl+Minus"),
        (
            CTRL_SHIFT,
            "_",
            "Slash",
            173,
            zoom_out,
            "German Ctrl+Shift+Minus",
        ),
        (CTRL, "-", "NumpadSubtract", 109, zoom_out, "Ctrl+Num-"),
        (CTRL, "-", "Minus", 189, zoom_out, "a Chromium keyCode"),
        (CTRL, "+", "Equal", 0, zoom_in, "no keyCode: the code"),
        (
            CTRL_SHIFT,
            "_",
            "Minus",
            0,
            zoom_out,
            "no keyCode, Shift: the code",
        ),
        (
            CTRL,
            "-",
            "Unidentified",
            0,
            zoom_out,
            "no keyCode, no code: the key",
        ),
        (0, "0", "Digit0", 48, None, "0 alone types"),
        (
            CTRL_SHIFT,
            "=",
            "Digit0",
            48,
            None,
            "German Ctrl+Shift+0 is rBrowser's",
        ),
        (
            CTRL_SHIFT,
            ")",
            "Digit0",
            48,
            None,
            "US Ctrl+Shift+0 is rBrowser's",
        ),
        (CTRL | MOD_ALT, "0", "Digit0", 48, None, "AltGr"),
        (MOD_META, "0", "Digit0", 48, None, "Meta is no Ctrl"),
        (MOD_META, "-", "Minus", 173, None, "Meta+Minus is no zoom"),
        (CTRL, "1", "Digit1", 49, None, "Ctrl+1"),
    ];
    for (modifiers, key, code, key_code, expected, case) in table {
        assert_eq!(
            page_key_action(modifiers, key, code, key_code),
            expected,
            "{case}"
        );
    }
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

/// One reading after rBrowser set `level` on a view: the view, its level and its page's URL.
fn reading(view: ViewId, level: f64, url: &str) -> Reading {
    Reading {
        view,
        level,
        url: url.to_owned(),
    }
}

// RB-090-28: Chromium keeps a zoom per host. A view of another site that the user zoomed with
// Ctrl+wheel to the very same level in the same reading is the user's own zoom - observed, not
// dropped as a follower - so its level reaches the model and `views.pageZoom.sync`.
#[test]
fn a_follower_is_a_view_of_the_same_site_not_only_of_the_same_level() {
    let level = 1.0;
    let read = vec![
        reading(A, level, "https://shop.example/cart"),
        reading(B, level, "https://shop.example:8443/checkout"),
        reading(C, level, "https://blog.example/"),
    ];
    let (followers, others) = split_followers(level, "https://shop.example/", read);
    assert_eq!(followers, vec![(A, level), (B, level)]);
    assert_eq!(others, vec![(C, level)]);
}

#[test]
fn a_view_at_another_level_is_observed_whatever_its_site() {
    let read = vec![
        reading(A, 1.0, "https://shop.example/"),
        reading(B, -1.0, "https://shop.example/other"),
    ];
    let (followers, others) = split_followers(1.0, "https://shop.example/", read);
    assert_eq!(followers, vec![(A, 1.0)]);
    assert_eq!(others, vec![(B, -1.0)]);
}

// A page without a host (`file:`, `about:blank`) shares its zoom only with the very same URL,
// as Chromium keys it by the whole URL then.
#[test]
fn a_page_without_a_host_follows_only_the_same_url() {
    let read = vec![
        reading(A, 1.0, "file:///C:/site/index.html"),
        reading(B, 1.0, "file:///C:/site/other.html"),
        reading(C, 1.0, "HTTPS://Shop.Example/"),
    ];
    let (followers, others) = split_followers(1.0, "file:///C:/site/index.html", read);
    assert_eq!(followers, vec![(A, 1.0)]);
    assert_eq!(others, vec![(B, 1.0), (C, 1.0)]);
    let read = vec![reading(C, 1.0, "HTTPS://Shop.Example/")];
    let (followers, others) = split_followers(1.0, "https://shop.example/x", read);
    assert_eq!(followers, vec![(C, 1.0)]);
    assert!(others.is_empty());
}

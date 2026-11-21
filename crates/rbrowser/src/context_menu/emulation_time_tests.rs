//! The Clock and Animations submenus (RB-094-07, `super::super::time_control`): the clock of
//! every view as one choice, the clicked view's rate marked among the settings' rates, Step
//! while stopped, 100 % for every view - each `emulation.clock` or `overlays.animations`.

use rb_config::{ClockUserSettings, EmulationUserSettings};
use serde_json::json;

use super::tests::{action, emulation, entry, state, submenu, text};
use super::*;
use crate::context_menu::model::build;
use crate::context_menu::time_control::TimeMenu;

fn rate_text(percent: &str) -> String {
    format!(
        "'{}'",
        rb_i18n::t_with("contextmenu.time.rate", &[("percent", percent)])
    )
}

fn menus(time: TimeMenu) -> Vec<Item> {
    let mut emulation = emulation();
    emulation.time = time;
    build(&state(Some(emulation)))
}

/// `(label, enabled, mark)` of each entry of `items`, separators as `-`.
fn summary(items: &[Item]) -> Vec<(String, bool, Mark)> {
    items
        .iter()
        .map(|item| match item {
            Item::Entry(entry) => (text(&entry.label), entry.enabled, entry.mark),
            Item::Separator => ("-".to_owned(), false, Mark::None),
            Item::Menu { label, .. } => (text(label), true, Mark::None),
        })
        .collect()
}

#[test]
fn the_real_time_is_chosen_until_a_clock_runs() {
    let items = menus(TimeMenu::default());
    let block = submenu(&items, "shell.menu.emulation");
    let clock = submenu(block, "contextmenu.time.clock");
    assert_eq!(
        summary(clock),
        [
            (
                "contextmenu.time.real_time".to_owned(),
                true,
                Mark::Radio(true)
            ),
            (
                "contextmenu.time.freeze_now".to_owned(),
                true,
                Mark::Radio(false)
            ),
        ],
        "the default settings' clock is the time stopped now: offered once"
    );
    assert_eq!(
        action(entry(clock, "contextmenu.time.real_time")),
        ("emulation.clock".to_owned(), json!({ "reset": true }))
    );
    assert_eq!(
        action(entry(clock, "contextmenu.time.freeze_now")),
        (
            "emulation.clock".to_owned(),
            json!({ "time": "now", "speed": 0 })
        )
    );
}

#[test]
fn a_running_clock_is_shown_chosen_and_the_settings_clock_offered() {
    let settings = EmulationUserSettings {
        clock: ClockUserSettings {
            default: "2026-12-24T10:00".to_owned(),
            speed: 2.5,
        },
        ..EmulationUserSettings::default()
    };
    let time = TimeMenu::with(
        Some(("2026-12-24T10:00:00".to_owned(), 100)),
        1.0,
        &settings,
    );
    let items = menus(time);
    let block = submenu(&items, "shell.menu.emulation");
    let clock = submenu(block, "contextmenu.time.clock");
    let shown = summary(clock);
    assert_eq!(shown.len(), 4);
    assert_eq!(
        (shown[0].1, shown[0].2),
        (false, Mark::Radio(true)),
        "the running clock: chosen, nothing to choose"
    );
    let running = rb_i18n::t_with(
        "contextmenu.time.clock_running",
        &[("start", "2026-12-24T10:00:00"), ("speed", "1")],
    );
    assert_eq!(shown[0].0, format!("'{running}'"));
    assert_eq!(
        shown[1].2,
        Mark::Radio(false),
        "the real time is not chosen"
    );
    assert_eq!(
        action(entry(clock, "contextmenu.time.settings_clock")),
        (
            "emulation.clock".to_owned(),
            json!({ "time": "2026-12-24T10:00", "speed": 2.5 })
        )
    );
}

#[test]
fn the_animations_offer_the_settings_rates_for_the_clicked_view() {
    let items = menus(TimeMenu::default());
    let block = submenu(&items, "shell.menu.emulation");
    let animations = submenu(block, "contextmenu.time.animations");
    assert_eq!(
        summary(animations),
        [
            (
                "contextmenu.time.stopped".to_owned(),
                true,
                Mark::Radio(false)
            ),
            (rate_text("10"), true, Mark::Radio(false)),
            (rate_text("25"), true, Mark::Radio(false)),
            (rate_text("100"), true, Mark::Radio(true)),
            ("-".to_owned(), false, Mark::None),
            ("contextmenu.time.step".to_owned(), false, Mark::None),
            ("contextmenu.time.all_normal".to_owned(), true, Mark::None),
        ],
        "100 % chosen; Step only while stopped"
    );
    assert_eq!(
        action(entry(animations, "contextmenu.time.stopped")),
        (
            "overlays.animations".to_owned(),
            json!({ "target": 2, "rate": 0.0 })
        )
    );
    assert_eq!(
        action(entry(animations, &rate_text("10"))),
        (
            "overlays.animations".to_owned(),
            json!({ "target": 2, "rate": 0.1 })
        )
    );
    assert_eq!(
        action(entry(animations, "contextmenu.time.all_normal")),
        (
            "overlays.animations".to_owned(),
            json!({ "target": "all", "rate": 1 })
        )
    );
}

#[test]
fn a_stopped_view_can_step_and_a_rate_set_elsewhere_is_shown() {
    let stopped = TimeMenu::with(None, 0.0, &EmulationUserSettings::default());
    let items = menus(stopped);
    let block = submenu(&items, "shell.menu.emulation");
    let animations = submenu(block, "contextmenu.time.animations");
    assert_eq!(
        entry(animations, "contextmenu.time.stopped").mark,
        Mark::Radio(true)
    );
    let step = entry(animations, "contextmenu.time.step");
    assert!(step.enabled);
    assert_eq!(
        action(step),
        (
            "overlays.animations".to_owned(),
            json!({ "target": 2, "step": true })
        )
    );
    let half = TimeMenu::with(None, 0.5, &EmulationUserSettings::default());
    let items = menus(half);
    let block = submenu(&items, "shell.menu.emulation");
    let animations = submenu(block, "contextmenu.time.animations");
    assert_eq!(entry(animations, &rate_text("50")).mark, Mark::Radio(true));
}

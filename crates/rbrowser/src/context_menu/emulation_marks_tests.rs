//! How the Emulation submenu marks its entries: the clicked view's state, one colour scheme
//! choice of three, and "All views" by what every view has (RB-080-49).

use rb_config::{ColorScheme, EmulationUserSettings};
use serde_json::json;

use super::tests::{action, emulated, emulation, entry, state, submenu, text};
use super::*;
use crate::context_menu::model::build;

fn radio_group(items: &[Item]) -> Vec<(String, bool)> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Entry(entry) => match entry.mark {
                Mark::Radio(chosen) => Some((text(&entry.label), chosen)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn the_active_state_is_marked_and_a_switch_turns_it_off() {
    let state_of = EmulationState::of(Some(&emulated()), &[], &EmulationUserSettings::default());
    assert_eq!(state_of.preset.as_deref(), Some("tokyo"));
    let items = build(&state(Some(state_of)));
    let block = submenu(&items, "shell.menu.emulation");
    assert_eq!(
        radio_group(block),
        [
            ("emulation.toolbar.light".to_owned(), false),
            ("emulation.toolbar.dark".to_owned(), true),
            ("contextmenu.emulation.scheme_none".to_owned(), false),
        ],
        "one of three"
    );
    let motion = entry(block, "emulation.toolbar.reducedMotion");
    assert_eq!(motion.mark, Mark::Check(true));
    assert_eq!(
        action(motion).1["fields"],
        json!({ "reducedMotion": false })
    );
    let offline = entry(block, "emulation.extras.offline");
    assert_eq!(offline.mark, Mark::Check(true));
    assert_eq!(action(offline).1["offline"], false);
    let location = submenu(block, "emulation.extras.geolocation");
    let chosen: Vec<_> = radio_group(location)
        .into_iter()
        .filter(|(_, chosen)| *chosen)
        .collect();
    assert_eq!(
        chosen,
        [("emulation.geolocation.preset.tokyo".to_owned(), true)]
    );
    // A zone outside the quick picks is shown, chosen.
    let timezone = submenu(block, "emulation.extras.timezone");
    let chosen: Vec<_> = radio_group(timezone)
        .into_iter()
        .filter(|(_, chosen)| *chosen)
        .collect();
    assert_eq!(chosen, [("'Asia/Kolkata'".to_owned(), true)]);
}

/// In every state of the view exactly one colour scheme choice is marked.
#[test]
fn exactly_one_colour_scheme_choice_is_marked() {
    for scheme in [None, Some(ColorScheme::Light), Some(ColorScheme::Dark)] {
        let mut emulation = emulation();
        emulation.color_scheme = scheme;
        let items = build(&state(Some(emulation)));
        let block = submenu(&items, "shell.menu.emulation");
        let marked = radio_group(block)
            .into_iter()
            .filter(|(_, chosen)| *chosen)
            .count();
        assert_eq!(marked, 1, "{scheme:?}");
    }
}

fn view(scheme: Option<ColorScheme>, motion: bool) -> rb_engine::Emulation {
    rb_engine::Emulation {
        color_scheme: scheme,
        reduced_motion: motion.then_some(true),
        ..rb_engine::Emulation::default()
    }
}

/// "All views" is marked by what every view has, never by the clicked one: mixed is unmarked
/// and the choice turns the switch on for all; all on, it turns it off.
#[test]
fn all_views_are_marked_by_what_every_view_has() {
    let settings = EmulationUserSettings::default();
    let dark = view(Some(ColorScheme::Dark), true);
    let plain = view(None, false);
    let all_views = |views: &[rb_engine::Emulation]| {
        // The clicked view is the first: dark, reduced motion on.
        let items = build(&state(Some(EmulationState::of(
            Some(&dark),
            views,
            &settings,
        ))));
        let block = submenu(&items, "shell.menu.emulation");
        let all = submenu(block, "contextmenu.emulation.all_views").to_vec();
        let motion = entry(&all, "emulation.toolbar.reducedMotion").clone();
        (radio_group(&all), motion)
    };
    let (schemes, motion) = all_views(&[dark.clone(), plain.clone()]);
    assert!(
        schemes.iter().all(|(_, chosen)| !chosen),
        "mixed: {schemes:?}"
    );
    assert_eq!(motion.mark, Mark::Check(false));
    assert_eq!(
        action(&motion).1,
        json!({ "target": "all", "fields": { "reducedMotion": true } }),
        "mixed turns it on for all, whatever the clicked view has"
    );
    let (schemes, motion) = all_views(&[dark.clone(), dark.clone()]);
    assert_eq!(
        schemes,
        [
            ("emulation.toolbar.light".to_owned(), false),
            ("emulation.toolbar.dark".to_owned(), true),
            ("contextmenu.emulation.scheme_none".to_owned(), false),
        ]
    );
    assert_eq!(motion.mark, Mark::Check(true));
    assert_eq!(
        action(&motion).1["fields"],
        json!({ "reducedMotion": false })
    );
    let (schemes, _) = all_views(&[plain.clone(), plain]);
    assert_eq!(
        schemes[2],
        ("contextmenu.emulation.scheme_none".to_owned(), true)
    );
}

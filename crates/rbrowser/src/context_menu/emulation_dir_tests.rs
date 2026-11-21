//! Text direction and pseudo-localisation in the Emulation submenu (RB-092-02): the page's own
//! direction or one forced, as one choice for the clicked view; the pseudo-localisation as a
//! switch; both again under All views.

use rb_config::{EmulationUserSettings, TextDirection};
use serde_json::json;

use super::tests::{action, entry, state, submenu, text};
use super::*;
use crate::context_menu::model::build;

fn emulated(dir: Option<TextDirection>) -> rb_engine::Emulation {
    rb_engine::Emulation {
        dir,
        ..rb_engine::Emulation::default()
    }
}

fn block_of(emulation: EmulationState) -> Vec<Item> {
    let items = build(&state(Some(emulation)));
    submenu(&items, "shell.menu.emulation").to_vec()
}

#[test]
fn the_direction_is_one_choice_and_the_views_is_marked() {
    let of = |dir| EmulationState::of(Some(&emulated(dir)), &[], &EmulationUserSettings::default());
    let block = block_of(of(Some(TextDirection::Rtl)));
    let choices: Vec<(String, Mark, serde_json::Value)> =
        submenu(&block, "contextmenu.emulation.dir")
            .iter()
            .filter_map(|item| match item {
                Item::Entry(entry) => Some((text(&entry.label), entry.mark, action(entry).1)),
                _ => None,
            })
            .collect();
    let set = |dir: serde_json::Value| json!({ "target": 2, "fields": { "dir": dir } });
    assert_eq!(
        choices,
        vec![
            (
                "contextmenu.emulation.dir_none".to_owned(),
                Mark::Radio(false),
                set(serde_json::Value::Null)
            ),
            (
                "contextmenu.emulation.dir_ltr".to_owned(),
                Mark::Radio(false),
                set(json!("ltr"))
            ),
            (
                "contextmenu.emulation.dir_rtl".to_owned(),
                Mark::Radio(true),
                set(json!("rtl"))
            ),
            (
                "contextmenu.emulation.dir_auto".to_owned(),
                Mark::Radio(false),
                set(json!("auto"))
            ),
        ]
    );
}

#[test]
fn the_pseudo_localisation_switches_for_the_view_and_for_all() {
    let mut emulation = EmulationState::of(
        Some(&emulated(None)),
        &[],
        &EmulationUserSettings::default(),
    );
    emulation.pseudo = true;
    let block = block_of(emulation.clone());
    let pseudo = entry(&block, "contextmenu.emulation.pseudo");
    assert_eq!(pseudo.mark, Mark::Check(true));
    assert_eq!(
        action(pseudo),
        (
            crate::emulation::page_locale::PSEUDO_LOCALE.to_owned(),
            json!({ "target": 2, "on": false })
        )
    );
    let all = submenu(&block, "contextmenu.emulation.all_views").to_vec();
    assert_eq!(
        action(entry(&all, "contextmenu.emulation.pseudo")).1,
        json!({ "target": "all", "on": true }),
        "not every view: the choice turns it on for all"
    );
}

#[test]
fn right_to_left_for_all_views_is_marked_only_when_every_view_has_it() {
    let rtl = emulated(Some(TextDirection::Rtl));
    let mixed = AllViews::of(&[rtl.clone(), emulated(None)]);
    assert!(!mixed.rtl);
    let every = AllViews::of(&[rtl.clone(), rtl.clone()]);
    assert!(every.rtl);
    assert!(!AllViews::of(&[]).rtl);
    let emulation = EmulationState::of(
        Some(&rtl),
        &[rtl.clone(), rtl.clone()],
        &EmulationUserSettings::default(),
    );
    let block = block_of(emulation);
    let all = submenu(&block, "contextmenu.emulation.all_views").to_vec();
    let switch = entry(&all, "contextmenu.emulation.rtl");
    assert_eq!(switch.mark, Mark::Check(true));
    assert_eq!(
        action(switch).1,
        json!({ "target": "all", "fields": { "dir": null } }),
        "off is the page's own direction again"
    );
}

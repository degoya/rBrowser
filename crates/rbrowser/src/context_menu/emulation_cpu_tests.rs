//! CPU throttling in the Emulation submenu (RB-090-63): off and the toolbar's factors as one
//! choice, the clicked view's factor marked, each `view.emulate.cpu` with the factor.

use rb_config::EmulationUserSettings;
use serde_json::json;

use super::tests::{action, emulation, entry, state, submenu, text};
use super::*;
use crate::context_menu::model::build;

fn choices(throttling: Option<f64>) -> Vec<(String, bool, serde_json::Value)> {
    let emulated = rb_engine::Emulation {
        cpu_throttling: throttling,
        ..rb_engine::Emulation::default()
    };
    let state_of = EmulationState::of(Some(&emulated), &[], &EmulationUserSettings::default());
    let items = build(&state(Some(state_of)));
    let block = submenu(&items, "contextmenu.emulation.menu");
    submenu(block, "emulation.extras.cpu")
        .iter()
        .filter_map(|item| match item {
            Item::Entry(entry) => match entry.mark {
                Mark::Radio(chosen) => Some((text(&entry.label), chosen, action(entry).1)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn rate_label(rate: u32) -> String {
    format!(
        "'{}'",
        rb_i18n::t_with("emulation.extras.cpuRate", &[("rate", &rate.to_string())])
    )
}

#[test]
fn the_cpu_submenu_offers_off_and_the_toolbars_factors() {
    let shown = choices(None);
    let mut expected = vec![(
        "emulation.extras.off".to_owned(),
        true,
        json!({ "target": 2, "rate": 1 }),
    )];
    for rate in CPU_RATES {
        expected.push((
            rate_label(rate),
            false,
            json!({ "target": 2, "rate": rate }),
        ));
    }
    assert_eq!(shown, expected);
}

#[test]
fn the_views_factor_is_marked() {
    let shown = choices(Some(4.0));
    let marked: Vec<&str> = shown
        .iter()
        .filter(|(_, chosen, _)| *chosen)
        .map(|(label, _, _)| label.as_str())
        .collect();
    assert_eq!(marked, [rate_label(4).as_str()]);
}

#[test]
fn no_throttling_at_factor_one_and_another_factor_is_shown_marked() {
    assert!(choices(Some(1.0))[0].1, "1 is no throttling");
    let shown = choices(Some(3.0));
    assert!(!shown[0].1);
    let last = shown.last().expect("a choice");
    assert_eq!(
        last,
        &(rate_label(3), true, json!({ "target": 2, "rate": 3 }))
    );
}

#[test]
fn the_cpu_entry_runs_the_toolbars_command() {
    let items = build(&state(Some(emulation())));
    let block = submenu(&items, "contextmenu.emulation.menu");
    let cpu = submenu(block, "emulation.extras.cpu");
    assert_eq!(
        action(entry(cpu, "emulation.extras.off")).0,
        crate::emulation::extras::CPU
    );
}

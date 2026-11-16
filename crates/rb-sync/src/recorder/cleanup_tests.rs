use crate::event::{ChangeData, MOD_ALT, MOD_CTRL, MouseButton, MouseData, SyncEvent};
use crate::recorder::model::{Action, Recording};
use crate::recorder::record::RecorderOptions;
use crate::recorder::testing::*;

fn record(options: RecorderOptions, events: Vec<SyncEvent>) -> Recording {
    let mut recorder = started(options, None);
    for (i, event) in events.into_iter().enumerate() {
        feed(&mut recorder, DESKTOP, &[MOBILE], 10 * i as u64, event);
    }
    recorder.stop().expect("stops")
}

fn kinds(recording: &Recording) -> Vec<String> {
    recording
        .steps
        .iter()
        .map(|step| {
            let value = serde_json::to_value(&step.action).expect("serialises");
            let kind = value["type"].as_str().unwrap_or("?").to_owned();
            match &step.action {
                Action::Click { target, .. }
                | Action::Dblclick { target }
                | Action::Hover { target }
                | Action::Check { target }
                | Action::Uncheck { target }
                | Action::Select { target, .. }
                | Action::Fill { target, .. } => format!("{kind} {}", target.css_path()),
                Action::Press { key, .. } => format!("{kind} {key}"),
                _ => kind,
            }
        })
        .collect()
}

#[test]
fn a_click_on_a_checkbox_and_its_change_are_one_check() {
    let terms = css("#terms");
    let recording = record(
        RecorderOptions::default(),
        vec![
            click(terms.clone()),
            checked(terms.clone(), true),
            click(terms.clone()),
            checked(terms, false),
        ],
    );
    assert_eq!(kinds(&recording), ["check #terms", "uncheck #terms"]);
}

#[test]
fn a_select_change_takes_the_click_that_opened_it() {
    let size = css("#size");
    let select = SyncEvent::Change(ChangeData {
        value: Some("l".into()),
        checked: None,
        selected: Some(vec![2]),
        sensitive: false,
        target: at(size.clone()),
    });
    let recording = record(RecorderOptions::default(), vec![click(size), select]);
    assert_eq!(kinds(&recording), ["select #size"]);
    let Action::Select { value, indexes, .. } = &recording.steps[0].action else {
        panic!("a select");
    };
    assert_eq!(
        (value.as_deref(), indexes.as_slice()),
        (Some("l"), &[2][..])
    );
}

#[test]
fn two_clicks_and_a_dblclick_are_one_dblclick() {
    let cell = css("#cell");
    let dblclick = SyncEvent::Dblclick(MouseData {
        button: MouseButton::Left,
        detail: 2,
        modifiers: 0,
        target: at(cell.clone()),
    });
    let recording = record(
        RecorderOptions::default(),
        vec![click(cell.clone()), click(cell), dblclick],
    );
    assert_eq!(kinds(&recording), ["dblclick #cell"]);
}

#[test]
fn a_change_that_confirms_the_typed_value_adds_nothing() {
    let name = css("#name");
    let recording = record(
        RecorderOptions::default(),
        vec![
            input(name.clone(), "a", false),
            input(name.clone(), "ab", false),
            change(name, Some("ab"), false),
        ],
    );
    assert_eq!(kinds(&recording), ["fill #name"]);
}

#[test]
fn a_file_input_is_no_step() {
    let recording = record(
        RecorderOptions::default(),
        vec![change(css("#upload"), None, false)],
    );
    assert!(recording.steps.is_empty());
}

#[test]
fn only_named_keys_and_shortcuts_are_presses() {
    let field = Some(css("#q"));
    let recording = record(
        RecorderOptions::default(),
        vec![
            key("a", 0, field.clone()),
            key("A", crate::event::MOD_SHIFT, field.clone()),
            key("@", MOD_CTRL | MOD_ALT, field.clone()),
            key("Shift", 0, field.clone()),
            key("Backspace", 0, field.clone()),
            key("a", MOD_CTRL, field.clone()),
            key("Escape", 0, None),
            key("Enter", 0, field),
        ],
    );
    assert_eq!(
        kinds(&recording),
        ["press a", "press Escape", "press Enter"]
    );
}

#[test]
fn the_hover_that_opened_a_menu_stays_other_moves_go() {
    let products = role("link", "Products", "#products");
    let item = testid("product-alpha", "#desktop-menu > li.dropdown > ul > li > a");
    let recording = record(
        RecorderOptions::default(),
        vec![
            hover(css("main > p")),
            hover(products),
            hover(css("#desktop-menu > li.dropdown > ul > li")),
            hover(item.clone()),
            click(item),
        ],
    );
    assert_eq!(
        kinds(&recording),
        [
            "hover #products",
            "click #desktop-menu > li.dropdown > ul > li > a"
        ]
    );
}

#[test]
fn a_hover_on_the_clicked_element_or_with_hover_off_is_dropped() {
    let save = role("button", "Save", "#save");
    let recording = record(
        RecorderOptions::default(),
        vec![hover(save.clone()), click(save)],
    );
    assert_eq!(kinds(&recording), ["click #save"]);
    let off = RecorderOptions {
        record_hover: false,
        ..RecorderOptions::default()
    };
    let products = role("link", "Products", "#products");
    let recording = record(off, vec![hover(products), click(css("#alpha"))]);
    assert_eq!(kinds(&recording), ["click #alpha"]);
}

#[test]
fn a_hover_without_a_click_is_no_step() {
    let products = role("link", "Products", "#products");
    let recording = record(
        RecorderOptions::default(),
        vec![hover(products), input(css("#q"), "x", false)],
    );
    assert_eq!(kinds(&recording), ["fill #q"]);
}

#[test]
fn scroll_is_a_step_only_with_the_setting_and_coalesced() {
    let events = || vec![scroll(100.0), scroll(400.0), scroll(900.0)];
    assert!(
        record(RecorderOptions::default(), events())
            .steps
            .is_empty()
    );
    let on = RecorderOptions {
        record_scroll: true,
        ..RecorderOptions::default()
    };
    let recording = record(on, events());
    assert_eq!(kinds(&recording), ["scroll"]);
    let Action::Scroll { y, target, .. } = &recording.steps[0].action else {
        panic!("a scroll");
    };
    assert!(target.is_none());
    assert!((*y - 900.0).abs() < f64::EPSILON);
}

#[test]
fn pointer_focus_and_key_up_events_are_no_steps() {
    let recording = record(
        RecorderOptions::default(),
        vec![SyncEvent::Focus(crate::event::TargetData {
            target: at(css("#q")),
        })],
    );
    assert!(recording.steps.is_empty());
}

#[test]
fn step_ids_are_unique_and_rising() {
    let recording = record(
        RecorderOptions::default(),
        vec![click(css("#a")), click(css("#b")), key("Enter", 0, None)],
    );
    let ids: Vec<u32> = recording.steps.iter().map(|step| step.id).collect();
    assert_eq!(ids, [1, 2, 3]);
}

use crate::event::{ChangeData, MOD_ALT, MOD_CTRL, MouseButton, MouseData, SyncEvent};
use crate::recorder::model::{Action, Recording};
use crate::recorder::record::{HoverCapture, RecorderOptions};
use crate::recorder::testing::*;

fn record(options: RecorderOptions, events: Vec<SyncEvent>) -> Recording {
    let mut recorder = started(options, None);
    for (i, event) in events.into_iter().enumerate() {
        feed(&mut recorder, DESKTOP, &[MOBILE], 10 * i as u64, event);
    }
    recorder.stop().expect("stops")
}

/// `recordHover: all`: the recorder's guess keeps a hover too, not only the agent's menu hover.
fn every_hover() -> RecorderOptions {
    RecorderOptions {
        record_hover: HoverCapture::All,
        ..RecorderOptions::default()
    }
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
        control: None,
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
        every_hover(),
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
        record_hover: HoverCapture::Off,
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
        every_hover(),
        vec![hover(products), input(css("#q"), "x", false)],
    );
    assert_eq!(kinds(&recording), ["fill #q"]);
}

#[test]
fn scroll_is_a_step_only_with_the_setting_and_coalesced() {
    let events = || vec![scroll(100.0), scroll(400.0), scroll(900.0)];
    let off = RecorderOptions {
        record_scroll: false,
        ..RecorderOptions::default()
    };
    assert!(record(off, events()).steps.is_empty());
    // On by default (RB-090-66), as `automation.recorder.recordScroll`.
    let recording = record(RecorderOptions::default(), events());
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

// RB-090-76: the step order of a recording made on a navigation with hover-only dropdowns and a
// checkbox that opens the navigation - with neutral names.

#[test]
fn a_hover_on_the_element_clicked_before_is_dropped() {
    let careers = role("link", "Careers", "#nav > li:nth-child(2) > a");
    let trainees = role(
        "link",
        "Trainees",
        "#nav > li:nth-child(2) > ul > li:nth-child(1) > a",
    );
    let experienced = role(
        "link",
        "Experienced",
        "#nav > li:nth-child(2) > ul > li:nth-child(2) > a",
    );
    let recording = record(
        every_hover(),
        vec![
            hover(careers.clone()),
            click(trainees.clone()),
            // The pointer still rests on the link it clicked: no step.
            hover(trainees.clone()),
            hover(careers),
            click(experienced),
        ],
    );
    assert_eq!(
        kinds(&recording),
        [
            "hover #nav > li:nth-child(2) > a",
            "click #nav > li:nth-child(2) > ul > li:nth-child(1) > a",
            "hover #nav > li:nth-child(2) > a",
            "click #nav > li:nth-child(2) > ul > li:nth-child(2) > a",
        ]
    );
    // The same link after a navigation has another path but the same role and name.
    let again = role("link", "Trainees", "#nav > li:nth-child(3) > a");
    let recording = record(
        every_hover(),
        vec![click(trainees), hover(again), click(css("#next"))],
    );
    assert_eq!(
        kinds(&recording),
        [
            "click #nav > li:nth-child(2) > ul > li:nth-child(1) > a",
            "click #next"
        ]
    );
}

/// The `input` a checkbox fires: no text, the value `on`.
fn toggled(element: crate::event::ElementSelector) -> SyncEvent {
    SyncEvent::Input(crate::event::InputData {
        input_type: String::new(),
        data: None,
        value: Some("on".into()),
        sensitive: false,
        target: at(element),
    })
}

#[test]
fn a_checkbox_is_checked_never_filled() {
    let toggle = role("checkbox", "Open the navigation", "#nav-toggle");
    let recording = record(
        RecorderOptions::default(),
        vec![
            click(toggle.clone()),
            toggled(toggle.clone()),
            checked(toggle.clone(), true),
            click(toggle.clone()),
            toggled(toggle.clone()),
            checked(toggle.clone(), false),
        ],
    );
    assert_eq!(
        kinds(&recording),
        ["check #nav-toggle", "uncheck #nav-toggle"]
    );
    // With form changes not captured, the click stays and the input is no fill.
    let recording = record(
        RecorderOptions::default(),
        vec![click(toggle.clone()), toggled(toggle)],
    );
    assert_eq!(kinds(&recording), ["click #nav-toggle"]);
}

#[test]
fn a_checkbox_without_a_name_is_checked_too() {
    let toggle = css("#agree");
    let recording = record(
        RecorderOptions::default(),
        vec![
            click(toggle.clone()),
            toggled(toggle.clone()),
            checked(toggle, true),
        ],
    );
    assert_eq!(kinds(&recording), ["check #agree"]);
    let Some(step) = recording.steps.first() else {
        panic!("one step");
    };
    assert!(step.only_in.is_empty());
}

#[test]
fn the_menu_entry_clicked_before_still_opens_its_submenu() {
    // Click the entry (its page loads), then open its dropdown and click into it: the agent
    // reports the entry's hover at the press, which stays though the entry was clicked before.
    let careers = role("link", "Careers", "#nav > li:nth-child(2) > a");
    let experienced = role(
        "link",
        "Experienced",
        "#nav > li:nth-child(2) > ul > li:nth-child(2) > a",
    );
    for options in [RecorderOptions::default(), every_hover()] {
        let recording = record(
            options,
            vec![
                click(careers.clone()),
                hover(careers.clone()),
                hover(experienced.clone()),
                menu_hover(careers.clone()),
                click(experienced.clone()),
            ],
        );
        assert_eq!(
            kinds(&recording),
            [
                "click #nav > li:nth-child(2) > a",
                "hover #nav > li:nth-child(2) > a",
                "click #nav > li:nth-child(2) > ul > li:nth-child(2) > a",
            ]
        );
    }
}

// RB-090-81: the pointer on its way through a navigation - over the entries, a heading, the
// entry whose dropdown it opens - and the agent's menu hover at the press into the dropdown.
fn through_the_navigation() -> Vec<SyncEvent> {
    let about = role("link", "About us", "#nav > li:nth-child(1) > a");
    let sustainability = role("link", "Sustainability", "#nav > li:nth-child(2) > a");
    let careers = role("link", "Careers", "#nav > li:nth-child(3) > a");
    let heading = role("heading", "Welcome", "main > h1");
    let experienced = role(
        "link",
        "Experienced",
        "#nav > li:nth-child(3) > ul > li:nth-child(2) > a",
    );
    vec![
        hover(heading),
        hover(about),
        hover(sustainability),
        hover(careers.clone()),
        hover(experienced.clone()),
        menu_hover(careers),
        click(experienced),
    ]
}

#[test]
fn by_default_only_the_agents_menu_hover_is_a_step() {
    let recording = record(RecorderOptions::default(), through_the_navigation());
    assert_eq!(
        kinds(&recording),
        [
            "hover #nav > li:nth-child(3) > a",
            "click #nav > li:nth-child(3) > ul > li:nth-child(2) > a",
        ]
    );
    // A move the agent did not mark is never a step, even on a named element.
    let products = role("link", "Products", "#products");
    let item = testid("product-alpha", "#products + ul > li > a");
    let recording = record(
        RecorderOptions::default(),
        vec![hover(products), hover(item.clone()), click(item)],
    );
    assert_eq!(kinds(&recording), ["click #products + ul > li > a"]);
}

#[test]
fn record_hover_off_keeps_not_even_the_menu_hover() {
    let off = RecorderOptions {
        record_hover: HoverCapture::Off,
        ..RecorderOptions::default()
    };
    let recording = record(off, through_the_navigation());
    assert_eq!(
        kinds(&recording),
        ["click #nav > li:nth-child(3) > ul > li:nth-child(2) > a"]
    );
}

/// A mouse press.
fn press(element: crate::event::ElementSelector) -> SyncEvent {
    SyncEvent::Pointer(crate::event::PointerData {
        phase: crate::event::PointerPhase::Down,
        button: MouseButton::Left,
        buttons: 1,
        pointer_type: "mouse".into(),
        modifiers: 0,
        target: at(element),
    })
}

#[test]
fn only_the_agents_menu_hover_is_kept_on_the_element_clicked_before() {
    // The pointer rests on the link it clicked; the next move's hover is still on its way when
    // the button goes down. Without the agent's mark the resting hover is no step.
    let careers = role("link", "Careers", "#nav > li:nth-child(2) > a");
    let search = role("button", "Search", "#search");
    let recording = record(
        every_hover(),
        vec![
            click(careers.clone()),
            hover(careers),
            press(search.clone()),
            click(search),
        ],
    );
    assert_eq!(
        kinds(&recording),
        ["click #nav > li:nth-child(2) > a", "click #search"]
    );
}

#[test]
fn a_check_that_replaces_a_fill_runs_where_both_ran_and_leaves_no_gap() {
    let toggle = css("#agree");
    let mut recorder = started(RecorderOptions::default(), None);
    feed(&mut recorder, DESKTOP, &[MOBILE], 0, click(toggle.clone()));
    // The fill reached only the desktop view.
    feed(&mut recorder, DESKTOP, &[], 10, toggled(toggle.clone()));
    feed(&mut recorder, DESKTOP, &[MOBILE], 20, checked(toggle, true));
    feed(&mut recorder, DESKTOP, &[MOBILE], 30, click(css("#next")));
    let recording = recorder.stop().expect("stops");
    assert_eq!(kinds(&recording), ["check #agree", "click #next"]);
    assert_eq!(recording.steps[0].only_in, ["desktop"]);
    let ids: Vec<u32> = recording.steps.iter().map(|step| step.id).collect();
    assert_eq!(ids, [1, 2]);
}

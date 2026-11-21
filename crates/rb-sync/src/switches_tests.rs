use super::*;
use crate::event::{
    ChangeData, ElementSelector, ElementTarget, HoverData, KeyData, MouseButton, MouseData, Offset,
    ScrollData, SubmitData, TargetData,
};

pub(crate) fn target(path: &str) -> ElementTarget {
    ElementTarget {
        hosts: Vec::new(),
        element: ElementSelector::Css {
            path: path.to_owned(),
        },
        offset: Offset { x: 0.5, y: 0.5 },
    }
}

fn click() -> SyncEvent {
    SyncEvent::Click(MouseData {
        control: None,
        button: MouseButton::Left,
        detail: 1,
        modifiers: 0,
        target: target("main > a"),
    })
}

#[test]
fn every_event_has_the_kind_of_its_switch() {
    assert_eq!(SyncKind::of(&click()), SyncKind::Click);
    assert_eq!(
        SyncKind::of(&SyncEvent::Hover(HoverData {
            target: target("nav"),
            menu: false,
        })),
        SyncKind::Hover
    );
    assert_eq!(
        SyncKind::of(&SyncEvent::Scroll(ScrollData {
            container: None,
            anchor: None,
            x: 0.0,
            y: 10.0,
            max_x: 0.0,
            max_y: 100.0,
        })),
        SyncKind::Scroll
    );
    assert_eq!(
        SyncKind::of(&SyncEvent::Keydown(KeyData {
            key: "a".into(),
            code: "KeyA".into(),
            key_code: 65,
            location: 0,
            repeat: false,
            modifiers: 0,
            sensitive: false,
            target: None,
        })),
        SyncKind::Keyboard
    );
    assert_eq!(
        SyncKind::of(&SyncEvent::Submit(SubmitData {
            method: "post".into(),
            action: "http://localhost/form".into(),
            target: target("form"),
        })),
        SyncKind::Input
    );
}

#[test]
fn hover_and_scroll_never_desync_a_view() {
    assert!(marks_desync(&click()));
    assert!(!marks_desync(&SyncEvent::Hover(HoverData {
        target: target("nav"),
        menu: false,
    })));
    assert!(!marks_desync(&SyncEvent::Scroll(ScrollData {
        container: None,
        anchor: None,
        x: 0.0,
        y: 0.0,
        max_x: 0.0,
        max_y: 0.0,
    })));
}

#[test]
fn the_event_names_its_type_and_target() {
    assert_eq!(event_type(&click()), "click");
    assert_eq!(selector_of(&click()), Some("main > a".to_owned()));
    let keydown = SyncEvent::Keydown(KeyData {
        key: "Tab".into(),
        code: "Tab".into(),
        key_code: 9,
        location: 0,
        repeat: false,
        modifiers: 0,
        sensitive: false,
        target: None,
    });
    assert_eq!(event_type(&keydown), "keydown");
    assert_eq!(selector_of(&keydown), None);
}

#[test]
fn the_switches_turn_kinds_on_and_off() {
    let mut switches = SyncSwitches::default();
    assert!(switches.enabled && switches.leader_only_submit);
    for kind in SyncKind::ALL {
        // Focus sync is off by default (RB-020-05).
        assert_eq!(switches.kind(kind), kind != SyncKind::Focus, "{kind:?}");
    }
    switches.set_kind(SyncKind::Hover, false);
    assert!(!switches.kind(SyncKind::Hover));
    let capture = switches.capture();
    assert!(!capture.hover && capture.click && capture.scroll);
}

#[test]
fn change_is_form_sync_and_focus_is_focus_sync() {
    let change = SyncEvent::Change(ChangeData {
        value: Some("de".into()),
        checked: None,
        selected: Some(vec![1]),
        sensitive: false,
        target: target("#lang"),
    });
    let focus = SyncEvent::Focus(TargetData {
        target: target("#q"),
    });
    assert_eq!(SyncKind::of(&change), SyncKind::Forms);
    assert_eq!(SyncKind::of(&focus), SyncKind::Focus);
    assert_eq!(SyncKind::ALL.len(), 8);

    let mut switches = SyncSwitches::default();
    assert!(switches.forms && !switches.focus);
    switches.set_kind(SyncKind::Focus, true);
    switches.set_kind(SyncKind::Forms, false);
    let capture = switches.capture();
    assert!(capture.focus && !capture.forms && capture.input);
}

use std::collections::BTreeSet;

use rb_commands::CommandError;
use rb_engine::{Target, ViewId};
use serde_json::json;

use super::*;

fn classes(list: &[PseudoClass]) -> BTreeSet<PseudoClass> {
    list.iter().copied().collect()
}

fn refused(args: serde_json::Value) -> String {
    match parse(&args) {
        Err(CommandError::InvalidArgs(why)) => why,
        other => panic!("{args} should be refused, got {other:?}"),
    }
}

/// The page menu's entry: the click in DIP of the clicked view, one state switched, in all
/// views.
#[test]
fn the_menus_toggle_reads_with_its_point_and_target() {
    let args = parse(&json!({
        "target": "all", "view": 2, "x": 40, "y": 12.5, "dip": true, "toggle": ":hover",
    }))
    .expect("reads");
    assert_eq!(
        args,
        Args {
            target: Some(Target::All),
            view: Some(ViewId(2)),
            element: Some(Pointed::At {
                point: Point { x: 40.0, y: 12.5 },
                dip: true,
            }),
            request: Request::Toggle(PseudoClass::Hover),
        }
    );
}

/// The shortcut runs it bare: `:hover` on the element the user clicks, in the active view.
#[test]
fn without_arguments_a_click_picks_the_element_and_it_hovers() {
    let args = parse(&serde_json::Value::Null).expect("reads");
    assert_eq!(args.element, Some(Pointed::Pick));
    assert_eq!(args.request, Request::Set(classes(&[PseudoClass::Hover])));
    assert_eq!(args.target, None);
}

#[test]
fn states_are_exactly_these_and_an_empty_list_takes_them_back() {
    let args = parse(&json!({ "selector": " nav > li ", "states": ["focus", ":focus-within"] }))
        .expect("reads");
    assert_eq!(args.element, Some(Pointed::Selector("nav > li".to_owned())));
    assert_eq!(
        args.request,
        Request::Set(classes(&[PseudoClass::Focus, PseudoClass::FocusWithin]))
    );
    let args = parse(&json!({ "x": 1, "y": 2, "states": [] })).expect("reads");
    assert_eq!(args.request, Request::Set(BTreeSet::new()));
}

#[test]
fn clear_with_an_element_empties_it_and_without_one_resets_the_views() {
    let args = parse(&json!({ "x": 1, "y": 2, "clear": true })).expect("reads");
    assert_eq!(args.request, Request::Set(BTreeSet::new()));
    assert!(matches!(args.element, Some(Pointed::At { .. })));
    let args = parse(&json!({ "target": 3, "clear": true })).expect("reads");
    assert_eq!(args.element, None, "every state of view 3 goes");
    assert_eq!(args.target, Some(Target::View(ViewId(3))));
    let args = parse(&json!({ "clear": false })).expect("reads");
    assert_eq!(
        args.element,
        Some(Pointed::Pick),
        "`clear: false` is no clear"
    );
}

#[test]
fn malformed_arguments_are_refused_with_the_reason() {
    assert!(refused(json!({ "states": ["hover"], "toggle": "focus" })).contains("one of"));
    assert!(refused(json!({ "toggle": "hover", "clear": true })).contains("one of"));
    assert!(refused(json!({ "states": ["checked"] })).contains("unknown state `checked`"));
    assert!(refused(json!({ "toggle": "nope" })).contains("focus-visible"));
    assert!(refused(json!({ "x": 1 })).contains("together"));
    assert!(refused(json!({ "x": -1, "y": 2 })).contains("non-negative"));
    assert!(refused(json!({ "x": 1, "y": 2, "selector": "li" })).contains("not both"));
    assert!(refused(json!({ "selector": "  " })).contains("empty"));
    assert!(refused(json!({ "selector": "a\nb" })).contains("control"));
    let long = "a".repeat(MAX_SELECTOR_CHARS + 1);
    assert!(refused(json!({ "selector": long })).contains("longer"));
    assert!(refused(json!({ "selector": "li", "toggle": "hover" })).contains("toggle"));
    assert!(
        !refused(json!({ "colour": "red" })).is_empty(),
        "unknown field"
    );
}

#[test]
fn the_reset_takes_every_view_without_a_target() {
    assert_eq!(
        parse_reset(&serde_json::Value::Null).expect("reads"),
        Target::All
    );
    assert_eq!(parse_reset(&json!({})).expect("reads"), Target::All);
    assert_eq!(
        parse_reset(&json!({ "target": 4 })).expect("reads"),
        Target::View(ViewId(4))
    );
    assert!(parse_reset(&json!({ "view": 4 })).is_err());
}

#[test]
fn the_notice_carries_its_key_level_and_placeholders() {
    let CoreToShell::Notification(notification) = notice(
        NotificationLevel::Warning,
        FORCED_KEY,
        &[("views", "Mobile, Desktop".to_owned())],
    ) else {
        panic!("a notification");
    };
    assert_eq!(notification.level, NotificationLevel::Warning);
    assert_eq!(notification.message_key, FORCED_KEY);
    assert_eq!(notification.args["views"], "Mobile, Desktop");
    assert_eq!(notification.settings_key, None);
}

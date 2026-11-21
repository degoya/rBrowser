use super::*;
use crate::classify::Kind;
use crate::value::Goal;

fn planned(index: u32, key: &str, fill: Option<Fill>) -> Planned {
    Planned {
        index,
        sig: format!("sig-{key}"),
        key: key.to_owned(),
        label: key.to_owned(),
        kind: Kind::Text,
        skip: if fill.is_some() {
            String::new()
        } else {
            "disabled".to_owned()
        },
        fill,
        violation: None,
        profile: false,
        required: false,
        form: None,
        filled: false,
        uid: 0,
    }
}

fn text() -> Option<Fill> {
    Some(Fill::Text {
        candidates: vec!["x".to_owned()],
        goal: Goal::Any,
    })
}

fn select() -> Option<Fill> {
    Some(Fill::Select {
        values: vec!["b".to_owned()],
        goal: Goal::Valid,
    })
}

fn check() -> Option<Fill> {
    Some(Fill::Check { checked: true })
}

fn radio() -> Option<Fill> {
    Some(Fill::Radio {
        value: Some("a".to_owned()),
    })
}

fn keys(list: &[&Planned]) -> Vec<String> {
    list.iter().map(|entry| entry.key.clone()).collect()
}

fn applied(fields: Value) -> Applied {
    serde_json::from_value(json!({ "fields": fields, "formValid": true, "blockedSubmits": 1 }))
        .expect("an answer")
}

#[test]
fn control_fields_come_first_and_keep_the_page_order() {
    let plan = [
        planned(0, "name", text()),
        planned(1, "topic", select()),
        planned(2, "city", text()),
        planned(3, "privacy", check()),
        planned(4, "salutation", radio()),
        planned(5, "off", None),
    ];
    let first = todo(&plan, &BTreeSet::new(), true);
    assert_eq!(
        keys(&first),
        ["topic", "privacy", "salutation", "name", "city"]
    );
    let actions = actions(&first);
    assert_eq!(actions.len(), 5);
    assert_eq!(actions[0]["index"], 1);
    assert_eq!(actions[0]["op"], "select");
    assert_eq!(actions[3]["index"], 0);
}

#[test]
fn a_later_pass_sets_only_the_new_fields_that_hold_nothing() {
    let mut held = planned(2, "company", text());
    held.filled = true;
    let plan = [
        planned(0, "topic", select()),
        planned(1, "name", text()),
        held,
        planned(3, "message", text()),
        planned(4, "off", None),
    ];
    let done: BTreeSet<String> = ["topic".to_owned(), "name".to_owned()].into();
    assert_eq!(keys(&todo(&plan, &done, false)), ["message"]);
    // The first pass sets every field with a value, as a fill always has.
    assert_eq!(
        keys(&todo(&plan, &BTreeSet::new(), true)),
        ["topic", "name", "company", "message"]
    );
}

#[test]
fn the_settle_request_follows_the_setting_and_its_caps() {
    let off = Pacing {
        settle_ms: 0,
        max_passes: 3,
    };
    assert_eq!(off.settle(), None);
    let on = Pacing {
        settle_ms: 300,
        max_passes: 3,
    };
    assert_eq!(
        on.settle(),
        Some(json!({ "quiet": 300, "cap": STEP_CAP_MS, "budget": APPLY_BUDGET_MS }))
    );
    assert!(
        STEP_CAP_MS >= MAX_SETTLE_MS,
        "a step waits at least the quiet window"
    );
    assert!(APPLY_BUDGET_MS > STEP_CAP_MS);
}

#[test]
fn an_outcome_keeps_the_page_order_and_takes_a_later_pass_over_an_earlier_one() {
    let mut outcome = Outcome::default();
    let first = [
        planned(0, "topic", select()),
        planned(1, "name", text()),
        planned(2, "company", None),
    ];
    outcome.note(&first, true);
    let todo_first = todo(&first, &outcome.done, true);
    let changed = outcome.absorb(
        &todo_first,
        applied(json!([
            { "index": 0, "status": "done", "changed": true, "record": { "kind": "select" } },
            { "index": 1, "status": "changed" },
        ])),
    );
    assert!(changed, "a changed select may bring fields");
    assert_eq!(outcome.passes, 1);
    assert_eq!(outcome.done, BTreeSet::from(["topic".to_owned()]));
    assert_eq!(outcome.record, [json!({ "kind": "select" })]);

    // The second read: the name moved, the company is enabled, a message field is new, and a
    // field the page filled itself is left alone.
    let mut held = planned(4, "subject", text());
    held.filled = true;
    let second = [
        planned(0, "topic", select()),
        planned(2, "name", text()),
        planned(3, "company", text()),
        held,
        planned(5, "message", text()),
    ];
    outcome.note(&second, false);
    let todo_second = todo(&second, &outcome.done, false);
    assert_eq!(keys(&todo_second), ["name", "company", "message"]);
    let changed = outcome.absorb(
        &todo_second,
        applied(json!([
            { "index": 2, "status": "done", "changed": false },
            { "index": 3, "status": "done", "changed": false },
            { "index": 5, "status": "done", "changed": false },
        ])),
    );
    assert!(!changed, "nothing changed: no further read is needed");
    assert_eq!(outcome.passes, 2);
    assert_eq!(outcome.blocked, 2);
    let entries: Vec<(&str, u32, &str, Option<&str>)> = outcome
        .entries()
        .map(|(entry, found)| {
            (
                entry.key.as_str(),
                entry.index,
                entry.skip.as_str(),
                found.and_then(|field| field.status.as_str()),
            )
        })
        .collect();
    assert_eq!(
        entries,
        [
            ("topic", 0, "", Some("done")),
            ("name", 2, "", Some("done")),
            ("company", 3, "", Some("done")),
            ("subject", 4, "filled", None),
            ("message", 5, "", Some("done")),
        ]
    );
}

#[test]
fn a_missing_field_asks_for_another_read() {
    let mut outcome = Outcome::default();
    let plan = [planned(0, "name", text())];
    outcome.note(&plan, true);
    let list = todo(&plan, &outcome.done, true);
    assert!(outcome.absorb(&list, applied(json!([{ "index": 0, "status": "missing" }]))));
    assert!(outcome.done.is_empty());
}

#[test]
fn a_field_inserted_before_another_is_set_and_the_other_not_twice() {
    // An unnamed field is keyed by its position: one the page inserts takes the old one's key,
    // the page script's identity (`uid`) stays with the element (the w32 review).
    let mut outcome = Outcome::default();
    let mut old = planned(0, "input:text:0", text());
    old.uid = 1;
    let first = [old];
    outcome.note(&first, true);
    let list = todo(&first, &outcome.done, true);
    outcome.absorb(
        &list,
        applied(json!([{ "index": 0, "status": "done", "changed": true }])),
    );
    let mut inserted = planned(0, "input:text:0", text());
    inserted.uid = 2;
    let mut moved = planned(1, "input:text:1", text());
    moved.uid = 1;
    moved.filled = true;
    let second = [inserted, moved];
    outcome.note(&second, false);
    let list = todo(&second, &outcome.done, false);
    assert_eq!(list.iter().map(|entry| entry.uid).collect::<Vec<_>>(), [2]);
    outcome.absorb(
        &list,
        applied(json!([{ "index": 0, "status": "done", "changed": false }])),
    );
    let set: Vec<(u32, Option<&str>)> = outcome
        .entries()
        .map(|(entry, found)| (entry.uid, found.and_then(|field| field.status.as_str())))
        .collect();
    assert_eq!(set, [(1, Some("done")), (2, Some("done"))]);
}

use super::*;
use crate::field::{Choice, Field};

fn input(control: &str) -> Field {
    Field {
        key: format!("input:{control}"),
        tag: "input".to_owned(),
        control: control.to_owned(),
        ..Field::default()
    }
}

fn run(field: &Field, strategy: Strategy) -> Option<(Fill, Violation)> {
    invalid(field, strategy, 4)
}

#[test]
fn every_strategy_parses_from_its_name() {
    for strategy in Strategy::ALL {
        assert_eq!(Strategy::parse(strategy.id()), Some(strategy));
    }
    assert_eq!(Strategy::parse("nope"), None);
}

#[test]
fn an_email_without_an_at_sign_breaks_the_format() {
    let (fill, violation) = run(&input("email"), Strategy::Format).expect("applies");
    assert_eq!(violation, Violation::Format);
    let Fill::Text { candidates, goal } = fill else {
        panic!("text")
    };
    assert_eq!(goal, Goal::Invalid);
    assert!(!candidates[0].contains('@'));
}

#[test]
fn bounds_are_left_by_one_and_a_step_by_half() {
    let mut age = input("number");
    age.min = "18".into();
    age.max = "120".into();
    let (fill, violation) = run(&age, Strategy::Bounds).expect("applies");
    assert_eq!(
        (fill.shown().as_str(), violation),
        ("17", Violation::Bounds)
    );
    let mut stepped = input("number");
    stepped.step = "5".into();
    let (fill, violation) = run(&stepped, Strategy::Bounds).expect("applies");
    assert_eq!((fill.shown().as_str(), violation), ("2.5", Violation::Step));
    let mut range = input("range");
    range.min = "0".into();
    assert!(
        run(&range, Strategy::Bounds).is_none(),
        "a range clamps its value"
    );
}

#[test]
fn a_required_field_is_left_empty() {
    let mut text = input("text");
    text.required = true;
    assert_eq!(
        run(&text, Strategy::Empty).map(|(f, v)| (f.shown(), v)),
        Some((String::new(), Violation::Empty))
    );
    let mut check = input("checkbox");
    check.required = true;
    assert_eq!(
        run(&check, Strategy::Empty),
        Some((Fill::Check { checked: false }, Violation::Empty))
    );
    let mut radio = input("radio");
    radio.required = true;
    radio.options = vec![Choice {
        value: "a".into(),
        ..Choice::default()
    }];
    assert_eq!(
        run(&radio, Strategy::Empty),
        Some((Fill::Radio { value: None }, Violation::Empty))
    );
    assert!(run(&input("text"), Strategy::Empty).is_none());
}

#[test]
fn length_and_characters() {
    let mut text = input("text");
    text.max_length = Some(5);
    let (fill, violation) = run(&text, Strategy::Length).expect("applies");
    assert_eq!(
        (fill.shown().chars().count(), violation),
        (6, Violation::Length)
    );
    let (fill, violation) = run(&input("text"), Strategy::Chars).expect("applies");
    assert_eq!(violation, Violation::Chars);
    let shown = fill.shown();
    assert!(
        shown.contains('<') && shown.chars().any(|c| (c as u32) > 0xffff),
        "{shown}"
    );
    assert!(shown.split(' ').any(|word| word.chars().count() >= 100));
    assert!(run(&input("number"), Strategy::Chars).is_none());
}

#[test]
fn a_pattern_counts_as_format() {
    let mut zip = input("text");
    zip.pattern = "[0-9]{5}".into();
    let (fill, violation) = run(&zip, Strategy::Format).expect("applies");
    assert_eq!(violation, Violation::Pattern);
    assert!(fill.shown().chars().all(|c| !c.is_ascii_digit()));
}

#[test]
fn mixed_always_breaks_a_field_with_a_constraint_the_browser_checks() {
    let mut fields = Vec::new();
    let mut email = input("email");
    email.required = true;
    fields.push(email);
    let mut number = input("number");
    number.max = "9".into();
    fields.push(number);
    let mut zip = input("text");
    zip.pattern = "[0-9]{5}".into();
    fields.push(zip);
    let mut check = input("checkbox");
    check.required = true;
    fields.push(check);
    for field in &fields {
        for seed in 0..20 {
            let (_, violation) = invalid(field, Strategy::Mixed, seed).expect("applies");
            assert!(violation.checked_by_browser(), "{violation:?}");
        }
    }
    let mut long = input("text");
    long.max_length = Some(3);
    assert_eq!(
        invalid(&long, Strategy::Mixed, 1).map(|(_, v)| v),
        Some(Violation::Length)
    );
    assert_eq!(
        invalid(&input("text"), Strategy::Mixed, 1).map(|(_, v)| v),
        Some(Violation::Chars)
    );
    assert_eq!(invalid(&input("checkbox"), Strategy::Mixed, 1), None);
}

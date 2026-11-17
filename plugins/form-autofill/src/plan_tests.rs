use super::*;
use crate::field::{Choice, Collected, Field};
use crate::settings::Profile;

fn field(index: u32, control: &str, name: &str) -> Field {
    Field {
        index,
        key: format!("input:{name}"),
        sig: format!("input|{control}|{name}|"),
        tag: if control == "select" {
            "select"
        } else {
            "input"
        }
        .to_owned(),
        control: control.to_owned(),
        name: name.to_owned(),
        ..Field::default()
    }
}

fn page(locale: &str, fields: Vec<Field>) -> Collected {
    Collected {
        locale: locale.to_owned(),
        fields,
        ..Collected::default()
    }
}

fn options(mode: Mode) -> Options {
    Options {
        mode,
        strategy: Strategy::Mixed,
        seed: 42,
        profile: None,
    }
}

#[test]
fn a_valid_plan_follows_the_locale() {
    let fields = vec![field(0, "text", "plz"), field(1, "tel", "phone")];
    let german = plan(&page("de-DE", fields.clone()), &options(Mode::Valid));
    assert_eq!(german[0].kind, Kind::PostalCode);
    let zip = german[0].fill.as_ref().map(Fill::shown).unwrap_or_default();
    assert_eq!(zip.len(), 5);
    assert!(
        german[1]
            .fill
            .as_ref()
            .map(Fill::shown)
            .unwrap_or_default()
            .starts_with("+49 ")
    );
    let american = plan(&page("en-US", fields), &options(Mode::Valid));
    assert!(
        american[1]
            .fill
            .as_ref()
            .map(Fill::shown)
            .unwrap_or_default()
            .starts_with("+1 ")
    );
}

#[test]
fn the_same_seed_gives_the_same_plan_in_every_view() {
    let fields = vec![
        field(0, "email", "email"),
        field(1, "password", "pw"),
        field(2, "text", "note"),
    ];
    let one = plan(&page("en-US", fields.clone()), &options(Mode::Valid));
    // Another view: one field fewer before the others, the same keys.
    let mut shifted = fields;
    shifted.insert(0, field(9, "text", "extra"));
    let two = plan(&page("en-US", shifted), &options(Mode::Valid));
    for planned in &one {
        let same = two
            .iter()
            .find(|other| other.key == planned.key)
            .expect("same key");
        assert_eq!(same.fill, planned.fill, "{}", planned.key);
    }
}

#[test]
fn skipped_fields_get_no_action() {
    let mut file = field(0, "file", "upload");
    file.skip = "file".to_owned();
    let planned = plan(&page("en-US", vec![file]), &options(Mode::Valid));
    assert_eq!(planned[0].fill, None);
    assert_eq!(planned[0].skip, "file");
    assert_eq!(planned[0].action(), None);
}

#[test]
fn a_profile_replaces_generated_values_but_never_a_password() {
    let mut values = std::collections::BTreeMap::new();
    values.insert("email".to_owned(), "qa@example.com".to_owned());
    values.insert("pw".to_owned(), "hunter2".to_owned());
    values.insert("password".to_owned(), "hunter2".to_owned());
    values.insert("agree".to_owned(), "false".to_owned());
    values.insert("nick".to_owned(), "Q".to_owned());
    let mut options = options(Mode::Valid);
    options.profile = Some(Profile {
        name: "testuser".into(),
        values,
    });
    let mut nick = field(3, "text", "x");
    nick.id = "nick".into();
    let fields = vec![
        field(0, "email", "mail"),
        field(1, "password", "pw"),
        field(2, "checkbox", "agree"),
        nick,
    ];
    let planned = plan(&page("en-US", fields), &options);
    assert_eq!(
        planned[0].fill.as_ref().map(Fill::shown).as_deref(),
        Some("qa@example.com")
    );
    assert!(planned[0].profile);
    assert_ne!(
        planned[1].fill.as_ref().map(Fill::shown).as_deref(),
        Some("hunter2")
    );
    assert!(!planned[1].profile);
    assert_eq!(planned[2].fill, Some(Fill::Check { checked: false }));
    assert_eq!(
        planned[3].fill.as_ref().map(Fill::shown).as_deref(),
        Some("Q")
    );
}

#[test]
fn an_invalid_plan_breaks_constrained_fields_and_names_how() {
    let mut email = field(0, "email", "email");
    email.required = true;
    let free = field(1, "checkbox", "news");
    let planned = plan(&page("en-US", vec![email, free]), &options(Mode::Invalid));
    assert!(
        planned[0]
            .violation
            .is_some_and(Violation::checked_by_browser)
    );
    assert_eq!(planned[1].violation, None);
    assert!(matches!(planned[1].fill, Some(Fill::Check { .. })));
}

#[test]
fn the_action_names_the_field_and_its_signature() {
    let mut select = field(4, "select", "country");
    select.options = vec![Choice {
        value: "US".into(),
        label: "USA".into(),
        disabled: false,
    }];
    let planned = plan(&page("en-US", vec![select]), &options(Mode::Valid));
    let action = planned[0].action().expect("an action");
    assert_eq!(action["index"], 4);
    assert_eq!(action["sig"], "input|select|country|");
    assert_eq!(action["op"], "select");
    assert_eq!(action["values"][0], "US");
}

#[test]
fn a_label_falls_back_to_the_name() {
    let mut labelled = field(0, "text", "a");
    labelled.label = "  Street  ".into();
    let planned = plan(
        &page("en-US", vec![labelled, field(1, "text", "b")]),
        &options(Mode::Valid),
    );
    assert_eq!(planned[0].label, "Street");
    assert_eq!(planned[1].label, "b");
}

#[test]
fn a_planned_field_keeps_whether_it_is_required_and_its_form() {
    let mut required = field(0, "text", "a");
    required.required = true;
    required.form = Some(0);
    let mut skipped = field(1, "file", "b");
    skipped.skip = "file".to_owned();
    skipped.required = true;
    let planned = plan(
        &page("en-US", vec![required, skipped]),
        &options(Mode::Valid),
    );
    assert!(planned[0].required && planned[1].required);
    assert_eq!(planned[0].form, Some(0));
    assert_eq!(planned[1].form, None);
}

#[test]
fn a_hidden_input_takes_only_a_profile_value() {
    // RB-090-70: `type=hidden` inputs reach the plan only with `captureHiddenInputs`; a preset
    // sets them, nothing generated and nothing invalid ever does.
    let mut values = std::collections::BTreeMap::new();
    values.insert("campaign".to_owned(), "spring".to_owned());
    let mut with_profile = options(Mode::Valid);
    with_profile.profile = Some(Profile {
        name: "taken".into(),
        values,
    });
    let fields = vec![field(0, "hidden", "campaign"), field(1, "hidden", "ref")];
    let planned = plan(&page("en-US", fields.clone()), &with_profile);
    assert_eq!(
        planned[0].fill.as_ref().map(Fill::shown).as_deref(),
        Some("spring")
    );
    assert!(planned[0].profile);
    assert_eq!(planned[1].fill, None);
    assert_eq!(planned[1].skip, "hidden-input");
    for mode in [Mode::Valid, Mode::Invalid] {
        let planned = plan(&page("en-US", fields.clone()), &options(mode));
        assert!(planned.iter().all(|entry| entry.fill.is_none()), "{mode:?}");
        assert!(planned.iter().all(|entry| entry.skip == "hidden-input"));
    }
}

#[test]
fn a_hidden_input_takes_a_value_by_its_name_or_id_never_by_its_kind() {
    // Review finding 2: `email` in a profile never lands in `newsletter_email_hash`.
    let mut values = std::collections::BTreeMap::new();
    values.insert("email".to_owned(), "qa@example.com".to_owned());
    values.insert("token_id".to_owned(), "by-id".to_owned());
    let mut options = options(Mode::Valid);
    options.profile = Some(Profile {
        name: "taken".into(),
        values,
    });
    let mut hash = field(0, "hidden", "newsletter_email_hash");
    hash.autocomplete = "email".into();
    let mut by_id = field(1, "hidden", "");
    by_id.id = "token_id".into();
    let planned = plan(&page("en-US", vec![hash, by_id]), &options);
    assert_eq!(planned[0].fill, None);
    assert_eq!(planned[0].skip, "hidden-input");
    assert_eq!(
        planned[1].fill.as_ref().map(Fill::shown).as_deref(),
        Some("by-id")
    );
}

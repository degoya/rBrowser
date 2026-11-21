use super::*;
use crate::field::{Choice, Field};
use crate::persona::Lang;

fn input(control: &str) -> Field {
    Field {
        key: format!("input:{control}"),
        tag: "input".to_owned(),
        control: control.to_owned(),
        ..Field::default()
    }
}

fn ctx(lang: Lang) -> Ctx {
    Ctx::new(11, lang)
}

fn first(fill: &Fill) -> String {
    fill.shown()
}

#[test]
fn text_kinds_take_the_persona() {
    let ctx = ctx(Lang::De);
    let p = &ctx.persona;
    for (kind, expected) in [
        (Kind::Email, p.email.clone()),
        (Kind::GivenName, p.given.clone()),
        (Kind::FamilyName, p.family.clone()),
        (Kind::FullName, p.full_name.clone()),
        (Kind::PostalCode, p.postal_code.clone()),
        (Kind::City, p.city.clone()),
        (Kind::Phone, p.phone.clone()),
        (Kind::Country, p.country.clone()),
        (Kind::Birthday, p.birthday_text(Lang::De)),
    ] {
        assert_eq!(
            first(&valid(&input("text"), kind, &ctx)),
            expected,
            "{kind:?}"
        );
    }
    assert_eq!(
        first(&valid(&input("date"), Kind::Birthday, &ctx)),
        p.birthday_iso()
    );
}

#[test]
fn the_same_field_gets_the_same_value_in_every_call() {
    let ctx = ctx(Lang::En);
    let field = input("text");
    assert_eq!(
        valid(&field, Kind::Text, &ctx),
        valid(&field, Kind::Text, &ctx)
    );
    assert_eq!(
        valid(&field, Kind::Password, &ctx),
        valid(&field, Kind::Password, &ctx)
    );
}

#[test]
fn lengths_are_kept() {
    let ctx = ctx(Lang::En);
    let mut field = input("text");
    field.max_length = Some(3);
    assert_eq!(first(&valid(&field, Kind::City, &ctx)).chars().count(), 3);
    field.max_length = None;
    field.min_length = Some(40);
    assert!(first(&valid(&field, Kind::GivenName, &ctx)).chars().count() >= 40);
    let mut mail = input("email");
    mail.min_length = Some(60);
    let address = first(&valid(&mail, Kind::Email, &ctx));
    assert!(
        address.chars().count() >= 60 && address.ends_with("@example.com"),
        "{address}"
    );
}

#[test]
fn a_password_is_generated_has_every_class_and_fits() {
    let ctx = ctx(Lang::En);
    for (min, max) in [
        (None, None),
        (Some(20), None),
        (None, Some(8)),
        (Some(6), Some(6)),
    ] {
        let mut field = input("password");
        field.min_length = min;
        field.max_length = max;
        let password = first(&valid(&field, Kind::Password, &ctx));
        let n = password.chars().count();
        assert!(
            n >= min.unwrap_or(0) as usize && n <= max.unwrap_or(64) as usize,
            "{password}"
        );
        assert!(
            password.chars().any(|c| c.is_ascii_uppercase()),
            "{password}"
        );
        assert!(
            password.chars().any(|c| c.is_ascii_lowercase()),
            "{password}"
        );
        assert!(password.chars().any(|c| c.is_ascii_digit()), "{password}");
        assert!(
            password.chars().any(|c| !c.is_ascii_alphanumeric()),
            "{password}"
        );
    }
}

#[test]
fn a_pattern_adds_a_matching_sample() {
    let ctx = ctx(Lang::En);
    let mut field = input("text");
    field.pattern = "[A-Z]{2}[0-9]{4}".to_owned();
    let Fill::Text { candidates, goal } = valid(&field, Kind::Text, &ctx) else {
        panic!("a text fill");
    };
    assert_eq!(goal, Goal::Valid);
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[1].len(), 6);
}

#[test]
fn ordered_controls_stay_in_bounds() {
    let ctx = ctx(Lang::En);
    let mut field = input("number");
    field.min = "18".to_owned();
    field.max = "120".to_owned();
    let n: i64 = first(&valid(&field, Kind::Number, &ctx))
        .parse()
        .expect("number");
    assert!((18..=120).contains(&n));
    let color = first(&valid(&input("color"), Kind::Color, &ctx));
    assert!(color.starts_with('#') && color.len() == 7);
}

#[test]
fn a_choice_prefers_the_locale_country_and_skips_placeholders() {
    let ctx = ctx(Lang::De);
    let mut select = input("select");
    select.tag = "select".to_owned();
    select.options = vec![
        Choice {
            value: String::new(),
            label: "Choose".into(),
            disabled: false,
        },
        Choice {
            value: "AT".into(),
            label: "Austria".into(),
            disabled: false,
        },
        Choice {
            value: "DE".into(),
            label: "Germany".into(),
            disabled: false,
        },
    ];
    assert_eq!(
        valid(&select, Kind::Country, &ctx),
        Fill::Select {
            values: vec!["DE".into()],
            goal: Goal::Valid
        }
    );
    let Fill::Select { values, .. } = valid(&select, Kind::Choice, &ctx) else {
        panic!("a select fill");
    };
    assert_eq!(values.len(), 1);
    assert_ne!(values[0], "");
    let mut radio = input("radio");
    radio.options = vec![
        Choice {
            value: "a".into(),
            label: "A".into(),
            disabled: true,
        },
        Choice {
            value: "b".into(),
            label: "B".into(),
            disabled: false,
        },
    ];
    assert_eq!(
        valid(&radio, Kind::Choice, &ctx),
        Fill::Radio {
            value: Some("b".into())
        }
    );
}

#[test]
fn a_required_checkbox_is_checked() {
    let ctx = ctx(Lang::En);
    let mut field = input("checkbox");
    field.required = true;
    assert_eq!(
        valid(&field, Kind::Checkbox, &ctx),
        Fill::Check { checked: true }
    );
}

#[test]
fn a_consent_checkbox_is_checked_with_every_seed() {
    // RB-094-22: a privacy or terms box a page asks for without `required` (it enables the
    // submit button instead) is checked in a valid fill, never left to chance.
    for (name, label) in [
        ("privacy", ""),
        ("accept_terms", ""),
        ("", "Datenschutzerklaerung"),
        ("gdpr", ""),
        ("", "I consent to being contacted"),
    ] {
        let mut field = input("checkbox");
        field.name = name.to_owned();
        field.label = label.to_owned();
        for seed in 0..40 {
            assert_eq!(
                valid(&field, Kind::Checkbox, &Ctx::new(seed, Lang::En)),
                Fill::Check { checked: true },
                "{name}{label} seed {seed}"
            );
        }
    }
    // A newsletter or marketing box is no consent the form needs, whatever else it says.
    for (name, label) in [
        ("newsletter", ""),
        ("accept-newsletter", ""),
        ("marketing_consent", ""),
        ("", "I accept to receive offers by email"),
    ] {
        let mut news = input("checkbox");
        news.name = name.to_owned();
        news.label = label.to_owned();
        assert!(
            by_chance(&news, true),
            "{name}{label} stays left to the seed"
        );
    }
}

/// Whether `field` is ticked by some seeds and not by others.
fn by_chance(field: &Field, tick_consent: bool) -> bool {
    let checked = (0..40)
        .filter(|seed| {
            let mut ctx = Ctx::new(*seed, Lang::En);
            ctx.tick_consent = tick_consent;
            valid(field, Kind::Checkbox, &ctx) == Fill::Check { checked: true }
        })
        .count();
    checked > 0 && checked < 40
}

#[test]
fn a_consent_checkbox_is_left_to_the_seed_with_the_setting_off() {
    let mut field = input("checkbox");
    field.name = "privacy".to_owned();
    assert!(by_chance(&field, false));
    assert!(!by_chance(&field, true));
}

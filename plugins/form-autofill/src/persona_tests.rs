use super::*;

#[test]
fn the_locale_picks_the_data() {
    assert_eq!(Lang::from_locale("de-DE"), Lang::De);
    assert_eq!(Lang::from_locale("de"), Lang::De);
    assert_eq!(Lang::from_locale("DE_at"), Lang::De);
    assert_eq!(Lang::from_locale("en-US"), Lang::En);
    assert_eq!(Lang::from_locale("fr-FR"), Lang::En);
    assert_eq!(Lang::from_locale(""), Lang::En);
    assert_eq!(Lang::De.tag(), "de-DE");
    assert_eq!(Lang::En.tag(), "en-US");
}

#[test]
fn a_german_persona_has_a_five_digit_postal_code_and_a_plus_49_number() {
    for seed in 0..50 {
        let person = Persona::new(seed, Lang::De);
        assert_eq!(person.postal_code.len(), 5, "{}", person.postal_code);
        assert!(person.postal_code.chars().all(|c| c.is_ascii_digit()));
        assert!(person.phone.starts_with("+49 1"), "{}", person.phone);
        assert_eq!(person.country_code, "DE");
        assert!(person.email.ends_with("@example.com"));
    }
}

#[test]
fn an_english_persona_is_american_and_uses_reserved_data() {
    for seed in 0..50 {
        let person = Persona::new(seed, Lang::En);
        assert_eq!(person.postal_code.len(), 5);
        assert!(
            person.phone.starts_with("+1 202-555-01"),
            "{}",
            person.phone
        );
        assert_eq!(person.country_code, "US");
        assert_eq!(person.region_code.len(), 2);
        assert!(person.url.starts_with("https://www.example.com/"));
    }
}

#[test]
fn the_parts_of_a_persona_agree() {
    let person = Persona::new(7, Lang::En);
    assert_eq!(
        person.full_name,
        format!("{} {}", person.given, person.family)
    );
    let local = person.email.split('@').next().unwrap_or_default();
    assert!(local.starts_with(&person.given.to_ascii_lowercase()));
    assert!(person.email.is_ascii());
    assert!(
        person
            .username
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.')
    );
    assert!((1960..=2000).contains(&person.birthday.0));
    assert!((1..=12).contains(&person.birthday.1));
    assert!((1..=28).contains(&person.birthday.2));
}

#[test]
fn the_same_seed_and_language_give_the_same_persona() {
    assert_eq!(Persona::new(99, Lang::De), Persona::new(99, Lang::De));
    assert_ne!(Persona::new(99, Lang::De), Persona::new(100, Lang::De));
}

#[test]
fn a_birthday_is_written_as_the_language_writes_dates() {
    let mut person = Persona::new(1, Lang::De);
    person.birthday = (1985, 5, 4);
    assert_eq!(person.birthday_text(Lang::De), "04.05.1985");
    assert_eq!(person.birthday_text(Lang::En), "05/04/1985");
    assert_eq!(person.birthday_iso(), "1985-05-04");
}

//! The fixture form across the plugin's two halves (RB-070-11): the page script's fields of
//! `tests/fixtures/forms/register.html` (`register.collected.json`, kept fresh by the web tests)
//! planned here into `register.plan.<name>.json`, which the web tests apply to the form and check
//! with the browser's constraint validation. `FORM_AUTOFILL_UPDATE=1` rewrites the plans.

use std::collections::BTreeMap;
use std::path::PathBuf;

use form_autofill::args::Mode;
use form_autofill::field::Collected;
use form_autofill::invalid::Strategy;
use form_autofill::plan::{Options, Planned, plan};
use form_autofill::settings::Profile;
use serde_json::{Value, json};

const SEED: u64 = 42;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/forms")
}

fn collected(locale: &str) -> Collected {
    let text = std::fs::read_to_string(fixtures().join("register.collected.json"))
        .expect("the collected fields (web: npm test)");
    let mut page = Collected::parse(&text).expect("parses");
    page.locale = locale.to_owned();
    page
}

fn testuser() -> Profile {
    let mut values = BTreeMap::new();
    values.insert("email".to_owned(), "qa@example.com".to_owned());
    values.insert("password".to_owned(), "hunter2".to_owned());
    Profile {
        name: "testuser".to_owned(),
        values,
    }
}

fn planned(locale: &str, mode: Mode, profile: Option<Profile>) -> Vec<Planned> {
    let options = Options {
        mode,
        strategy: Strategy::Mixed,
        seed: SEED,
        profile,
    };
    plan(&collected(locale), &options)
}

/// The plan as the web tests read it, compared with (or written to) its committed file.
fn check(name: &str, locale: &str, mode: Mode, fields: &[Planned]) {
    let fields: Vec<Value> = fields
        .iter()
        .map(|field| {
            json!({ "key": field.key, "kind": field.kind.id(), "skip": field.skip,
                    "violation": field.violation.map(|v| v.id()), "profile": field.profile,
                    "action": field.action() })
        })
        .collect();
    let text = format!(
        "{}\n",
        serde_json::to_string_pretty(
            &json!({ "locale": locale, "mode": mode.id(), "seed": SEED, "fields": fields })
        )
        .expect("serialises")
    );
    let path = fixtures().join(format!("register.plan.{name}.json"));
    if std::env::var("FORM_AUTOFILL_UPDATE").as_deref() == Ok("1") {
        std::fs::write(&path, &text).expect("writes");
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == text,
        "{} is stale: FORM_AUTOFILL_UPDATE=1 cargo test",
        path.display()
    );
}

fn value_of(fields: &[Planned], key: &str) -> String {
    fields
        .iter()
        .find(|field| field.key == key)
        .and_then(|field| field.fill.as_ref())
        .map(form_autofill::value::Fill::shown)
        .unwrap_or_default()
}

#[test]
fn a_german_view_gets_a_german_postal_code_and_phone_number() {
    let fields = planned("de-DE", Mode::Valid, None);
    let zip = value_of(&fields, "input:text:zip");
    assert!(
        zip.len() == 5 && zip.chars().all(|c| c.is_ascii_digit()),
        "{zip}"
    );
    assert!(value_of(&fields, "input:tel:phone").starts_with("+49 1"));
    assert_eq!(value_of(&fields, "select:select:country"), "DE");
    check("valid-de", "de-DE", Mode::Valid, &fields);
}

#[test]
fn an_american_view_gets_american_data() {
    let fields = planned("en-US", Mode::Valid, None);
    assert!(value_of(&fields, "input:tel:phone").starts_with("+1 202-555-01"));
    assert_eq!(value_of(&fields, "select:select:country"), "US");
    let kinds: BTreeMap<String, &str> = fields
        .iter()
        .map(|field| (field.key.clone(), field.kind.id()))
        .collect();
    for (key, kind) in [
        ("input:radio:salutation", "choice"),
        ("input:text:firstName", "givenName"),
        ("input:text:lastName", "familyName"),
        ("input:email:email", "email"),
        ("input:text:username", "username"),
        ("input:password:password", "password"),
        ("input:date:birthday", "birthday"),
        ("input:number:age", "number"),
        ("input:text:street", "street"),
        ("input:text:zip", "postalCode"),
        ("input:text:city", "city"),
        ("select:select:country", "country"),
        ("input:tel:phone", "phone"),
        ("input:url:website", "url"),
        ("textarea:textarea:comment", "longText"),
        ("input:checkbox:terms", "checkbox"),
    ] {
        assert_eq!(kinds.get(key).copied(), Some(kind), "{key}");
    }
    let skipped: Vec<&str> = fields
        .iter()
        .filter(|field| field.fill.is_none())
        .map(|field| field.skip.as_str())
        .collect();
    assert_eq!(skipped, ["file", "hidden"]);
    check("valid-en", "en-US", Mode::Valid, &fields);
}

#[test]
fn an_invalid_run_breaks_every_field_with_a_constraint() {
    let fields = planned("en-US", Mode::Invalid, None);
    for field in &fields {
        let constrained = !matches!(
            field.key.as_str(),
            "input:checkbox:newsletter" | "input:file:avatar" | "input:text:promo"
        );
        if constrained {
            assert!(field.violation.is_some(), "{}", field.key);
        }
    }
    check("invalid", "en-US", Mode::Invalid, &fields);
}

#[test]
fn the_profile_testuser_sets_its_email_but_not_the_password() {
    let fields = planned("en-US", Mode::Valid, Some(testuser()));
    assert_eq!(value_of(&fields, "input:email:email"), "qa@example.com");
    assert_ne!(value_of(&fields, "input:password:password"), "hunter2");
    check("profile", "en-US", Mode::Valid, &fields);
}

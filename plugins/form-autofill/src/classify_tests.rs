use super::*;
use crate::field::Field;

fn field(control: &str, name: &str, autocomplete: &str, label: &str) -> Field {
    Field {
        tag: if control == "select" || control == "textarea" {
            control.to_owned()
        } else {
            "input".to_owned()
        },
        control: control.to_owned(),
        name: name.to_owned(),
        autocomplete: autocomplete.to_owned(),
        label: label.to_owned(),
        ..Field::default()
    }
}

/// The table of the job: field hints -> type.
#[test]
fn hints_give_the_kind() {
    let table: &[(&str, &str, &str, &str, Kind)] = &[
        ("email", "contact", "", "", Kind::Email),
        ("text", "x", "email", "", Kind::Email),
        ("text", "userEmail", "", "", Kind::Email),
        ("text", "", "", "E-Mail-Adresse", Kind::Email),
        ("text", "firstName", "", "", Kind::GivenName),
        ("text", "vorname", "", "", Kind::GivenName),
        ("text", "x", "given-name", "", Kind::GivenName),
        ("text", "last_name", "", "", Kind::FamilyName),
        ("text", "", "", "Nachname", Kind::FamilyName),
        ("text", "surname", "", "", Kind::FamilyName),
        ("text", "name", "", "", Kind::FullName),
        ("text", "", "", "Your name", Kind::FullName),
        ("text", "company_name", "", "", Kind::Organization),
        ("text", "username", "", "", Kind::Username),
        ("text", "login", "", "", Kind::Username),
        ("password", "pw", "", "", Kind::Password),
        ("text", "x", "new-password", "", Kind::Password),
        ("text", "street", "", "", Kind::Street),
        ("text", "", "", "Stra\u{df}e und Hausnummer", Kind::Street),
        ("text", "x", "shipping street-address", "", Kind::Street),
        ("text", "zip", "", "", Kind::PostalCode),
        ("text", "plz", "", "", Kind::PostalCode),
        ("text", "x", "billing postal-code", "", Kind::PostalCode),
        ("text", "city", "", "", Kind::City),
        ("text", "ort", "", "", Kind::City),
        ("text", "sport", "", "", Kind::Text),
        ("text", "state", "", "", Kind::Region),
        ("select", "country", "", "", Kind::Country),
        ("text", "", "", "Land", Kind::Country),
        ("tel", "x", "", "", Kind::Phone),
        ("text", "telefon", "", "", Kind::Phone),
        ("text", "mobile", "", "", Kind::Phone),
        ("url", "x", "", "", Kind::Url),
        ("text", "website", "", "", Kind::Url),
        ("date", "dob", "", "", Kind::Birthday),
        ("date", "start", "", "", Kind::Date),
        ("text", "x", "bday", "", Kind::Birthday),
        ("time", "x", "", "", Kind::Time),
        ("month", "x", "", "", Kind::Month),
        ("week", "x", "", "", Kind::Week),
        ("datetime-local", "x", "", "", Kind::DateTime),
        ("number", "age", "", "", Kind::Number),
        ("range", "x", "", "", Kind::Range),
        ("color", "x", "", "", Kind::Color),
        ("search", "q", "", "", Kind::Search),
        ("textarea", "message", "", "", Kind::LongText),
        ("checkbox", "email_me", "", "", Kind::Checkbox),
        ("radio", "salutation", "", "", Kind::Choice),
        ("select", "size", "", "", Kind::Choice),
        ("text", "x", "", "", Kind::Text),
    ];
    for (control, name, autocomplete, label, kind) in table {
        assert_eq!(
            classify(&field(control, name, autocomplete, label)),
            *kind,
            "{control} name={name} autocomplete={autocomplete} label={label}"
        );
    }
}

#[test]
fn the_name_and_id_win_over_the_label_and_the_placeholder() {
    let mut f = field("text", "city", "", "Your name");
    assert_eq!(classify(&f), Kind::City);
    f = field("text", "", "", "");
    f.id = "zipCode".to_owned();
    f.placeholder = "Name".to_owned();
    assert_eq!(classify(&f), Kind::PostalCode);
    f = field("text", "", "", "");
    f.placeholder = "jane@example.com".to_owned();
    assert_eq!(classify(&f), Kind::Text);
    f.placeholder = "Email".to_owned();
    assert_eq!(classify(&f), Kind::Email);
}

#[test]
fn every_kind_has_a_distinct_id() {
    let ids: std::collections::BTreeSet<&str> = Kind::ALL.iter().map(|k| k.id()).collect();
    assert_eq!(ids.len(), Kind::ALL.len());
    assert_eq!(Kind::GivenName.id(), "givenName");
}

#[test]
fn words_split_camel_case_and_separators() {
    assert_eq!(words("firstName"), "first name");
    assert_eq!(words("E-Mail_Address"), "e mail address");
    assert_eq!(words("ZIPCode"), "zipcode");
    assert_eq!(words("  Stra\u{df}e:  "), "stra\u{df}e");
}

//! Which kind of value a field wants, from its hints: the control (`type`, tag), the
//! `autocomplete` tokens, then the words of its `name` and `id`, its label and its placeholder -
//! in that order, so the field's own name wins over a label that names its neighbour. English and
//! German words; whatever matches nothing is free text (or a free choice).

use serde::Serialize;

use crate::field::Field;

/// The kind of value a field gets. Its [`id`](Kind::id) names it in results and profiles.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum Kind {
    Email,
    GivenName,
    FamilyName,
    FullName,
    Username,
    Password,
    Street,
    PostalCode,
    City,
    Region,
    Country,
    Phone,
    Organization,
    Url,
    Birthday,
    Date,
    Time,
    Month,
    Week,
    DateTime,
    Number,
    Range,
    Color,
    Search,
    LongText,
    Text,
    Checkbox,
    /// A select or radio group whose hints name nothing.
    Choice,
}

impl Kind {
    pub const ALL: [Kind; 28] = [
        Kind::Email,
        Kind::GivenName,
        Kind::FamilyName,
        Kind::FullName,
        Kind::Username,
        Kind::Password,
        Kind::Street,
        Kind::PostalCode,
        Kind::City,
        Kind::Region,
        Kind::Country,
        Kind::Phone,
        Kind::Organization,
        Kind::Url,
        Kind::Birthday,
        Kind::Date,
        Kind::Time,
        Kind::Month,
        Kind::Week,
        Kind::DateTime,
        Kind::Number,
        Kind::Range,
        Kind::Color,
        Kind::Search,
        Kind::LongText,
        Kind::Text,
        Kind::Checkbox,
        Kind::Choice,
    ];

    /// The kind's name in results, MCP answers and profiles (`givenName`).
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Kind::Email => "email",
            Kind::GivenName => "givenName",
            Kind::FamilyName => "familyName",
            Kind::FullName => "name",
            Kind::Username => "username",
            Kind::Password => "password",
            Kind::Street => "street",
            Kind::PostalCode => "postalCode",
            Kind::City => "city",
            Kind::Region => "region",
            Kind::Country => "country",
            Kind::Phone => "phone",
            Kind::Organization => "organization",
            Kind::Url => "url",
            Kind::Birthday => "birthday",
            Kind::Date => "date",
            Kind::Time => "time",
            Kind::Month => "month",
            Kind::Week => "week",
            Kind::DateTime => "datetime",
            Kind::Number => "number",
            Kind::Range => "range",
            Kind::Color => "color",
            Kind::Search => "search",
            Kind::LongText => "longText",
            Kind::Text => "text",
            Kind::Checkbox => "checkbox",
            Kind::Choice => "choice",
        }
    }
}

/// The words a kind is recognised by, most specific kinds first (`email address` is an email,
/// not a street; `company name` an organization, not a name). A keyword matches whole words.
const KEYWORDS: &[(Kind, &[&str])] = &[
    (
        Kind::Email,
        &["email", "e mail", "emailaddress", "mail", "mailadresse"],
    ),
    (
        Kind::Password,
        &["password", "passwort", "kennwort", "passwd", "pwd"],
    ),
    (
        Kind::Username,
        &[
            "username",
            "user name",
            "login",
            "benutzername",
            "benutzer",
            "nickname",
        ],
    ),
    (
        Kind::GivenName,
        &[
            "first name",
            "firstname",
            "given name",
            "givenname",
            "fname",
            "forename",
            "vorname",
        ],
    ),
    (
        Kind::FamilyName,
        &[
            "last name",
            "lastname",
            "surname",
            "family name",
            "familyname",
            "lname",
            "nachname",
            "familienname",
        ],
    ),
    (
        Kind::Organization,
        &[
            "company",
            "organization",
            "organisation",
            "firma",
            "unternehmen",
        ],
    ),
    (
        Kind::PostalCode,
        &[
            "zip",
            "zipcode",
            "postal",
            "postalcode",
            "postcode",
            "plz",
            "postleitzahl",
        ],
    ),
    (
        Kind::Street,
        &[
            "street",
            "address",
            "strasse",
            "stra\u{df}e",
            "adresse",
            "anschrift",
            "hausnummer",
        ],
    ),
    (Kind::City, &["city", "town", "ort", "stadt", "wohnort"]),
    (
        Kind::Region,
        &["state", "region", "province", "county", "bundesland"],
    ),
    (Kind::Country, &["country", "land", "nation"]),
    (
        Kind::Phone,
        &[
            "phone",
            "tel",
            "telephone",
            "mobile",
            "phonenumber",
            "telefon",
            "telefonnummer",
            "handy",
            "handynummer",
            "mobil",
            "mobilnummer",
        ],
    ),
    (
        Kind::Url,
        &["website", "homepage", "url", "webseite", "web site"],
    ),
    (
        Kind::Birthday,
        &[
            "birth",
            "birthday",
            "birthdate",
            "dob",
            "geburt",
            "geburtstag",
            "geburtsdatum",
        ],
    ),
    (
        Kind::FullName,
        &["name", "full name", "fullname", "your name"],
    ),
];

/// `text` as lower-case words: camelCase split, every other character a space.
#[must_use]
pub fn words(text: &str) -> String {
    let mut out = String::new();
    let mut previous_lower = false;
    for c in text.chars() {
        if c.is_alphanumeric() {
            if c.is_uppercase() && previous_lower {
                out.push(' ');
            }
            previous_lower = c.is_lowercase() || c.is_ascii_digit();
            out.extend(c.to_lowercase());
        } else {
            previous_lower = false;
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
        }
    }
    out.trim_end().to_owned()
}

/// The first kind whose keyword `text` holds as whole words.
fn by_words(text: &str) -> Option<Kind> {
    if text.is_empty() {
        return None;
    }
    let padded = format!(" {text} ");
    KEYWORDS
        .iter()
        .find(|(_, keywords)| {
            keywords
                .iter()
                .any(|keyword| padded.contains(&format!(" {keyword} ")))
        })
        .map(|(kind, _)| *kind)
}

/// The kind an `autocomplete` value names (its last token: `shipping postal-code`).
fn by_autocomplete(autocomplete: &str) -> Option<Kind> {
    let token = autocomplete.split_whitespace().last()?.to_ascii_lowercase();
    Some(match token.as_str() {
        "email" => Kind::Email,
        "given-name" => Kind::GivenName,
        "family-name" => Kind::FamilyName,
        "name" => Kind::FullName,
        "username" | "nickname" => Kind::Username,
        "new-password" | "current-password" => Kind::Password,
        "street-address" | "address-line1" => Kind::Street,
        "postal-code" => Kind::PostalCode,
        "address-level2" => Kind::City,
        "address-level1" => Kind::Region,
        "country" | "country-name" => Kind::Country,
        "tel" | "tel-national" => Kind::Phone,
        "organization" => Kind::Organization,
        "url" => Kind::Url,
        "bday" => Kind::Birthday,
        _ => return None,
    })
}

/// The kind `field` gets.
#[must_use]
pub fn classify(field: &Field) -> Kind {
    let control = field.control.to_ascii_lowercase();
    match control.as_str() {
        "checkbox" => return Kind::Checkbox,
        "password" => return Kind::Password,
        _ => {}
    }
    if let Some(kind) = by_autocomplete(&field.autocomplete) {
        return kind;
    }
    let own = words(&format!("{} {}", field.name, field.id));
    let hinted = [
        own.as_str(),
        &words(&field.label),
        &words(&field.placeholder),
    ]
    .into_iter()
    .find_map(by_words);
    match control.as_str() {
        "email" => return Kind::Email,
        "tel" => return Kind::Phone,
        "url" => return Kind::Url,
        "date" => {
            return if hinted == Some(Kind::Birthday) {
                Kind::Birthday
            } else {
                Kind::Date
            };
        }
        "time" => return Kind::Time,
        "month" => return Kind::Month,
        "week" => return Kind::Week,
        "datetime-local" => return Kind::DateTime,
        "number" => return Kind::Number,
        "range" => return Kind::Range,
        "color" => return Kind::Color,
        "search" => return Kind::Search,
        "textarea" => return Kind::LongText,
        _ => {}
    }
    let choice = control == "select" || control == "radio";
    match hinted {
        Some(Kind::Country | Kind::Region) if choice => hinted.unwrap_or(Kind::Choice),
        Some(kind) if !choice => kind,
        _ if choice => Kind::Choice,
        _ => Kind::Text,
    }
}

#[cfg(test)]
#[path = "classify_tests.rs"]
mod tests;

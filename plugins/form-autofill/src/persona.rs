//! The made-up person a run fills in: one per run and language, so the first name, the e-mail
//! address and the user name of one form agree. The data follows the view's locale - German
//! (`de-*`: German names, a five-digit postal code, a `+49` mobile number) or American English
//! (everything else) - and uses reserved values where they exist: `example.com` addresses, the
//! fictional `555-01xx` numbers.

use crate::rng::Rng;

/// The language of the generated data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lang {
    De,
    En,
}

impl Lang {
    /// The data for a BCP 47 locale (`de-DE`, `en-US`, ...): German for `de`, else English.
    #[must_use]
    pub fn from_locale(locale: &str) -> Self {
        let language = locale.split(['-', '_']).next().unwrap_or_default();
        if language.eq_ignore_ascii_case("de") {
            Lang::De
        } else {
            Lang::En
        }
    }

    /// The locale the data is made for.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Lang::De => "de-DE",
            Lang::En => "en-US",
        }
    }

    fn data(self) -> &'static Data {
        match self {
            Lang::De => &DE,
            Lang::En => &EN,
        }
    }

    /// A short free text.
    pub fn text(self, rng: &mut Rng) -> String {
        (*rng.pick(self.data().words)).to_owned()
    }

    /// A sentence or two.
    pub fn sentences(self, rng: &mut Rng) -> String {
        let data = self.data();
        format!("{} {}", rng.pick(data.sentences), rng.pick(data.sentences))
    }
}

/// A city with its region (code, name) and the first digits of its postal codes.
struct City {
    name: &'static str,
    region_code: &'static str,
    region: &'static str,
    postal_prefix: &'static str,
}

const fn city(
    name: &'static str,
    region_code: &'static str,
    region: &'static str,
    postal_prefix: &'static str,
) -> City {
    City {
        name,
        region_code,
        region,
        postal_prefix,
    }
}

struct Data {
    given: &'static [&'static str],
    family: &'static [&'static str],
    streets: &'static [&'static str],
    cities: &'static [City],
    organizations: &'static [&'static str],
    words: &'static [&'static str],
    sentences: &'static [&'static str],
    country_code: &'static str,
    country: &'static str,
}

const DE: Data = Data {
    given: &[
        "Anna", "Lukas", "Lena", "Jonas", "Laura", "Felix", "Marie", "Paul", "Sophie", "Leon",
        "Hanna", "Elias", "Clara", "Max", "Emma", "Noah",
    ],
    family: &[
        "Schmidt",
        "Schneider",
        "Fischer",
        "Weber",
        "Meyer",
        "Wagner",
        "Becker",
        "Hoffmann",
        "Schulz",
        "Koch",
        "Richter",
        "Wolf",
        "Neumann",
        "Schwarz",
        "Braun",
        "Zimmermann",
    ],
    streets: &[
        "Hauptstra\u{df}e",
        "Bahnhofstra\u{df}e",
        "Gartenweg",
        "Lindenallee",
        "Schulstra\u{df}e",
        "Bergstra\u{df}e",
        "Rosenweg",
        "Am Markt",
    ],
    cities: &[
        city("Berlin", "BE", "Berlin", "10"),
        city("Hamburg", "HH", "Hamburg", "20"),
        city("M\u{fc}nchen", "BY", "Bayern", "80"),
        city("K\u{f6}ln", "NW", "Nordrhein-Westfalen", "50"),
        city("Frankfurt am Main", "HE", "Hessen", "60"),
        city("Stuttgart", "BW", "Baden-W\u{fc}rttemberg", "70"),
        city("Leipzig", "SN", "Sachsen", "04"),
        city("Hannover", "NI", "Niedersachsen", "30"),
    ],
    organizations: &["Muster GmbH", "Beispiel AG", "Testhaus KG"],
    words: &["Testeingabe", "Beispieltext", "Probe", "Musterwert"],
    sentences: &[
        "Dies ist ein Testtext.",
        "Automatisch erzeugter Inhalt zum Testen.",
        "Nur ein Test, keine echten Daten.",
        "Ein kurzer Beispielsatz.",
    ],
    country_code: "DE",
    country: "Deutschland",
};

const EN: Data = Data {
    given: &[
        "James", "Mary", "John", "Patricia", "Robert", "Jennifer", "Michael", "Linda", "David",
        "Emily", "Daniel", "Sarah", "Ethan", "Olivia", "Lucas", "Grace",
    ],
    family: &[
        "Smith", "Johnson", "Williams", "Brown", "Jones", "Miller", "Davis", "Wilson", "Anderson",
        "Taylor", "Thomas", "Moore", "Martin", "Jackson", "Clark", "Lewis",
    ],
    streets: &[
        "Main Street",
        "Oak Avenue",
        "Maple Drive",
        "Cedar Lane",
        "Pine Street",
        "Elm Street",
        "Lake Road",
        "Hill Court",
    ],
    cities: &[
        city("Springfield", "IL", "Illinois", "627"),
        city("Portland", "OR", "Oregon", "972"),
        city("Austin", "TX", "Texas", "787"),
        city("Denver", "CO", "Colorado", "802"),
        city("Columbus", "OH", "Ohio", "432"),
        city("Madison", "WI", "Wisconsin", "537"),
        city("Raleigh", "NC", "North Carolina", "276"),
        city("Boston", "MA", "Massachusetts", "021"),
    ],
    organizations: &["Example Corp", "Acme Testing LLC", "Sample Industries Inc."],
    words: &["Test input", "Sample text", "Example", "Placeholder value"],
    sentences: &[
        "This is test input.",
        "Automatically generated content for testing.",
        "Please ignore this message.",
        "A short sample sentence.",
    ],
    country_code: "US",
    country: "United States",
};

/// The person of a run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Persona {
    pub given: String,
    pub family: String,
    pub full_name: String,
    pub email: String,
    pub username: String,
    pub phone: String,
    pub street: String,
    pub postal_code: String,
    pub city: String,
    pub region_code: String,
    pub region: String,
    pub country_code: String,
    pub country: String,
    pub organization: String,
    pub url: String,
    /// Year, month, day.
    pub birthday: (i64, u32, u32),
}

/// The lower-case ASCII letters and digits of `text`.
fn plain(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

impl Persona {
    /// The person of `seed` in `lang`.
    #[must_use]
    pub fn new(seed: u64, lang: Lang) -> Self {
        let data = lang.data();
        let mut rng = Rng::keyed(seed, &["persona", lang.tag()]);
        let given = (*rng.pick(data.given)).to_owned();
        let family = (*rng.pick(data.family)).to_owned();
        let number = rng.between(1, 99);
        let local = format!("{}.{}{number}", plain(&given), plain(&family));
        let home = rng.pick(data.cities);
        let street_name = rng.pick(data.streets);
        let house = rng.between(1, 180);
        let (street, phone) = match lang {
            Lang::De => (
                format!("{street_name} {house}"),
                format!(
                    "+49 {} {}",
                    rng.pick(&["151", "152", "160", "170", "171", "176"]),
                    rng.digits(8)
                ),
            ),
            Lang::En => (
                format!("{house} {street_name}"),
                format!("+1 202-555-01{}", rng.digits(2)),
            ),
        };
        let postal_code = format!(
            "{}{}",
            home.postal_prefix,
            rng.digits(5 - home.postal_prefix.len())
        );
        let birthday = (
            rng.between(1960, 2000),
            u32::try_from(rng.between(1, 12)).unwrap_or(1),
            u32::try_from(rng.between(1, 28)).unwrap_or(1),
        );
        Self {
            full_name: format!("{given} {family}"),
            email: format!("{local}@example.com"),
            username: local,
            phone,
            street,
            postal_code,
            city: home.name.to_owned(),
            region_code: home.region_code.to_owned(),
            region: home.region.to_owned(),
            country_code: data.country_code.to_owned(),
            country: data.country.to_owned(),
            organization: (*rng.pick(data.organizations)).to_owned(),
            url: format!("https://www.example.com/{}", plain(&family)),
            given,
            family,
            birthday,
        }
    }

    /// The birthday as `YYYY-MM-DD` (a date input's value).
    #[must_use]
    pub fn birthday_iso(&self) -> String {
        let (year, month, day) = self.birthday;
        format!("{year:04}-{month:02}-{day:02}")
    }

    /// The birthday as `lang` writes a date in a text field.
    #[must_use]
    pub fn birthday_text(&self, lang: Lang) -> String {
        let (year, month, day) = self.birthday;
        match lang {
            Lang::De => format!("{day:02}.{month:02}.{year:04}"),
            Lang::En => format!("{month:02}/{day:02}/{year:04}"),
        }
    }
}

#[cfg(test)]
#[path = "persona_tests.rs"]
mod tests;

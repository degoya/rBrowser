//! The valid value of a field: the run's person for the personal kinds, generated text, numbers
//! and dates inside their bounds, a generated password, an option of a select or radio group -
//! each fitted to `minlength` and `maxlength`, with a matching sample of `pattern` as a further
//! candidate. The page script sets the first candidate the page accepts.

use serde::Serialize;

use crate::bounds::{self, Scale};
use crate::classify::Kind;
use crate::field::{Choice, Field};
use crate::pattern;
use crate::persona::{Lang, Persona};
use crate::rng::Rng;

/// What the page script aims for when it tries the candidates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Goal {
    /// The first candidate the field accepts; the first one when none is accepted.
    Valid,
    /// The first candidate the field refuses; the last one when none is refused.
    Invalid,
    /// The first candidate.
    Any,
}

/// What the page script does with one field (its `op`).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Fill {
    Text {
        candidates: Vec<String>,
        goal: Goal,
    },
    Check {
        checked: bool,
    },
    /// The option values to select; none clears the selection.
    Select {
        values: Vec<String>,
        goal: Goal,
    },
    /// The radio of the group to check; none unchecks the group.
    Radio {
        value: Option<String>,
    },
}

impl Fill {
    /// The value as a result shows it (the first candidate).
    #[must_use]
    pub fn shown(&self) -> String {
        match self {
            Fill::Text { candidates, .. } => candidates.first().cloned().unwrap_or_default(),
            Fill::Check { checked } => checked.to_string(),
            Fill::Select { values, .. } => values.join(", "),
            Fill::Radio { value } => value.clone().unwrap_or_default(),
        }
    }
}

/// What every value of a run is made of.
pub struct Ctx {
    pub seed: u64,
    pub lang: Lang,
    pub persona: Persona,
}

impl Ctx {
    #[must_use]
    pub fn new(seed: u64, lang: Lang) -> Self {
        Self {
            seed,
            lang,
            persona: Persona::new(seed, lang),
        }
    }
}

/// The scale of an ordered control, if it is one.
#[must_use]
pub fn scale(field: &Field) -> Option<Scale> {
    Some(match field.control.as_str() {
        "number" | "range" => Scale::Number,
        "date" => Scale::Date,
        "time" => Scale::Time,
        "month" => Scale::Month,
        "week" => Scale::Week,
        "datetime-local" => Scale::DateTime,
        _ => return None,
    })
}

/// Whether the field holds free text (an input of a text type or a textarea).
#[must_use]
pub fn is_text(field: &Field) -> bool {
    field.tag == "textarea"
        || matches!(
            field.control.as_str(),
            "" | "text" | "email" | "url" | "tel" | "search" | "password"
        )
}

const LETTERS: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'k', 'm', 'n', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w',
    'x', 'z',
];

/// `text` cut to `max` characters and padded to `min` (before the `@` of an address).
fn fit(text: &str, min: Option<u32>, max: Option<u32>) -> String {
    let mut out = text.to_owned();
    let count = out.chars().count();
    let min = min.unwrap_or(0) as usize;
    if count < min {
        let pad = "x".repeat(min - count);
        match out.find('@') {
            Some(at) => out.insert_str(at, &pad),
            None => out.push_str(&pad),
        }
    }
    if let Some(max) = max {
        out = out.chars().take(max as usize).collect();
    }
    out
}

/// A generated password: an upper- and a lower-case letter, a digit and a sign, at least, as
/// long as the field allows (14 when it does not say). Test data, never a person's secret.
fn password(field: &Field, rng: &mut Rng) -> String {
    let max = field.max_length.map_or(64, |n| n as usize);
    let length = 14.max(field.min_length.unwrap_or(0) as usize).min(max);
    let upper = rng.pick(LETTERS).to_ascii_uppercase();
    let mut out: String = if length >= 14 {
        format!("Test-{upper}")
    } else {
        upper.to_string()
    };
    out.push(*rng.pick(LETTERS));
    out.push_str(&rng.digits(4));
    out.push('!');
    while out.chars().count() < length {
        out.push(*rng.pick(LETTERS));
    }
    if out.chars().count() > length {
        // A short field: a letter of each case, a digit and a sign first.
        let core = format!("{upper}{}{}!", rng.pick(LETTERS), rng.digits(1));
        out = core
            .chars()
            .chain(std::iter::repeat('x'))
            .take(length)
            .collect();
    }
    out
}

fn usable(options: &[Choice]) -> Vec<&Choice> {
    options
        .iter()
        .filter(|option| !option.disabled && !option.value.is_empty())
        .collect()
}

/// The option for `kind`: the person's country or region when the list has it, else any.
fn option(field: &Field, kind: Kind, ctx: &Ctx, rng: &mut Rng) -> Option<String> {
    let options = usable(&field.options);
    let p = &ctx.persona;
    let wanted: &[&str] = match kind {
        Kind::Country => &[&p.country_code, &p.country],
        Kind::Region => &[&p.region_code, &p.region],
        _ => &[],
    };
    let named = options.iter().find(|option| {
        wanted.iter().any(|want| {
            option.value.eq_ignore_ascii_case(want) || option.label.eq_ignore_ascii_case(want)
        })
    });
    named
        .or_else(|| (!options.is_empty()).then(|| rng.pick(&options)))
        .map(|option| option.value.clone())
}

fn text_base(field: &Field, kind: Kind, ctx: &Ctx, rng: &mut Rng) -> String {
    let p = &ctx.persona;
    match kind {
        Kind::Email => p.email.clone(),
        Kind::GivenName => p.given.clone(),
        Kind::FamilyName => p.family.clone(),
        Kind::FullName => p.full_name.clone(),
        Kind::Username => p.username.clone(),
        Kind::Password => password(field, rng),
        Kind::Street => p.street.clone(),
        Kind::PostalCode => p.postal_code.clone(),
        Kind::City => p.city.clone(),
        Kind::Region => p.region.clone(),
        Kind::Country => p.country.clone(),
        Kind::Phone => p.phone.clone(),
        Kind::Organization => p.organization.clone(),
        Kind::Url => p.url.clone(),
        Kind::Birthday => p.birthday_text(ctx.lang),
        Kind::Color => format!("#{:06x}", rng.below(0x0100_0000)),
        Kind::LongText => ctx.lang.sentences(rng),
        _ => ctx.lang.text(rng),
    }
}

/// The valid value of `field` of `kind`.
#[must_use]
pub fn valid(field: &Field, kind: Kind, ctx: &Ctx) -> Fill {
    let mut rng = Rng::keyed(ctx.seed, &["valid", &field.key, kind.id()]);
    if kind == Kind::Checkbox || field.control == "checkbox" {
        return Fill::Check {
            checked: field.required || rng.below(2) == 0,
        };
    }
    if field.control == "radio" {
        return Fill::Radio {
            value: option(field, kind, ctx, &mut rng),
        };
    }
    if field.tag == "select" {
        return Fill::Select {
            values: option(field, kind, ctx, &mut rng).into_iter().collect(),
            goal: Goal::Valid,
        };
    }
    if let Some(scale) = scale(field) {
        let mut candidates = Vec::new();
        if kind == Kind::Birthday && scale == Scale::Date {
            candidates.push(ctx.persona.birthday_iso());
        }
        candidates.extend(bounds::valid_candidates(
            scale,
            &field.min,
            &field.max,
            &field.step,
            &mut rng,
        ));
        return Fill::Text {
            candidates,
            goal: Goal::Valid,
        };
    }
    let base = text_base(field, kind, ctx, &mut rng);
    let mut candidates = vec![fit(&base, field.min_length, field.max_length)];
    if kind == Kind::Email {
        candidates.push(fit("qa@example.com", field.min_length, field.max_length));
    }
    if !field.pattern.is_empty() {
        candidates.extend(pattern::sample(&field.pattern, &mut rng));
    }
    candidates.dedup();
    Fill::Text {
        candidates,
        goal: Goal::Valid,
    }
}

#[cfg(test)]
#[path = "value_tests.rs"]
mod tests;

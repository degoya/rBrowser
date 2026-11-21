//! A view's plan: for every field the page script read, its kind, the value it gets and how that
//! value breaks the field (invalid runs). Every value comes from the run's seed keyed by the
//! field's key, never by its position, so the same field gets the same value in every view even
//! when a breakpoint shows other fields; the locale picks the person (a German view gets German
//! data). A profile replaces the generated value of the fields it names - never a password, which
//! is always generated - in valid runs only. A `type=hidden` input (read only with
//! `captureHiddenInputs`) gets a profile's value or is left alone (RB-090-70).

use serde_json::{Value, json};

use crate::args::Mode;
use crate::classify::{Kind, classify};
use crate::field::{Collected, Field, cut};
use crate::invalid::{Strategy, Violation, invalid};
use crate::persona::Lang;
use crate::settings::Profile;
use crate::value::{Ctx, Fill, Goal, valid};

pub struct Options {
    pub mode: Mode,
    pub strategy: Strategy,
    pub seed: u64,
    pub profile: Option<Profile>,
    /// A valid fill ticks the consent checkboxes (`tickConsent`).
    pub tick_consent: bool,
}

/// One field of the plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Planned {
    pub index: u32,
    pub sig: String,
    pub key: String,
    pub label: String,
    pub kind: Kind,
    /// None for a field left alone (`skip`).
    pub fill: Option<Fill>,
    pub violation: Option<Violation>,
    pub skip: String,
    /// The value is the profile's.
    pub profile: bool,
    /// The page marks it required.
    pub required: bool,
    /// Its form, by position among the page's forms.
    pub form: Option<u32>,
    /// It held a value when the page was read (a later pass leaves it alone, RB-094-22).
    pub filled: bool,
    /// The page script's identity of the element (`Field::uid`), 0 for none.
    pub uid: u32,
}

impl Planned {
    /// What the page script's `apply` gets for this field; none for a skipped one.
    #[must_use]
    pub fn action(&self) -> Option<Value> {
        let mut action = serde_json::to_value(self.fill.as_ref()?).ok()?;
        let object = action.as_object_mut()?;
        object.insert("index".to_owned(), json!(self.index));
        object.insert("sig".to_owned(), json!(self.sig));
        Some(action)
    }

    /// The field across the reads of one view's passes: the page script's identity, else (none
    /// given) its key. Unlike the key it does not move when the page inserts a field before it.
    #[must_use]
    pub fn identity(&self) -> String {
        if self.uid > 0 {
            format!("#{}", self.uid)
        } else {
            self.key.clone()
        }
    }
}

/// The text a result shows for a field: its label, else its placeholder, name, id or key.
#[must_use]
pub fn label(field: &Field) -> String {
    [
        &field.label,
        &field.placeholder,
        &field.name,
        &field.id,
        &field.key,
    ]
    .into_iter()
    .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
    .find(|text| !text.is_empty())
    .map(|text| cut(&text, 80))
    .unwrap_or_default()
}

/// The profile's value for `field`: by its name, its id, then - unless `named_only` - its kind.
fn from_profile(profile: &Profile, field: &Field, kind: Kind, named_only: bool) -> Option<Fill> {
    if kind == Kind::Password || field.control == "password" {
        return None;
    }
    let by_kind = if named_only { "" } else { kind.id() };
    let value = [field.name.as_str(), field.id.as_str(), by_kind]
        .into_iter()
        .filter(|key| !key.is_empty())
        .find_map(|key| profile.values.get(key))?
        .clone();
    Some(match field.control.as_str() {
        "checkbox" => Fill::Check {
            checked: matches!(
                value.to_ascii_lowercase().as_str(),
                "true" | "on" | "1" | "yes"
            ),
        },
        "radio" => Fill::Radio { value: Some(value) },
        _ if field.tag == "select" => Fill::Select {
            values: vec![value],
            goal: Goal::Any,
        },
        _ => Fill::Text {
            candidates: vec![value],
            goal: Goal::Any,
        },
    })
}

/// The plan of `page`.
#[must_use]
pub fn plan(page: &Collected, options: &Options) -> Vec<Planned> {
    let mut ctx = Ctx::new(options.seed, Lang::from_locale(&page.locale));
    ctx.tick_consent = options.tick_consent;
    page.fields
        .iter()
        .map(|field| {
            let kind = classify(field);
            let mut planned = Planned {
                index: field.index,
                sig: field.sig.clone(),
                key: field.key.clone(),
                label: label(field),
                kind,
                fill: None,
                violation: None,
                skip: field.skip.clone(),
                profile: false,
                required: field.required,
                form: field.form,
                filled: field.filled,
                uid: field.uid,
            };
            if !field.skip.is_empty() {
                return planned;
            }
            if field.control == "hidden" {
                // A hidden input (RB-090-70, `captureHiddenInputs`): a profile's value by its name
                // or id, never by its kind (`email` is no value for `newsletter_email_hash`).
                planned.fill = match (options.mode, &options.profile) {
                    (Mode::Valid, Some(profile)) => from_profile(profile, field, kind, true),
                    _ => None,
                };
                planned.profile = planned.fill.is_some();
                if planned.fill.is_none() {
                    planned.skip = "hidden-input".to_owned();
                }
                return planned;
            }
            let broken = match options.mode {
                Mode::Invalid => invalid(field, options.strategy, options.seed),
                Mode::Valid => None,
            };
            let profiled = match (options.mode, &options.profile) {
                (Mode::Valid, Some(profile)) => from_profile(profile, field, kind, false),
                _ => None,
            };
            planned.profile = profiled.is_some();
            planned.violation = broken.as_ref().map(|(_, violation)| *violation);
            planned.fill = Some(
                broken
                    .map(|(fill, _)| fill)
                    .or(profiled)
                    .unwrap_or_else(|| valid(field, kind, &ctx)),
            );
            planned
        })
        .collect()
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;

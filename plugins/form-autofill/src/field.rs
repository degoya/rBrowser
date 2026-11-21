//! A view's form fields as the page script (`src/page.js`, `collect`) reads them through
//! `evaluate`, held to the plugin's limits again: the page shapes all of it, so every text is cut,
//! every list capped and a value of the wrong type reads as its default.

use rb_plugin_api::{Failure, FailureKind, codes};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// The most fields read per view (the page script stops at the same number).
pub const MAX_FIELDS: usize = 300;
/// The most options of one select or radio group.
pub const MAX_OPTIONS: usize = 200;
/// The longest hint kept, in characters.
pub const MAX_TEXT: usize = 200;
/// The longest `pattern` kept, in characters.
pub const MAX_PATTERN: usize = 1024;
/// The most forms of a page named (the page script stops at the same number).
pub const MAX_FORMS: usize = 50;

/// `text` cut to `max` characters.
#[must_use]
pub fn cut(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(cut(value.as_str().unwrap_or_default(), MAX_TEXT))
}

fn pattern<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(cut(value.as_str().unwrap_or_default(), MAX_PATTERN))
}

fn flag<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Ok(Value::deserialize(deserializer)?.as_bool().unwrap_or(false))
}

fn index<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(value
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(0))
}

/// A position (a form's among the page's forms), else none.
fn position<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u32>, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(value.as_u64().and_then(|n| u32::try_from(n).ok()))
}

/// A length limit: a whole number from 0 to a million, else none (`-1` is the DOM's "none").
fn length<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u32>, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(value
        .as_u64()
        .filter(|n| *n <= 1_000_000)
        .and_then(|n| u32::try_from(n).ok()))
}

/// One option of a select or radio group.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Choice {
    #[serde(deserialize_with = "text")]
    pub value: String,
    #[serde(deserialize_with = "text")]
    pub label: String,
    #[serde(deserialize_with = "flag")]
    pub disabled: bool,
}

/// One form of the page: a selector that finds it alone (the tool's `selector`) and its name
/// (`aria-label`, the text of `aria-labelledby`, `name`, `title` or `id`), for a choice of forms.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Form {
    #[serde(deserialize_with = "pattern")]
    pub selector: String,
    #[serde(deserialize_with = "text")]
    pub name: String,
}

/// One field: a control, or a radio group (one entry, its radios as options).
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Field {
    /// The position among the page script's fields, which `apply` finds it by.
    #[serde(deserialize_with = "index")]
    pub index: u32,
    /// The same field in every view: tag and name, id or position.
    #[serde(deserialize_with = "text")]
    pub key: String,
    /// Tag, type, name and id: `apply` checks the field is still the one read.
    #[serde(deserialize_with = "text")]
    pub sig: String,
    #[serde(deserialize_with = "text")]
    pub tag: String,
    /// The control: the input's `type` (`text`, `email`, `radio`, ...), `select` or `textarea`.
    #[serde(rename = "type", deserialize_with = "text")]
    pub control: String,
    #[serde(deserialize_with = "text")]
    pub name: String,
    #[serde(deserialize_with = "text")]
    pub id: String,
    #[serde(deserialize_with = "text")]
    pub autocomplete: String,
    #[serde(deserialize_with = "text")]
    pub label: String,
    #[serde(deserialize_with = "text")]
    pub placeholder: String,
    #[serde(deserialize_with = "pattern")]
    pub pattern: String,
    #[serde(deserialize_with = "text")]
    pub min: String,
    #[serde(deserialize_with = "text")]
    pub max: String,
    #[serde(deserialize_with = "text")]
    pub step: String,
    #[serde(deserialize_with = "length")]
    pub min_length: Option<u32>,
    #[serde(deserialize_with = "length")]
    pub max_length: Option<u32>,
    #[serde(deserialize_with = "flag")]
    pub required: bool,
    #[serde(deserialize_with = "flag")]
    pub multiple: bool,
    pub options: Vec<Choice>,
    /// Why the page script leaves it alone (`file`, `disabled`, `readonly`, `hidden`), or empty.
    #[serde(deserialize_with = "text")]
    pub skip: String,
    /// The form it belongs to, by its position in [`Collected::forms`]; none outside a form.
    #[serde(deserialize_with = "position")]
    pub form: Option<u32>,
    /// It holds a value already - a text, a checked box or radio, an option chosen - so a later
    /// pass of a fill leaves it alone (RB-094-22).
    #[serde(deserialize_with = "flag")]
    pub filled: bool,
    /// The element's identity in this view, the same in every read of its document (the page
    /// script keeps it in the isolated world); 0 for none. The passes tell fields apart by it.
    #[serde(deserialize_with = "index")]
    pub uid: u32,
}

/// One view's fields.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Collected {
    /// The locale the view's page runs in (`Intl`, then `navigator.language`).
    #[serde(deserialize_with = "text")]
    pub locale: String,
    #[serde(deserialize_with = "text")]
    pub url: String,
    pub fields: Vec<Field>,
    /// The forms of the whole document, whatever `selector` names.
    pub forms: Vec<Form>,
    /// `selector` (not a valid selector) or `not-found` (nothing matches), else empty.
    #[serde(deserialize_with = "text")]
    pub error: String,
    #[serde(deserialize_with = "flag")]
    pub truncated: bool,
    /// Which controls the fill runs for (RB-090-86): `selector` (the selector's element) or
    /// `page` (the page's but its navigation and, outside a form, its disclosure toggles).
    #[serde(deserialize_with = "text")]
    pub scope: String,
}

impl Collected {
    /// The page script's answer, its lists capped.
    ///
    /// # Errors
    ///
    /// `plugins.error.bad_output` when the answer is no object.
    pub fn parse(answer: &str) -> Result<Self, Failure> {
        let bad = || Failure::coded(FailureKind::BadData, codes::BAD_OUTPUT);
        let value: Value = serde_json::from_str(answer).map_err(|_| bad())?;
        if !value.is_object() {
            return Err(bad());
        }
        let mut page: Self = serde_json::from_value(value).map_err(|_| bad())?;
        page.locale = cut(&page.locale, 35);
        page.fields.truncate(MAX_FIELDS);
        page.forms.truncate(MAX_FORMS);
        let forms = page.forms.len();
        for field in &mut page.fields {
            field.options.truncate(MAX_OPTIONS);
            field.form = field
                .form
                .filter(|at| usize::try_from(*at).is_ok_and(|at| at < forms));
        }
        Ok(page)
    }
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;

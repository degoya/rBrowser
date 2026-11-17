//! Which fields a preset never takes a value from (RB-090-70): a password field and a field whose
//! hints name a secret - an `autocomplete` token of a password, a one-time code or card data, or
//! a name, id, label or placeholder with a word of one (`csrf_token`, `userPin`, "Security
//! code"). The page script applies the same rule before a value leaves the page
//! (`src/page.js`, `secretOf`); this side checks again what comes back, and a locator a caller
//! saves (`autofill_save`). `tests/fixtures/sensitive-fields.json` holds both to one behaviour.

use crate::field::Field;

/// `autocomplete` tokens of a password.
const PASSWORD_TOKENS: &[&str] = &["current-password", "new-password"];
/// `autocomplete` tokens of a one-time code and card data.
const SECRET_TOKENS: &[&str] = &[
    "one-time-code",
    "cc-number",
    "cc-csc",
    "cc-exp",
    "cc-exp-month",
    "cc-exp-year",
];
/// Whole words that name a secret.
const WORDS: &[&str] = &[
    "pin", "pincode", "otp", "cvv", "cvc", "csc", "pwd", "pass", "passcode", "ssn",
];
/// Parts of the words run together that name a secret.
const PARTS: &[&str] = &[
    "password",
    "passwort",
    "passwd",
    "kennwort",
    "secret",
    "token",
    "csrf",
    "xsrf",
    "apikey",
    "onetime",
    "securitycode",
    "verificationcode",
    "cardnumber",
    "creditcard",
    "ccnumber",
];

/// The words of `text`: split at a lower-case letter or digit followed by an upper-case one and
/// at every character that is no ASCII letter or digit, lower-cased.
#[must_use]
pub fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut previous_lower = false;
    for c in text.chars() {
        if !c.is_ascii_alphanumeric() {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            previous_lower = false;
            continue;
        }
        if c.is_ascii_uppercase() && previous_lower && !word.is_empty() {
            out.push(std::mem::take(&mut word));
        }
        previous_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        word.push(c.to_ascii_lowercase());
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// Whether `text` (a name, id, label, placeholder or a preset's locator) names a secret.
#[must_use]
pub fn secret_text(text: &str) -> bool {
    let words = words(text);
    let compact = words.concat();
    words.iter().any(|word| WORDS.contains(&word.as_str()))
        || PARTS.iter().any(|part| compact.contains(part))
}

/// Why a preset takes no value from `field`: `password` (a password field, or its
/// `autocomplete` asks for one) or `secret` (its hints name a secret); none for any other.
#[must_use]
pub fn reason(field: &Field) -> Option<&'static str> {
    let tokens: Vec<String> = field
        .autocomplete
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect();
    let has = |list: &[&str]| tokens.iter().any(|token| list.contains(&token.as_str()));
    if field.control.eq_ignore_ascii_case("password") || has(PASSWORD_TOKENS) {
        return Some("password");
    }
    let named = [&field.name, &field.id, &field.label, &field.placeholder]
        .into_iter()
        .any(|text| secret_text(text));
    (has(SECRET_TOKENS) || named).then_some("secret")
}

#[cfg(test)]
#[path = "secret_tests.rs"]
mod tests;

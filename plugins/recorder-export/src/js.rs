//! TypeScript text from recorded strings: quoted literals, secret placeholders as environment
//! variables, one-line comments, numbers, and URLs relative to the base URL.
//!
//! Nothing recorded reaches the generated code unquoted: a string is a single-quoted literal
//! with every character escaped that could end it or the line; a step's name only ever becomes
//! a `//` comment, cut to one line. Only a `fill` the recorder marked `secret` reads the
//! environment ([`secret`]); text anywhere else that looks like a placeholder - typed, from a
//! page, in a URL or an assertion - stays a literal, so no page can make the code send a secret
//! somewhere.

use std::collections::BTreeSet;

/// The prefix of every secret's environment variable.
pub const SECRET_ENV_PREFIX: &str = "RB_SECRET_";

const PLACEHOLDER_OPEN: &str = "{{secret:";
const PLACEHOLDER_CLOSE: &str = "}}";

/// `text` as a single-quoted TypeScript string literal.
pub fn string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    for c in text.chars() {
        match c {
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if c.is_control() => {
                let code = u32::from(c);
                if code <= 0xff {
                    out.push_str(&format!("\\x{code:02x}"));
                } else {
                    out.push_str(&format!("\\u{code:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

/// The environment variables of the secrets a file uses.
#[derive(Clone, Debug, Default)]
pub struct Secrets(BTreeSet<String>);

impl Secrets {
    /// Their names, sorted.
    pub fn names(&self) -> Vec<String> {
        self.0.iter().cloned().collect()
    }
}

/// The function a file with secrets defines once, before its steps: the environment variable's
/// value, or an error naming the variable when it is not set - a step never fills `''` in
/// silence.
pub const SECRET_HELPER: &str = "function rbSecret(name: string): string {\n  \
     const value = process.env[name];\n  \
     if (value === undefined || value === '') {\n    \
     throw new Error(`The secret ${name} is not set in the environment.`);\n  \
     }\n  \
     return value;\n\
     }\n";

/// The value of a `fill` the recorder marked secret: exactly one `{{secret:<name>}}`
/// placeholder, as the call `rbSecret('RB_SECRET_<NAME>')` ([`SECRET_HELPER`], the variable
/// recorded in `secrets`); `None` for any other text. The secret's value is never known here -
/// the recorder kept only the placeholder.
pub fn secret(text: &str, secrets: &mut Secrets) -> Option<String> {
    let name = text
        .strip_prefix(PLACEHOLDER_OPEN)?
        .strip_suffix(PLACEHOLDER_CLOSE)?;
    if !valid_secret_name(name) {
        return None;
    }
    let variable = env_name(name);
    let call = format!("rbSecret({})", string(&variable));
    secrets.0.insert(variable);
    Some(call)
}

/// A secret's name as the recorder writes it: `[A-Za-z0-9_-]`, 1 to 64 characters.
fn valid_secret_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// `RB_SECRET_<NAME>`: upper case, `-` as `_`.
fn env_name(name: &str) -> String {
    let mut variable = String::from(SECRET_ENV_PREFIX);
    variable.extend(name.chars().map(|c| match c {
        '-' => '_',
        c => c.to_ascii_uppercase(),
    }));
    variable
}

/// `text` for a `//` comment: one line, control characters as spaces, runs of space as one.
pub fn comment(text: &str) -> String {
    text.split(|c: char| c.is_control() || c.is_whitespace())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `value` as a TypeScript number; `0` for what is not finite.
pub fn number(value: f64) -> String {
    if value.is_finite() {
        format!("{value}")
    } else {
        "0".to_owned()
    }
}

/// `url` relative to `base` when `base` is a bare origin (`scheme://host[:port]`, an optional
/// `/`) and `url` lies on it; `url` itself otherwise. Playwright resolves a relative URL against
/// the config's `baseURL`, so the spec runs against another origin by `RB_BASE_URL`.
pub fn relative_url(url: &str, base: Option<&str>) -> String {
    let Some(origin) = base.and_then(bare_origin) else {
        return url.to_owned();
    };
    let Some(rest) = url.strip_prefix(origin) else {
        return url.to_owned();
    };
    match rest.chars().next() {
        None => "/".to_owned(),
        Some('/') => rest.to_owned(),
        Some('?' | '#') => format!("/{rest}"),
        Some(_) => url.to_owned(),
    }
}

/// `base` without its trailing `/` when it is `http(s)://<authority>` with no path.
fn bare_origin(base: &str) -> Option<&str> {
    let origin = base.strip_suffix('/').unwrap_or(base);
    let authority = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))?;
    (!authority.is_empty() && !authority.contains(['/', '?', '#', ' '])).then_some(origin)
}

#[cfg(test)]
#[path = "js_tests.rs"]
mod tests;

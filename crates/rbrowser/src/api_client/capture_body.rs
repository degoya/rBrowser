//! The post data of a captured request as a body (RB-096-08), every secret field a variable: a
//! JSON body - by its `Content-Type`, or because the text parses as an object or an array (a
//! `fetch` of `JSON.stringify(...)` without headers goes out as `text/plain`) -, a form, and the
//! value parts of a `multipart/form-data` body. Any other text is taken as it is only when it
//! names no secret just before a value (`password=`, `"token":`, `<secret>`). Under a secret's
//! name in JSON every value but an object becomes a variable: a string's in its quotes, a
//! number's or a list's bare, so it is sent with its type. A body that cannot be checked - such a
//! text, a JSON string that names a secret inside (a GraphQL query's `password: "x"`), a
//! multipart body with a file part or one that does not read, JSON deeper than
//! [`MAX_JSON_DEPTH`], a line that reads as a file include (`< ./file`, the w31 security review)
//! - is `None`: left out and said, never written.

use rb_api_model::http_file::{holds_include, is_include_line};
use rb_api_model::http_parse::form_unescape;
use rb_api_model::{Body, Entry, Part};
use serde_json::Value;

use super::{Names, is_secret_param};

/// The deepest JSON body looked through.
const MAX_JSON_DEPTH: usize = 64;
/// The most parts of a multipart body taken.
const MAX_PARTS: usize = 200;

impl Names {
    /// The body of post data `text` sent as `content_type`; `None` for one that cannot be checked.
    pub(super) fn body(&mut self, content_type: &str, text: &str) -> Option<Body> {
        let kind = content_type
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let json_kind = kind == "application/json" || kind.ends_with("+json");
        if let Ok(mut value) = serde_json::from_str::<Value>(text)
            && (json_kind || value.is_object() || value.is_array())
        {
            let mut bare = Vec::new();
            let text = if self.json(&mut value, 0, &mut bare)? {
                let mut pretty = serde_json::to_string_pretty(&value).ok()?;
                for variable in bare {
                    pretty = pretty.replace(&format!("\"{variable}\""), &variable);
                }
                pretty
            } else {
                text.to_owned()
            };
            return Some(Body::Json { text });
        }
        if kind == "application/x-www-form-urlencoded" {
            let fields = self.form(text);
            // The body is one line from its first name on.
            if fields
                .first()
                .is_some_and(|field| is_include_line(&field.name))
            {
                return None;
            }
            return Some(Body::Form { fields });
        }
        if kind == "multipart/form-data" {
            return self.multipart(content_type, text);
        }
        (!may_hold_secret(text) && !holds_include(text)).then(|| Body::Raw {
            text: text.to_owned(),
        })
    }

    /// Every value under a secret's name in `value` as a variable, the ones written bare into
    /// `bare`: whether one changed; `None` past [`MAX_JSON_DEPTH`] or for a string that names a
    /// secret inside.
    fn json(&mut self, value: &mut Value, depth: usize, bare: &mut Vec<String>) -> Option<bool> {
        let mut changed = false;
        match value {
            Value::Object(_) | Value::Array(_) if depth > MAX_JSON_DEPTH => return None,
            Value::Object(map) => {
                for (key, item) in map.iter_mut() {
                    if is_secret_param(key)
                        && let Some(variable) = self.secret_value(key, item, bare)
                    {
                        *item = Value::String(variable);
                        changed = true;
                    } else {
                        changed |= self.json(item, depth + 1, bare)?;
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    changed |= self.json(item, depth + 1, bare)?;
                }
            }
            Value::String(text) if may_hold_secret(text) => return None,
            _ => {}
        }
        Some(changed)
    }

    /// The variable for `item` under the secret's name `key`: a string's (none for an empty
    /// one), a number's or a list's without an object (bare: noted in `bare`); none for an
    /// object, a list that holds one, `null` or a boolean - those are looked through.
    fn secret_value(&mut self, key: &str, item: &Value, bare: &mut Vec<String>) -> Option<String> {
        match item {
            Value::String(text) if !text.is_empty() => Some(self.variable(key, text)),
            Value::Number(_) => self.bare_variable(key, item, bare),
            Value::Array(items) if !items.is_empty() && !items.iter().any(holds_object) => {
                self.bare_variable(key, item, bare)
            }
            _ => None,
        }
    }

    fn bare_variable(&mut self, key: &str, item: &Value, bare: &mut Vec<String>) -> Option<String> {
        let variable = self.variable(key, &serde_json::to_string(item).ok()?);
        bare.push(variable.clone());
        Some(variable)
    }

    fn form(&mut self, text: &str) -> Vec<Entry> {
        text.split('&')
            .filter(|pair| !pair.is_empty())
            .map(|pair| {
                let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
                let (name, value) = (form_unescape(name), form_unescape(value));
                let value = if is_secret_param(&name) && !value.is_empty() {
                    self.variable(&name, &value)
                } else {
                    value
                };
                Entry::new(name, value)
            })
            .collect()
    }

    /// The value parts of a multipart body; `None` for a file part, a part that does not read,
    /// a value that may hold a secret under another name, or a body without its closing line.
    fn multipart(&mut self, content_type: &str, text: &str) -> Option<Body> {
        let boundary = param(content_type, "boundary").filter(|boundary| !boundary.is_empty())?;
        let open = format!("--{boundary}");
        let mut sections = text.split(open.as_str());
        if !sections.next()?.trim().is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        for section in sections {
            if section.starts_with("--") {
                return Some(Body::Multipart { parts });
            }
            if parts.len() >= MAX_PARTS {
                return None;
            }
            let section = section
                .strip_prefix("\r\n")
                .or_else(|| section.strip_prefix('\n'))?;
            let (head, value) = section
                .split_once("\r\n\r\n")
                .or_else(|| section.split_once("\n\n"))?;
            let value = value
                .strip_suffix("\r\n")
                .or_else(|| value.strip_suffix('\n'))
                .unwrap_or(value);
            let (mut name, mut part_type) = (None, None);
            for line in head.lines() {
                let (key, rest) = line.split_once(':')?;
                let key = key.trim();
                if key.eq_ignore_ascii_case("content-disposition") {
                    if param(rest, "filename").is_some() || param(rest, "filename*").is_some() {
                        return None;
                    }
                    name = param(rest, "name");
                } else if key.eq_ignore_ascii_case("content-type") {
                    part_type = Some(rest.trim().to_owned());
                }
            }
            let name = name?;
            let value = if is_secret_param(&name) && !value.is_empty() {
                self.variable(&name, value)
            } else if may_hold_secret(value) || holds_include(value) {
                return None;
            } else {
                value.to_owned()
            };
            parts.push(Part {
                name,
                value,
                file: None,
                content_type: part_type,
            });
        }
        None
    }
}

/// Whether `value` is or holds an object.
fn holds_object(value: &Value) -> bool {
    match value {
        Value::Object(_) => true,
        Value::Array(items) => items.iter().any(holds_object),
        _ => false,
    }
}

/// The value of parameter `key` of a header value (`form-data; name="a"`, `...; boundary=x`).
fn param(header: &str, key: &str) -> Option<String> {
    header.split(';').skip(1).find_map(|item| {
        let (name, value) = item.split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case(key)
            .then(|| value.trim().trim_matches('"').to_owned())
    })
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// Whether `text` names a secret just before a value: `password=`, `"token":`, `<secret>`.
fn may_hold_secret(text: &str) -> bool {
    let mut rest = text;
    while let Some(start) = rest.find(is_word) {
        let word_on = &rest[start..];
        let end = word_on.find(|c: char| !is_word(c)).unwrap_or(word_on.len());
        let (word, tail) = word_on.split_at(end);
        if is_secret_param(word) {
            let next =
                tail.trim_start_matches(|c: char| c.is_whitespace() || c == '"' || c == '\'');
            if next.starts_with(['=', ':', '>']) {
                return true;
            }
        }
        rest = tail;
    }
    false
}

#[cfg(test)]
#[path = "capture_body_tests.rs"]
mod tests;

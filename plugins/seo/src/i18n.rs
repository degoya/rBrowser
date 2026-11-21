//! The plugin's English catalogue, compiled in: the host shows the findings in the person's
//! language from `locales/<lang>.json`, but the report the component writes and the MCP answer
//! need their text here, and phase 1 is English only.

use std::sync::OnceLock;

use serde_json::Value;

const EN: &str = include_str!("../locales/en.json");

/// The catalogue, parsed at its first use.
fn catalogue() -> &'static Value {
    static CATALOGUE: OnceLock<Value> = OnceLock::new();
    CATALOGUE.get_or_init(|| serde_json::from_str(EN).unwrap_or(Value::Null))
}

fn lookup<'a>(catalogue: &'a Value, key: &str) -> Option<&'a str> {
    key.split('.')
        .try_fold(catalogue, |node, part| node.get(part))
        .and_then(Value::as_str)
}

/// The English text of `key` with its `{name}` placeholders filled; the key itself when the
/// catalogue has no text for it.
#[must_use]
pub fn text(key: &str, params: &[(String, String)]) -> String {
    let Some(template) = lookup(catalogue(), key) else {
        return key.to_owned();
    };
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let name = &after[..close];
                match params.iter().find(|(key, _)| key == name) {
                    Some((_, value)) => out.push_str(value),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
#[path = "i18n_tests.rs"]
mod tests;

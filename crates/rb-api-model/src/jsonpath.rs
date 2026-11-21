//! The JSONPath subset of the checks (RB-096-04), the panel's own (`plugins/api-client/web/src/
//! panel/jsonpath.ts`): `$`, `.name`, `['name']`, `[0]`, `[-1]`, `[*]`, `.*` and `..name` (every
//! depth). Anything else - filters, slices, unions - is no path.

use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    Child(String),
    Index(i64),
    All,
    Deep(String),
}

/// The length of a name at the start of `text` (`[A-Za-z_$][\w$-]*`).
fn name_len(text: &str) -> usize {
    let mut len = 0;
    for (at, c) in text.char_indices() {
        let ok = if at == 0 {
            c.is_ascii_alphabetic() || c == '_' || c == '$'
        } else {
            c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '-')
        };
        if !ok {
            break;
        }
        len = at + c.len_utf8();
    }
    len
}

fn bracket(inner: &str) -> Option<Step> {
    let inner = inner.trim();
    if inner == "*" {
        return Some(Step::All);
    }
    if let Ok(index) = inner.parse::<i64>() {
        return Some(Step::Index(index));
    }
    let quoted = |q: char| {
        inner
            .strip_prefix(q)
            .and_then(|rest| rest.strip_suffix(q))
            .filter(|name| !name.contains(q))
    };
    quoted('\'')
        .or_else(|| quoted('"'))
        .map(|name| Step::Child(name.to_owned()))
}

fn steps(path: &str) -> Option<Vec<Step>> {
    let text = path.trim();
    let mut rest = text.strip_prefix('$')?;
    let mut out = Vec::new();
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("..") {
            let len = name_len(after);
            if len == 0 {
                return None;
            }
            out.push(Step::Deep(after[..len].to_owned()));
            rest = &after[len..];
        } else if let Some(after) = rest.strip_prefix(".*") {
            out.push(Step::All);
            rest = after;
        } else if let Some(after) = rest.strip_prefix('.') {
            let len = name_len(after);
            if len == 0 {
                return None;
            }
            out.push(Step::Child(after[..len].to_owned()));
            rest = &after[len..];
        } else {
            let after = rest.strip_prefix('[')?;
            let close = after.find(']')?;
            out.push(bracket(&after[..close])?);
            rest = &after[close + 1..];
        }
    }
    Some(out)
}

fn children(value: &Value) -> Vec<&Value> {
    match value {
        Value::Array(items) => items.iter().collect(),
        Value::Object(map) => map.values().collect(),
        _ => Vec::new(),
    }
}

fn deep<'a>(value: &'a Value, name: &str, out: &mut Vec<&'a Value>) {
    if let Value::Object(map) = value
        && let Some(found) = map.get(name)
    {
        out.push(found);
    }
    for child in children(value) {
        deep(child, name, out);
    }
}

/// What `path` selects in `value`, in document order; `None` when `path` is no path of the
/// subset.
#[must_use]
pub fn select<'a>(value: &'a Value, path: &str) -> Option<Vec<&'a Value>> {
    let mut current = vec![value];
    for step in steps(path)? {
        let mut next = Vec::new();
        for item in current {
            match (&step, item) {
                (Step::All, _) => next.extend(children(item)),
                (Step::Deep(name), _) => deep(item, name, &mut next),
                (Step::Index(index), Value::Array(items)) => {
                    let len = i64::try_from(items.len()).unwrap_or(i64::MAX);
                    let at = if *index < 0 { len + index } else { *index };
                    if let Some(found) = usize::try_from(at).ok().and_then(|at| items.get(at)) {
                        next.push(found);
                    }
                }
                (Step::Child(name), Value::Object(map)) => {
                    if let Some(found) = map.get(name) {
                        next.push(found);
                    }
                }
                _ => {}
            }
        }
        current = next;
    }
    Some(current)
}

#[cfg(test)]
#[path = "jsonpath_tests.rs"]
mod tests;

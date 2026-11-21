//! `{{variables}}` in a request's texts, as VS Code REST Client and the JetBrains HTTP Client
//! write them: `{{name}}` takes a variable of the environment ([`crate::env`]), `{{$name args}}`
//! a dynamic one the caller knows (`{{$processEnv RB_SECRET_TOKEN}}`, `{{$timestamp}}`). A
//! variable's value may itself hold variables, resolved up to [`MAX_DEPTH`] deep. What is not a
//! variable's form (`{{ 1 + 1 }}`, a lone `{{`) stays as it is.

/// How deep a variable's value is resolved again.
pub const MAX_DEPTH: usize = 4;

/// A variable's value, and whether it is a secret (masked wherever it could leave the core).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Value {
    pub text: String,
    pub secret: bool,
}

impl Value {
    /// A value that is no secret.
    #[must_use]
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            secret: false,
        }
    }

    /// A secret value.
    #[must_use]
    pub fn secret(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            secret: true,
        }
    }
}

/// Where the variables come from.
pub trait Lookup {
    /// The variable `name`.
    fn variable(&self, name: &str) -> Option<Value>;
    /// The dynamic variable `name` (with its `$`) and the text after it.
    fn dynamic(&self, name: &str, args: &str) -> Option<Value>;
}

/// A text with every variable put in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resolved {
    pub text: String,
    /// The secret values put in, for the mask.
    pub secrets: Vec<String>,
}

/// The variables a text names that have no value, each once, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unresolved {
    pub names: Vec<String>,
}

/// One `{{...}}` of a text.
enum Token<'a> {
    Text(&'a str),
    Variable(&'a str),
    Dynamic(&'a str, &'a str),
}

/// Whether `name` is a variable's name: a letter or `_`, then letters, digits, `_`, `-`, `.`.
#[must_use]
pub fn is_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))
}

fn tokens(template: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        let Some(close) = rest[open + 2..].find("}}") else {
            break;
        };
        let inner = rest[open + 2..open + 2 + close].trim();
        let token = if let Some(dynamic) = inner.strip_prefix('$') {
            let (name, args) = dynamic
                .split_once(char::is_whitespace)
                .unwrap_or((dynamic, ""));
            is_name(name).then(|| Token::Dynamic(&inner[..name.len() + 1], args.trim()))
        } else {
            is_name(inner).then_some(Token::Variable(inner))
        };
        match token {
            Some(token) => {
                if open > 0 {
                    out.push(Token::Text(&rest[..open]));
                }
                out.push(token);
                rest = &rest[open + 2 + close + 2..];
            }
            None => {
                out.push(Token::Text(&rest[..open + 2]));
                rest = &rest[open + 2..];
            }
        }
    }
    if !rest.is_empty() {
        out.push(Token::Text(rest));
    }
    out
}

/// The plain variables `template` names, each once, in order (no dynamic ones).
#[must_use]
pub fn references(template: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for token in tokens(template) {
        if let Token::Variable(name) = token
            && !names.iter().any(|known| known == name)
        {
            names.push(name.to_owned());
        }
    }
    names
}

/// `template` with its variables from `lookup`.
///
/// # Errors
///
/// The names without a value (a dynamic one with its `$`); nothing is half-resolved.
pub fn substitute(template: &str, lookup: &dyn Lookup) -> Result<Resolved, Unresolved> {
    let mut resolved = Resolved::default();
    let mut missing = Unresolved::default();
    put(template, lookup, 0, &mut resolved, &mut missing);
    if missing.names.is_empty() {
        resolved.secrets.sort();
        resolved.secrets.dedup();
        Ok(resolved)
    } else {
        Err(missing)
    }
}

fn put(
    template: &str,
    lookup: &dyn Lookup,
    depth: usize,
    resolved: &mut Resolved,
    missing: &mut Unresolved,
) {
    for token in tokens(template) {
        let (shown, value) = match token {
            Token::Text(text) => {
                resolved.text.push_str(text);
                continue;
            }
            Token::Variable(name) => (name.to_owned(), lookup.variable(name)),
            Token::Dynamic(name, args) => (name.to_owned(), lookup.dynamic(name, args)),
        };
        let Some(value) = value else {
            if !missing.names.contains(&shown) {
                missing.names.push(shown);
            }
            continue;
        };
        if value.secret && !value.text.is_empty() {
            resolved.secrets.push(value.text.clone());
        }
        if depth + 1 < MAX_DEPTH && value.text.contains("{{") {
            let mut inner = Resolved::default();
            put(&value.text, lookup, depth + 1, &mut inner, missing);
            if value.secret && !inner.text.is_empty() {
                resolved.secrets.push(inner.text.clone());
            }
            resolved.text.push_str(&inner.text);
            resolved.secrets.extend(inner.secrets);
        } else {
            resolved.text.push_str(&value.text);
        }
    }
}

#[cfg(test)]
#[path = "vars_tests.rs"]
mod tests;

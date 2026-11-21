//! Masking secrets in log lines and in the config of the diagnostics ZIP, after rCommander's
//! `rc-db/src/redact.rs` (rDownloader's before it).
//!
//! Two layers. [`Secrets`] holds values known to be secret - the MCP token, the CDP proxy token,
//! header values from the config - registered at runtime ([`register_secret`]) and masked
//! wherever they stand. [`redact_text`] finds secrets by their shape: a secret's value after its
//! name (`token=x`, `"password": "x"`, `RB_MCP_TOKEN=x`), credential header lines
//! (`Authorization: ...`, `Cookie: ...`), a `Bearer`/`Basic`/`Digest` value and the user info of
//! an address (`http://user:pass@host`). Every value becomes [`PLACEHOLDER`].
//!
//! Two invariants hold: a string with nothing to hide comes back byte for byte, and redacting
//! twice equals redacting once. Names stay, only values are replaced.

use std::sync::{LazyLock, PoisonError, RwLock};

use serde_json::Value;

/// What every masked value becomes.
pub const PLACEHOLDER: &str = "***";

/// Known secrets shorter than this are not registered: masking `ab` everywhere would destroy
/// the log and protect nothing.
pub const MIN_SECRET_LEN: usize = 4;

/// Names whose value is a credential, matched case-insensitively on the whole name.
const SECRET_NAMES: &[&str] = &[
    "access_key",
    "access_token",
    "api-key",
    "api_key",
    "apikey",
    "auth",
    "auth_token",
    "authorization",
    "client_secret",
    "cookie",
    // `breakpoints[].iosDevice`: a device's UDID identifies the device (RB-095-04).
    "iosdevice",
    "key",
    "passphrase",
    "passwd",
    "password",
    "private_key",
    "proxy-authorization",
    "pwd",
    "refresh_token",
    "secret",
    "session",
    "set-cookie",
    "sid",
    "token",
    "x-api-key",
];

/// Parts of a name that make it a secret's wherever they stand in it (`DB_PASSWORD`).
const SECRET_PARTS: &[&str] = &["passphrase", "passwd", "password", "private_key", "secret"];

/// Words of a name, between `_`, `-` and `.`, that make it a secret's (`RB_MCP_TOKEN`,
/// `cdp.token`); as words only, so `max_tokens` stays.
const SECRET_WORDS: &[&str] = &["apikey", "authtoken", "pwd", "token"];

/// Names of [`SECRET_NAMES`] too common in prose for the colon form: `key: F5`, `session: 3`.
const COLON_AMBIGUOUS: &[&str] = &["auth", "key", "session", "sid"];

/// Header names whose value is always a credential.
const CREDENTIAL_HEADERS: &[&str] = &[
    "authorization",
    "cookie",
    "proxy-authorization",
    "set-cookie",
];

/// Authentication schemes whose parameter is a credential.
const CREDENTIAL_SCHEMES: &[&str] = &["bearer ", "basic ", "digest "];

/// The scheme masked in free text, with its case: `Basic` and `Digest` values only follow an
/// `Authorization` name (masked whole), while `basic layout` in prose must stay.
const INLINE_SCHEME: &str = "Bearer ";

/// Address schemes searched for inside text.
const SCHEMES: &[&str] = &[
    "https://",
    "http://",
    "wss://",
    "ws://",
    "socks5://",
    "socks4://",
    "ftp://",
];

/// A set of values known to be secret, masked wherever they appear.
#[derive(Debug, Default)]
pub struct Secrets {
    values: RwLock<Vec<String>>,
}

impl Secrets {
    /// Registers `value`; ignored when shorter than [`MIN_SECRET_LEN`] or known already.
    pub fn add(&self, value: &str) {
        if value.chars().count() < MIN_SECRET_LEN || value == PLACEHOLDER {
            return;
        }
        let mut values = self.values.write().unwrap_or_else(PoisonError::into_inner);
        if !values.iter().any(|known| known == value) {
            values.push(value.to_owned());
            // The longest first, so a secret that contains another is masked whole.
            values.sort_by_key(|known| std::cmp::Reverse(known.len()));
        }
    }

    /// Forgets `value`: a secret that was given up (a stopped endpoint's token), so the set
    /// does not grow with every run.
    pub fn remove(&self, value: &str) {
        let mut values = self.values.write().unwrap_or_else(PoisonError::into_inner);
        values.retain(|known| known != value);
    }

    /// How many secrets are known.
    pub fn len(&self) -> usize {
        self.values
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Whether no secret is known.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `text` with every known secret masked, then [`redact_text`] applied.
    pub fn redact(&self, text: &str) -> String {
        let values = self.values.read().unwrap_or_else(PoisonError::into_inner);
        let mut scrubbed = text.to_owned();
        for value in values.iter() {
            if scrubbed.contains(value.as_str()) {
                scrubbed = scrubbed.replace(value.as_str(), PLACEHOLDER);
            }
        }
        redact_text(&scrubbed)
    }
}

static GLOBAL: LazyLock<Secrets> = LazyLock::new(Secrets::default);

/// Registers a secret of the running process (the MCP token, the CDP proxy token, a header
/// value): from now on no log line and no diagnostics ZIP shows it.
pub fn register_secret(value: &str) {
    GLOBAL.add(value);
}

/// Forgets a secret registered with [`register_secret`] once nothing can use it any more (the
/// token of a stopped endpoint); later lines are no longer masked for it.
pub fn unregister_secret(value: &str) {
    GLOBAL.remove(value);
}

/// `text` with the registered secrets and every secret [`redact_text`] finds masked.
pub fn redact(text: &str) -> String {
    GLOBAL.redact(text)
}

/// A JSON value after [`redact_json`]: the masked value and the original strings it removed (so
/// the ZIP can mask them in the logs too).
#[derive(Clone, Debug, PartialEq)]
pub struct RedactedJson {
    /// The value with every secret replaced by [`PLACEHOLDER`].
    pub value: Value,
    /// The removed strings: masked values, and the user info and password of addresses.
    pub removed: Vec<String>,
}

/// `value` with the secrets of a config masked: every value under a secret's name (`token`,
/// `password`, `cookie`, ...; not the ambiguous `key`, `session`, `auth`, `sid`, which name
/// ordinary config keys) or under `headers`, and every string [`redact_text`] would change.
pub fn redact_json(value: &Value) -> RedactedJson {
    let mut removed = Vec::new();
    let value = walk(value, &mut removed);
    RedactedJson { value, removed }
}

fn walk(value: &Value, removed: &mut Vec<String>) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (key, child) in map {
                let masked = if is_secret_key(key) {
                    mask(child, removed)
                } else if key.eq_ignore_ascii_case("headers")
                    && let Value::Object(headers) = child
                {
                    let headers = headers
                        .iter()
                        .map(|(name, header)| (name.clone(), mask(header, removed)))
                        .collect();
                    Value::Object(headers)
                } else {
                    walk(child, removed)
                };
                out.insert(key.clone(), masked);
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(|item| walk(item, removed)).collect()),
        Value::String(text) => {
            let redacted = redact_text(text);
            if redacted != *text {
                removed.extend(user_infos(text));
            }
            Value::String(redacted)
        }
        other => other.clone(),
    }
}

/// [`PLACEHOLDER`] for `value`, recording every string inside it.
fn mask(value: &Value, removed: &mut Vec<String>) -> Value {
    collect_strings(value, removed);
    Value::String(PLACEHOLDER.to_owned())
}

fn collect_strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) if !text.is_empty() && text != PLACEHOLDER => out.push(text.clone()),
        Value::Array(items) => items.iter().for_each(|item| collect_strings(item, out)),
        Value::Object(map) => map.values().for_each(|item| collect_strings(item, out)),
        _ => {}
    }
}

fn is_secret_key(key: &str) -> bool {
    names_a_secret(key, ':') || CREDENTIAL_HEADERS.contains(&key.to_ascii_lowercase().as_str())
}

/// The user info (`user:pass`) and the password of every address in `text`.
fn user_infos(text: &str) -> Vec<String> {
    let lowered = text.to_ascii_lowercase();
    let mut out = Vec::new();
    for scheme in SCHEMES {
        let mut from = 0;
        while let Some(found) = lowered[from..].find(scheme) {
            let start = from + found + scheme.len();
            let end = authority_end(text, start);
            if let Some((info, _)) = text[start..end].rsplit_once('@')
                && !info.is_empty()
                && info != PLACEHOLDER
            {
                out.push(info.to_owned());
                if let Some((_, password)) = info.split_once(':')
                    && !password.is_empty()
                {
                    out.push(password.to_owned());
                }
            }
            from = end.max(start);
        }
    }
    out
}

/// Whether `name` is the name of a credential.
pub fn is_secret_name(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    SECRET_NAMES.contains(&name.as_str())
}

fn has_secret_part(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    SECRET_PARTS.iter().any(|part| name.contains(part))
        || name
            .split(['_', '-', '.'])
            .any(|word| SECRET_WORDS.contains(&word))
}

/// Whether the value after `name` and `separator` is a secret: `=` after any secret's name, `:`
/// only after one that is beyond doubt.
fn names_a_secret(name: &str, separator: char) -> bool {
    has_secret_part(name)
        || (is_secret_name(name)
            && (separator == '=' || !COLON_AMBIGUOUS.contains(&name.to_ascii_lowercase().as_str())))
}

/// Free text with every credential it carries masked, line by line.
pub fn redact_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (body, terminator) = match line.strip_suffix('\n') {
            Some(body) => (body, "\n"),
            None => (line, ""),
        };
        out.push_str(&redact_line(body));
        out.push_str(terminator);
    }
    out
}

fn redact_line(line: &str) -> String {
    if let Some(position) = line.find(':') {
        let name = line[..position].trim();
        if CREDENTIAL_HEADERS.contains(&name.to_ascii_lowercase().as_str()) {
            return format!("{}: {PLACEHOLDER}", line[..position].trim_end());
        }
    }
    redact_inline(line)
}

/// Addresses, auth schemes and `secret=value` inside one line. Scans by byte index; every marker
/// is ASCII, so the slices always fall on character boundaries.
fn redact_inline(line: &str) -> String {
    let lowered = line.to_ascii_lowercase();
    let mut out = String::with_capacity(line.len());
    let mut index = 0;
    while index < line.len() {
        if let Some(scheme) = SCHEMES
            .iter()
            .find(|scheme| lowered[index..].starts_with(**scheme))
        {
            let start = index + scheme.len();
            let end = authority_end(line, start);
            out.push_str(&line[index..start]);
            match line[start..end].rsplit_once('@') {
                Some((_, host)) => {
                    out.push_str(PLACEHOLDER);
                    out.push('@');
                    out.push_str(host);
                }
                None => out.push_str(&line[start..end]),
            }
            index = end;
            continue;
        }
        if is_word_start(line, index) {
            if line[index..].starts_with(INLINE_SCHEME) {
                let value_start = index + INLINE_SCHEME.len();
                let value_end = value_end(line, value_start);
                if value_end > value_start {
                    out.push_str(&line[index..value_start]);
                    out.push_str(PLACEHOLDER);
                    index = value_end;
                    continue;
                }
            }
            if let Some(next) = redact_assignment(line, index, &mut out) {
                index = next;
                continue;
            }
        }
        let next = line[index..]
            .chars()
            .next()
            .map_or(line.len(), |character| index + character.len_utf8());
        out.push_str(&line[index..next]);
        index = next;
    }
    out
}

/// A secret's value after its name at `start`: writes the name, the separator and
/// [`PLACEHOLDER`] (inside the value's quotes when it has them) and returns where the value
/// ended; `None` when there is no such value at `start`.
fn redact_assignment(line: &str, start: usize, out: &mut String) -> Option<usize> {
    let name_len = line[start..]
        .find(|character: char| !(character.is_ascii_alphanumeric() || "_-.".contains(character)))
        .unwrap_or(line.len() - start);
    if name_len == 0 {
        return None;
    }
    let name = &line[start..start + name_len];
    let mut at = start + name_len;
    // A quoted name (`"password": ...`) closes with the quote it opened with.
    if let Some(quote) = line[..start]
        .chars()
        .next_back()
        .filter(|c| "\"'".contains(*c))
        && line[at..].starts_with(quote)
    {
        at += quote.len_utf8();
    }
    at += blanks(&line[at..]);
    let separator = line[at..].chars().next().filter(|c| "=:".contains(*c))?;
    if !names_a_secret(name, separator) {
        return None;
    }
    let value_start = at + 1 + blanks(&line[at + 1..]);
    let rest = &line[value_start..];
    let first = rest.chars().next()?;
    // `Authorization: Bearer x`: the scheme's rule takes the credential, not its scheme word.
    let lowered = rest.get(..8).unwrap_or(rest).to_ascii_lowercase();
    if CREDENTIAL_SCHEMES
        .iter()
        .any(|scheme| lowered.starts_with(scheme))
    {
        return None;
    }
    if first == '"' || first == '\'' {
        let inner = value_start + 1;
        let close = closing_quote(&line[inner..], first).map_or(line.len(), |end| inner + end);
        if close == inner {
            return None;
        }
        out.push_str(&line[start..inner]);
        out.push_str(PLACEHOLDER);
        return Some(close);
    }
    if separator == ':' {
        let word = rest
            .split(|c: char| c.is_whitespace() || ",;}".contains(c))
            .next()
            .unwrap_or_default();
        if first == '[' || first == '{' || ["true", "false", "null"].contains(&word) {
            return None;
        }
    }
    let stops: &[char] = &['&', ';', ',', '"', '\'', '<', '>', '}'];
    let end = rest
        .find(|character: char| character.is_whitespace() || stops.contains(&character))
        .map_or(line.len(), |offset| value_start + offset);
    if end == value_start {
        return None;
    }
    out.push_str(&line[start..value_start]);
    out.push_str(PLACEHOLDER);
    Some(end)
}

fn blanks(text: &str) -> usize {
    text.len() - text.trim_start_matches([' ', '\t']).len()
}

/// Where the `quote` that closes a quoted value lies in `text`; a quote after a backslash does
/// not close it.
fn closing_quote(text: &str, quote: char) -> Option<usize> {
    let mut escaped = false;
    for (offset, character) in text.char_indices() {
        match character {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            _ if character == quote => return Some(offset),
            _ => {}
        }
    }
    None
}

/// Whether `index` starts a word, so `member` does not look like `basic `.
fn is_word_start(line: &str, index: usize) -> bool {
    line[..index]
        .chars()
        .next_back()
        .is_none_or(|previous| !previous.is_alphanumeric() && previous != '-' && previous != '_')
}

/// The end of the authority of an address whose authority starts at `start`.
fn authority_end(line: &str, start: usize) -> usize {
    line[start..]
        .find(|character: char| {
            character.is_whitespace() || "/?#\"'<>`\\|,;)]}".contains(character)
        })
        .map_or(line.len(), |offset| start + offset)
}

/// The end of a credential after an auth scheme.
fn value_end(line: &str, start: usize) -> usize {
    line[start..]
        .find(|character: char| character.is_whitespace() || "\"'<>,;}".contains(character))
        .map_or(line.len(), |offset| start + offset)
}

#[cfg(test)]
#[path = "redact_tests.rs"]
mod tests;

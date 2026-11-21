//! A request's variables put in for a send (RB-096-01): the environment layers
//! (`rb_api_model::env`) and the dynamic variables the core knows -
//! `{{$processEnv RB_SECRET_<NAME>}}` (REST Client's form; only names under `RB_SECRET_`, so a
//! checked-in request reads no other variable of the machine), `{{$secret <name>}}` (the same
//! variable, `RB_SECRET_<NAME>`, as the recorder's exports read it), `{{$timestamp}}` and
//! `{{$guid}}`. Every value from the environment of the process or the private layer is a secret.
//!
//! `{{$secret name}}` reads, in this order (RB-096-11): the private values (`api-client.secrets`,
//! Settings > Network > API client > Private values) of the environment in effect, the private
//! `$shared`, then the process variable `RB_SECRET_<NAME>`. So a value set in the panel ("Set
//! value") wins over one of the machine, and a run with no private values (CI) reads the
//! process's.

use rb_api_model::env::SHARED;
use rb_api_model::vars::{Lookup, Unresolved, Value, is_name, substitute};
use rb_api_model::{ApiRequest, Auth, Body, Layers};

/// The prefix of the process variables a request may read.
pub const SECRET_PREFIX: &str = "RB_SECRET_";

/// Reads a variable of the process.
pub type EnvReader = dyn Fn(&str) -> Option<String> + Send + Sync;

/// The variables of one send.
pub struct Variables<'a> {
    pub layers: Layers,
    pub env: &'a EnvReader,
    /// Seconds since the epoch, for `{{$timestamp}}`.
    pub now: u64,
}

/// The process variable `{{$secret name}}` reads: `RB_SECRET_` and the name in upper case,
/// `-` and `.` as `_`.
#[must_use]
pub fn secret_variable(name: &str) -> Option<String> {
    let name = name.trim();
    let ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c));
    ok.then(|| {
        let upper: String = name
            .chars()
            .map(|c| {
                if c == '-' || c == '.' {
                    '_'
                } else {
                    c.to_ascii_uppercase()
                }
            })
            .collect();
        format!("{SECRET_PREFIX}{upper}")
    })
}

/// The private value `{{$secret name}}` takes: the environment's, else `$shared`'s.
#[must_use]
pub fn private_secret(layers: &Layers, name: &str) -> Option<String> {
    let name = name.trim();
    if !is_name(name) {
        return None;
    }
    [layers.environment.as_deref(), Some(SHARED)]
        .into_iter()
        .flatten()
        .find_map(|block| layers.private.get(block, name))
        .map(str::to_owned)
}

fn guid() -> String {
    let mut bytes = [0u8; 16];
    if getrandom::fill(&mut bytes).is_err() {
        return String::new();
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = rb_util::hex(&bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

impl Lookup for Variables<'_> {
    fn variable(&self, name: &str) -> Option<Value> {
        self.layers.variable(name)
    }

    fn dynamic(&self, name: &str, args: &str) -> Option<Value> {
        match name {
            "$processEnv" if args.starts_with(SECRET_PREFIX) && !args.contains(' ') => {
                (self.env)(args).map(Value::secret)
            }
            "$secret" => private_secret(&self.layers, args)
                .or_else(|| (self.env)(&secret_variable(args)?))
                .map(Value::secret),
            "$timestamp" if args.is_empty() => Some(Value::plain(self.now.to_string())),
            "$guid" | "$uuid" if args.is_empty() => Some(Value::plain(guid())),
            _ => None,
        }
    }
}

/// `request` with every variable put in, and the secret values used.
///
/// # Errors
///
/// The names without a value, from every field at once.
pub fn resolve(
    request: &ApiRequest,
    lookup: &dyn Lookup,
) -> Result<(ApiRequest, Vec<String>), Unresolved> {
    let mut out = request.clone();
    let mut secrets = Vec::new();
    let mut missing = Unresolved::default();
    let mut put = |text: &mut String| match substitute(text, lookup) {
        Ok(resolved) => {
            *text = resolved.text;
            secrets.extend(resolved.secrets);
        }
        Err(unresolved) => {
            for name in unresolved.names {
                if !missing.names.contains(&name) {
                    missing.names.push(name);
                }
            }
        }
    };
    put(&mut out.url);
    for header in &mut out.headers {
        put(&mut header.name);
        put(&mut header.value);
    }
    match &mut out.body {
        Body::None => {}
        Body::Json { text } | Body::Raw { text } => put(text),
        Body::Binary { path } => put(path),
        Body::GraphQl { query, variables } => {
            put(query);
            put(variables);
        }
        Body::Form { fields } => {
            for field in fields {
                put(&mut field.name);
                put(&mut field.value);
            }
        }
        Body::Multipart { parts } => {
            for part in parts {
                put(&mut part.value);
                if let Some(file) = &mut part.file {
                    put(file);
                }
            }
        }
    }
    match &mut out.auth {
        Auth::None => {}
        Auth::Basic { username, password } | Auth::Digest { username, password } => {
            put(username);
            put(password);
        }
        Auth::Bearer { token } => put(token),
        Auth::ApiKey { name, value, .. } => {
            put(name);
            put(value);
        }
        Auth::OAuth2 {
            token_url,
            auth_url,
            client_id,
            client_secret,
            scope,
            redirect_uri,
            ..
        } => {
            for text in [
                token_url,
                auth_url,
                client_id,
                client_secret,
                scope,
                redirect_uri,
            ] {
                put(text);
            }
        }
    }
    if missing.names.is_empty() {
        secrets.sort();
        secrets.dedup();
        Ok((out, secrets))
    } else {
        Err(missing)
    }
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;

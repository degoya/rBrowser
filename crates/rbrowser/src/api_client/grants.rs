//! The hosts a project may send to (RB-096-01): the first send to a new host asks the person in
//! the panel; the answer is kept in `config.local.json` as `api-client.allowedHosts` - this
//! project, this machine, never checked in, and `x-protected`, so no MCP client writes it - and
//! the person takes a host back out in Settings > Network > API client. A grant names a host
//! without its port and scheme: `api.example.com`, `localhost`, `[::1]`.

use serde_json::Value;

/// The settings key of the grants.
pub const ALLOWED_HOSTS_KEY: &str = "api-client.allowedHosts";
/// The most hosts kept.
pub const MAX_HOSTS: usize = 500;

/// The host an `http(s)` URL sends to, as a grant names it; `None` for another scheme or no
/// host.
#[must_use]
pub fn host_of(url: &str) -> Option<String> {
    let lowered = url.trim().to_ascii_lowercase();
    if !(lowered.starts_with("http://") || lowered.starts_with("https://")) {
        return None;
    }
    rb_util::url::host_of(url.trim()).and_then(|host| normalized(&host))
}

/// `host` as a grant keeps it - lower case, no trailing dot - when it is a host name, an IPv4
/// address or a bracketed IPv6 address.
#[must_use]
pub fn normalized(host: &str) -> Option<String> {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let ok = if let Some(inner) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        !inner.is_empty()
            && inner
                .chars()
                .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
    } else {
        !host.is_empty()
            && host.len() <= 253
            && host.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && label
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            })
    };
    ok.then_some(host)
}

/// The hosts of the setting's value; what is no host is left out.
#[must_use]
pub fn list_of(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(Value::as_str)
        .filter_map(normalized)
        .take(MAX_HOSTS)
        .collect()
}

/// Whether `list` allows `host`.
#[must_use]
pub fn allowed(list: &[String], host: &str) -> bool {
    normalized(host).is_some_and(|host| list.contains(&host))
}

/// `list` with `host`, sorted, each once; `None` for no host or a full list.
#[must_use]
pub fn with_host(list: &[String], host: &str) -> Option<Vec<String>> {
    let host = normalized(host)?;
    let mut out: Vec<String> = list.to_vec();
    if !out.contains(&host) {
        if out.len() >= MAX_HOSTS {
            return None;
        }
        out.push(host);
    }
    out.sort();
    out.dedup();
    Some(out)
}

#[cfg(test)]
#[path = "grants_tests.rs"]
mod tests;

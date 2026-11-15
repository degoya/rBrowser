//! Cookies as CDP `Network` knows them: read from `Network.getCookies` with every attribute
//! (HttpOnly ones too - the CDP level, not `document.cookie`), written with `Network.setCookie`
//! and removed with `Network.deleteCookies`. A cookie's identity is its name, domain, path and
//! partition: two views show "the same cookie" when those agree.

use serde_json::{Map, Value, json};

use crate::origin;

/// The longest name plus value a cookie may have (RFC 6265bis, as Chromium enforces).
pub const MAX_COOKIE_BYTES: usize = 4096;
/// The longest path accepted.
pub const MAX_PATH_CHARS: usize = 1024;
/// The `SameSite` values CDP takes.
pub const SAME_SITE: [&str; 3] = ["Strict", "Lax", "None"];

/// One cookie of a view's cookie jar.
#[derive(Clone, Debug, PartialEq)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    /// As CDP reports it: a leading dot for a domain cookie, the bare host for a host-only one.
    pub domain: String,
    pub path: String,
    /// Seconds since the epoch; `None` for a session cookie.
    pub expires: Option<f64>,
    pub http_only: bool,
    pub secure: bool,
    pub same_site: Option<String>,
    /// The top-level site of a partitioned (CHIPS) cookie.
    pub partition: Option<String>,
}

impl Cookie {
    /// The identity two views' cookies are compared by: domain, path, name, partition.
    #[must_use]
    pub fn identity(&self) -> (String, String, String, String) {
        (
            self.domain.to_ascii_lowercase(),
            self.path.clone(),
            self.name.clone(),
            self.partition.clone().unwrap_or_default(),
        )
    }

    /// Whether `other` holds the same value with the same flags (the expiry may differ by the
    /// moment each was set, so it does not count).
    #[must_use]
    pub fn same_as(&self, other: &Self) -> bool {
        self.value == other.value
            && self.http_only == other.http_only
            && self.secure == other.secure
            && self.same_site == other.same_site
    }

    /// The attributes as the panel and MCP show them (no value).
    #[must_use]
    pub fn attributes(&self) -> Value {
        json!({
            "domain": self.domain,
            "path": self.path,
            "expires": self.expires,
            "httpOnly": self.http_only,
            "secure": self.secure,
            "sameSite": self.same_site,
            "partitioned": self.partition.is_some(),
        })
    }
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// The cookies of a `Network.getCookies` answer; an entry without a name, domain or path is
/// skipped.
///
/// # Errors
///
/// An answer that is no JSON object with a `cookies` list.
pub fn parse_cookies(answer: &str) -> Result<Vec<Cookie>, String> {
    let value: Value = serde_json::from_str(answer).map_err(|_| "cookies".to_owned())?;
    let list = value
        .get("cookies")
        .and_then(Value::as_array)
        .ok_or_else(|| "cookies".to_owned())?;
    Ok(list
        .iter()
        .filter_map(|entry| {
            let session = entry.get("session").and_then(Value::as_bool) == Some(true);
            let expires = entry
                .get("expires")
                .and_then(Value::as_f64)
                .filter(|seconds| !session && *seconds > 0.0);
            // Older Chromium sends the partition key as a string, newer as an object.
            let partition = match entry.get("partitionKey") {
                Some(Value::String(site)) if !site.is_empty() => Some(site.clone()),
                Some(Value::Object(key)) => text(&Value::Object(key.clone()), "topLevelSite"),
                _ => None,
            };
            Some(Cookie {
                name: text(entry, "name").filter(|name| !name.is_empty())?,
                value: text(entry, "value").unwrap_or_default(),
                domain: text(entry, "domain")?,
                path: text(entry, "path")?,
                expires,
                http_only: entry.get("httpOnly").and_then(Value::as_bool) == Some(true),
                secure: entry.get("secure").and_then(Value::as_bool) == Some(true),
                same_site: text(entry, "sameSite").filter(|s| SAME_SITE.contains(&s.as_str())),
                partition,
            })
        })
        .collect())
}

/// A cookie to write, checked.
#[derive(Clone, Debug, PartialEq)]
pub struct CookieSpec {
    pub name: String,
    pub value: String,
    /// `None`: a host-only cookie of the view's host.
    pub domain: Option<String>,
    pub path: String,
    pub expires: Option<f64>,
    pub http_only: bool,
    pub secure: bool,
    pub same_site: Option<String>,
    pub partition: Option<String>,
}

/// Whether `name` may be a cookie's name: printable ASCII without separators.
#[must_use]
pub fn is_cookie_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_graphic() && !"()<>@,;:\\\"/[]?={}".contains(c))
}

/// Whether `value` may be a cookie's value: no control character, no `;`.
#[must_use]
pub fn is_cookie_value(value: &str) -> bool {
    !value.chars().any(|c| c.is_control() || c == ';')
}

/// The attributes of `attrs` (`domain`, `path`, `expires`, `httpOnly`, `secure`, `sameSite`,
/// `partition`) for a cookie `name`=`value`, checked.
///
/// # Errors
///
/// The name of the field that does not hold.
pub fn spec(name: &str, value: &str, attrs: &Value) -> Result<CookieSpec, String> {
    if !is_cookie_name(name) {
        return Err("name".to_owned());
    }
    if !is_cookie_value(value) || name.len() + value.len() > MAX_COOKIE_BYTES {
        return Err("value".to_owned());
    }
    let attrs = match attrs {
        Value::Null => Map::new(),
        Value::Object(map) => map.clone(),
        _ => return Err("cookie".to_owned()),
    };
    let field = |key: &str| attrs.get(key).filter(|value| !value.is_null());
    let flag = |key: &str| match field(key) {
        None => Ok(false),
        Some(Value::Bool(on)) => Ok(*on),
        Some(_) => Err(key.to_owned()),
    };
    let domain = match field("domain") {
        None => None,
        Some(Value::String(domain)) if origin::is_cookie_domain(domain) => {
            Some(domain.to_ascii_lowercase())
        }
        Some(_) => return Err("domain".to_owned()),
    };
    let path = match field("path") {
        None => "/".to_owned(),
        Some(Value::String(path))
            if path.starts_with('/')
                && path.len() <= MAX_PATH_CHARS
                && !path.chars().any(|c| c.is_control() || c == ';') =>
        {
            path.clone()
        }
        Some(_) => return Err("path".to_owned()),
    };
    let expires = match field("expires") {
        None => None,
        Some(value) => Some(
            value
                .as_f64()
                .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
                .ok_or_else(|| "expires".to_owned())?,
        ),
    };
    let same_site = match field("sameSite") {
        None => None,
        Some(Value::String(s)) if SAME_SITE.contains(&s.as_str()) => Some(s.clone()),
        Some(_) => return Err("sameSite".to_owned()),
    };
    let partition = match field("partition") {
        None => None,
        Some(Value::String(site)) if origin::is_origin(site) => Some(site.clone()),
        Some(_) => return Err("partition".to_owned()),
    };
    Ok(CookieSpec {
        name: name.to_owned(),
        value: value.to_owned(),
        domain,
        path,
        expires,
        http_only: flag("httpOnly")?,
        secure: flag("secure")?,
        same_site,
        partition,
    })
}

/// The `Network.setCookie` parameters of `spec` for a view showing `view_url`: a host-only
/// cookie goes to the URL of its host, a domain cookie names its domain.
#[must_use]
pub fn set_params(spec: &CookieSpec, view_url: &str) -> Value {
    let mut params = json!({
        "name": spec.name,
        "value": spec.value,
        "path": spec.path,
        "httpOnly": spec.http_only,
        "secure": spec.secure,
    });
    match &spec.domain {
        Some(domain) if domain.starts_with('.') => params["domain"] = json!(domain),
        host => {
            // The view's own origin when the cookie is its host's (so the port is kept), else
            // the host over https for a secure cookie, http otherwise.
            let view_host = origin::host_of(view_url);
            let host = host
                .clone()
                .or_else(|| view_host.clone())
                .unwrap_or_default();
            let view_origin = origin::origin_of(view_url)
                .filter(|_| view_host.as_deref() == Some(host.as_str()))
                .filter(|o| !spec.secure || o.starts_with("https://"));
            let base = view_origin.unwrap_or_else(|| {
                let scheme = if spec.secure { "https" } else { "http" };
                format!("{scheme}://{host}")
            });
            params["url"] = json!(format!("{base}{}", spec.path));
        }
    }
    if let Some(expires) = spec.expires {
        params["expires"] = json!(expires);
    }
    if let Some(same_site) = &spec.same_site {
        params["sameSite"] = json!(same_site);
    }
    if let Some(site) = &spec.partition {
        params["partitionKey"] = json!({ "topLevelSite": site, "hasCrossSiteAncestor": false });
    }
    params
}

/// The `Network.deleteCookies` parameters removing exactly the cookie `name` of `domain` (as
/// CDP reports it) and `path`; `None` as domain: the view's host.
#[must_use]
pub fn delete_params(
    name: &str,
    domain: Option<&str>,
    path: &str,
    partition: Option<&str>,
    view_url: &str,
) -> Value {
    let domain = domain
        .map(str::to_owned)
        .or_else(|| origin::host_of(view_url))
        .unwrap_or_default();
    let mut params = json!({ "name": name, "domain": domain, "path": path });
    if let Some(site) = partition {
        params["partitionKey"] = json!({ "topLevelSite": site, "hasCrossSiteAncestor": false });
    }
    params
}

#[cfg(test)]
#[path = "cookie_tests.rs"]
mod tests;

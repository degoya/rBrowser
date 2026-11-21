//! A session's login state as Playwright's `storageState` file: its cookies and the
//! `localStorage` of the origins its views show. The values are secrets: the `Debug` of every
//! type here counts and never shows one, and a file read back is held to bounds and shapes
//! before anything of it reaches CDP.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use rb_config::ProjectConfig;

use crate::cookies::filter::{cookie_host, domain_matches};
use crate::cookies::model::ChromeCookie;

/// The most cookies a file may carry.
pub const MAX_COOKIES: usize = 3000;
/// The most origins a file may carry.
pub const MAX_ORIGINS: usize = 200;
/// The most `localStorage` items per origin.
pub const MAX_ITEMS: usize = 2000;

/// Why a file is not taken. Never names a value.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StateError {
    /// More than the bounds allow.
    #[error("the file carries too many cookies, origins or items")]
    TooMany,
    /// Cookie number `0` (from 0) is malformed.
    #[error("cookie {0} has no name or domain, a path not starting with '/', or a bad sameSite")]
    Cookie(usize),
    /// Origin number `0` is no `http(s)` origin.
    #[error("origin {0} is no http or https origin")]
    Origin(usize),
}

/// Playwright's `storageState`.
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StorageState {
    /// The cookies.
    #[serde(default)]
    pub cookies: Vec<StateCookie>,
    /// The `localStorage` of each origin.
    #[serde(default)]
    pub origins: Vec<OriginState>,
}

/// A cookie of `storageState`.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateCookie {
    pub name: String,
    /// A secret.
    pub value: String,
    pub domain: String,
    pub path: String,
    /// Seconds since the epoch; -1 for a session cookie.
    pub expires: f64,
    pub http_only: bool,
    pub secure: bool,
    /// `Strict`, `Lax` or `None`.
    pub same_site: String,
}

/// The `localStorage` of one origin.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OriginState {
    /// `scheme://host[:port]`.
    pub origin: String,
    #[serde(default)]
    pub local_storage: Vec<NameValue>,
}

/// One `localStorage` item; the value is a secret.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct NameValue {
    pub name: String,
    pub value: String,
}

impl fmt::Debug for StorageState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StorageState")
            .field("cookies", &self.cookies.len())
            .field("origins", &self.origins.len())
            .finish()
    }
}

impl StorageState {
    /// The state of a Chromium session: `cookies` as `Network.getAllCookies` gave them (a
    /// partitioned cookie left out: `storageState` cannot name its partition), and `origins`.
    #[must_use]
    pub fn from_chrome(cookies: &[ChromeCookie], origins: Vec<OriginState>) -> Self {
        let cookies = cookies
            .iter()
            .filter(|cookie| cookie.partition_key.is_none() && !cookie.partition_key_opaque)
            .map(|cookie| StateCookie {
                name: cookie.name.clone(),
                value: cookie.value.clone(),
                domain: cookie.domain.clone(),
                path: cookie.path.clone(),
                expires: if cookie.session || !cookie.expires.is_finite() || cookie.expires <= 0.0 {
                    -1.0
                } else {
                    cookie.expires
                },
                http_only: cookie.http_only,
                secure: cookie.secure,
                same_site: match cookie.same_site.as_deref() {
                    Some("Strict") => "Strict",
                    Some("None") => "None",
                    _ => "Lax",
                }
                .to_owned(),
            })
            .collect();
        Self { cookies, origins }
    }

    /// Whether the state keeps to the bounds and shapes.
    ///
    /// # Errors
    ///
    /// The first [`StateError`].
    pub fn check(&self) -> Result<(), StateError> {
        if self.cookies.len() > MAX_COOKIES
            || self.origins.len() > MAX_ORIGINS
            || self
                .origins
                .iter()
                .any(|origin| origin.local_storage.len() > MAX_ITEMS)
        {
            return Err(StateError::TooMany);
        }
        let plain = |text: &str| !text.chars().any(char::is_control);
        for (index, cookie) in self.cookies.iter().enumerate() {
            let ok = !cookie.name.is_empty()
                && plain(&cookie.name)
                && plain(&cookie.value)
                && !cookie.domain.trim_start_matches('.').is_empty()
                && plain(&cookie.domain)
                && cookie.path.starts_with('/')
                && plain(&cookie.path)
                && cookie.expires.is_finite()
                && matches!(cookie.same_site.as_str(), "Strict" | "Lax" | "None");
            if !ok {
                return Err(StateError::Cookie(index));
            }
        }
        for (index, origin) in self.origins.iter().enumerate() {
            if !is_origin(&origin.origin) {
                return Err(StateError::Origin(index));
            }
        }
        Ok(())
    }

    /// The parameters of CDP's `Network.setCookies` for every cookie.
    #[must_use]
    pub fn set_cookies_params(&self) -> Value {
        let cookies: Vec<Value> = self
            .cookies
            .iter()
            .map(|cookie| {
                let mut param = json!({
                    "name": cookie.name,
                    "value": cookie.value,
                    "domain": cookie.domain,
                    "path": cookie.path,
                    "secure": cookie.secure,
                    "httpOnly": cookie.http_only,
                    "sameSite": cookie.same_site,
                });
                if cookie.expires > 0.0 {
                    param["expires"] = json!(cookie.expires);
                }
                param
            })
            .collect();
        json!({ "cookies": cookies })
    }
}

impl StorageState {
    /// Keeps the cookies and origins of `hosts` (and their subdomains, and the parent domains
    /// their cookies are set for); the rest of the session's jar stays out of the file
    /// (security review w28).
    pub fn keep_hosts(&mut self, hosts: &[String]) {
        let related = |site: &str| {
            hosts
                .iter()
                .any(|host| domain_matches(host, site) || domain_matches(site, host))
        };
        self.cookies.retain(|cookie| {
            let domain = cookie.domain.trim_start_matches('.').to_ascii_lowercase();
            related(&domain)
        });
        self.origins
            .retain(|origin| cookie_host(&origin.origin).is_some_and(|host| related(&host)));
    }
}

/// The hosts an export keeps: the environments' and the start URL's, sorted; without either,
/// those of `shown` (the origins the session's views show).
#[must_use]
pub fn scope_hosts(config: &ProjectConfig, shown: &[String]) -> Vec<String> {
    let configured = config
        .environments
        .iter()
        .map(|environment| environment.url.as_str())
        .chain(config.start_url.as_deref());
    let mut hosts: Vec<String> = configured.filter_map(cookie_host).collect();
    if hosts.is_empty() {
        hosts = shown
            .iter()
            .filter_map(|origin| cookie_host(origin))
            .collect();
    }
    hosts.sort();
    hosts.dedup();
    hosts
}

/// Whether `text` is an `http(s)` origin exactly as `location.origin` writes it: lower case, no
/// user info, no path, no default port.
pub fn is_origin(text: &str) -> bool {
    rb_util::url::origin_of(text).as_deref() == Some(text)
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;

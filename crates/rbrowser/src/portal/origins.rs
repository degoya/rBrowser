//! Which origins the portal forwards to (RB-080-05): only the project's own - the origins of
//! `environments[].url` and of `startUrl` - and of those only `http`. The device always sees the
//! origin of the grid's active view: the portal is no open proxy, and a page outside the
//! project's origins is refused. Headers that name the upstream or the portal are rewritten
//! with [`rewrite_url`]. Beside it, an environment's `assetOrigins` (RB-090-65): further `http`
//! origins its page loads assets from, each forwarded on a listener of its own
//! ([`super::assets`]) while that environment is shown.

use rb_sync::nav_url::origin_of;

/// Why a request has no upstream; each answers 403.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The active view shows no page.
    NoPage,
    /// The active view's origin is none of the project's.
    NotAllowed,
    /// The origin is the project's but `https`, which the portal does not forward (phase 1).
    Https,
}

impl Refusal {
    /// The i18n key of the page the device sees.
    #[must_use]
    pub fn message_key(self) -> &'static str {
        match self {
            Self::NoPage => "portal.page.no_page",
            Self::NotAllowed => "portal.page.not_allowed",
            Self::Https => "portal.page.https",
        }
    }
}

/// An `http` origin the portal forwards to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Upstream {
    origin: String,
    host: String,
    port: u16,
}

/// Whether `url` carries credentials in its authority, as written: `origin_of` would drop
/// them silently, and such an entry is never a device's upstream.
fn has_credentials(url: &str) -> bool {
    url.split_once("://")
        .and_then(|(_, rest)| rest.split(['/', '?', '#']).next())
        .is_some_and(|raw| raw.contains('@'))
}

impl Upstream {
    /// The upstream of an `http` URL or origin.
    #[must_use]
    pub fn parse(url: &str) -> Option<Self> {
        if has_credentials(url) {
            return None;
        }
        let origin = origin_of(url)?;
        let authority = origin.strip_prefix("http://")?;
        let (host, port) = match authority.find(']') {
            Some(end) => match authority[end + 1..].strip_prefix(':') {
                Some(port) => (&authority[..=end], port.parse().ok()?),
                None => (&authority[..=end], 80),
            },
            None => match authority.rsplit_once(':') {
                Some((host, port)) => (host, port.parse().ok()?),
                None => (authority, 80),
            },
        };
        let host = host.to_owned();
        (!host.is_empty()).then_some(Self { origin, host, port })
    }

    /// `http://host[:port]`.
    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// `host[:port]`, the `Host` header it expects.
    #[must_use]
    pub fn authority(&self) -> &str {
        self.origin.strip_prefix("http://").unwrap_or(&self.origin)
    }

    /// The host to connect to (brackets kept for IPv6).
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The port to connect to.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }
}

/// The project's origins: `start_url`'s and each environment's, canonical, once each, in order.
pub fn allowlist<'a>(
    start_url: Option<&'a str>,
    environments: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    let mut origins: Vec<String> = Vec::new();
    for url in start_url.into_iter().chain(environments) {
        if !has_credentials(url)
            && let Some(origin) = origin_of(url)
            && !origins.contains(&origin)
        {
            origins.push(origin);
        }
    }
    origins
}

/// Per environment origin, the asset origins the portal forwards while it is shown
/// (`environments[].assetOrigins`, RB-090-65).
pub type AssetOrigins = Vec<(String, Vec<String>)>;

/// The asset origins of `environments` (each its URL and its `assetOrigins`): per environment
/// origin the canonical `http` origins, once each, in order. An entry with credentials, a path,
/// another scheme or the environment's own origin is left out; environments of one origin share
/// their lists.
pub fn asset_origins<'a>(
    environments: impl IntoIterator<Item = (&'a str, &'a [String])>,
) -> AssetOrigins {
    let mut out: AssetOrigins = Vec::new();
    for (url, assets) in environments {
        if has_credentials(url) {
            continue;
        }
        let Some(origin) = origin_of(url) else {
            continue;
        };
        let at = match out.iter().position(|(known, _)| *known == origin) {
            Some(at) => at,
            None => {
                out.push((origin.clone(), Vec::new()));
                out.len() - 1
            }
        };
        for asset in assets {
            if !rb_config::is_http_origin(asset) || has_credentials(asset) {
                continue;
            }
            let Some(upstream) = Upstream::parse(asset) else {
                continue;
            };
            let list = &mut out[at].1;
            if upstream.origin() != origin && !list.iter().any(|known| known == upstream.origin()) {
                list.push(upstream.origin().to_owned());
            }
        }
    }
    out.retain(|(_, assets)| !assets.is_empty());
    out
}

/// Every asset origin of `assets`, once each, in order: one listener each.
#[must_use]
pub fn listened(assets: &AssetOrigins) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for origin in assets.iter().flat_map(|(_, list)| list) {
        if !out.contains(origin) {
            out.push(origin.clone());
        }
    }
    out
}

/// The asset origins of the environment whose origin is `upstream`; none for another.
#[must_use]
pub fn assets_of<'a>(assets: &'a AssetOrigins, upstream: &str) -> &'a [String] {
    assets
        .iter()
        .find(|(origin, _)| origin == upstream)
        .map(|(_, list)| list.as_slice())
        .unwrap_or_default()
}

/// The upstream for a device while the active view shows `current`.
///
/// # Errors
///
/// [`Refusal`] when there is no page, its origin is not the project's, or it is `https`.
pub fn upstream_for(current: Option<&str>, allowlist: &[String]) -> Result<Upstream, Refusal> {
    let origin = current.and_then(origin_of).ok_or(Refusal::NoPage)?;
    if !allowlist.contains(&origin) {
        return Err(Refusal::NotAllowed);
    }
    if origin.starts_with("https://") {
        return Err(Refusal::Https);
    }
    Upstream::parse(&origin).ok_or(Refusal::NotAllowed)
}

/// Path, query and hash of `url` (`/` for none): where a device goes to follow it.
#[must_use]
pub fn device_path(url: &str) -> Option<String> {
    origin_of(url)?;
    let url = url.trim();
    let authority = url.find("://")? + "://".len();
    let rest = url[authority..]
        .find(['/', '?', '#'])
        .map_or("", |offset| &url[authority + offset..]);
    Some(if !rest.starts_with('/') {
        format!("/{rest}")
    } else {
        rest.to_owned()
    })
}

/// Whether the `Host` header names the portal itself (`authority`, `ip:port`): a page that a
/// name in DNS points at the portal (DNS rebinding) is refused.
#[must_use]
pub fn host_matches(host: Option<&str>, authority: &str) -> bool {
    host.is_some_and(|host| host.trim().eq_ignore_ascii_case(authority))
}

/// `url` with the origin `from` replaced by `to`; any other URL unchanged.
#[must_use]
pub fn rewrite_url(url: &str, from: &str, to: &str) -> String {
    if origin_of(url).as_deref() != Some(from) {
        return url.to_owned();
    }
    match device_path(url) {
        Some(path) => format!("{to}{path}"),
        None => url.to_owned(),
    }
}

#[cfg(test)]
#[path = "origins_tests.rs"]
mod tests;

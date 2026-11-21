//! One map from upstream origins to portal origins (RB-090-65): the page's own (the active
//! view's origin and the portal's main listener) and one pair per asset origin of the shown
//! environment (`environments[].assetOrigins`, each with a listener of its own). Every rewrite
//! uses it - bodies (HTML, CSS, JavaScript, JSON; plain and escaped `http:\/\/` URLs, RB-090-69,
//! and HTML character references, `http&#x3A;&#x2F;&#x2F;`, RB-090-75 - [`super::url_forms`];
//! read whole or streamed through [`Needles`], RB-090-72),
//! `Location`, `Origin`, `Referer` and `Access-Control-Allow-Origin` - so a page that loads its
//! styles and scripts from a Vite dev server beside a PHP app reaches both through the portal.
//! Pure: no socket, no lock.

use std::sync::Mutex;

use rb_sync::nav_url::origin_of;
use rb_util::url::host_of;

use super::offer::offer;
use super::origins::rewrite_url;
use super::url_forms::{
    Form, LONGEST_REFERENCE, SCHEME, authority_at, is_whole, partial_reference, scheme_form_at,
    upper_hex,
};
use crate::cdp::lock;

/// Upstream origins and the portal origins that stand for them; the first pair is the page's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginMap {
    pairs: Vec<(String, String)>,
    /// The presentation's traffic over the internet (RB-096-05, [`super::present::headers`]).
    public: bool,
}

impl OriginMap {
    /// The page's pair: the upstream `upstream` is `portal` on the device.
    #[must_use]
    pub fn new(upstream: &str, portal: &str) -> Self {
        Self {
            pairs: vec![(upstream.to_owned(), portal.to_owned())],
            public: false,
        }
    }

    /// The same map for the presentation's traffic over the internet (RB-096-05): no cookie
    /// either way, no Cloudflare header to the upstream, no server detail to the viewer.
    #[must_use]
    pub fn public(mut self) -> Self {
        self.public = true;
        self
    }

    /// Whether it serves the presentation's traffic.
    #[must_use]
    pub fn is_public(&self) -> bool {
        self.public
    }

    /// Adds the asset origin `upstream`, served on the device as `portal`; a pair whose either
    /// side is mapped already is left out.
    #[must_use]
    pub fn with(mut self, upstream: &str, portal: &str) -> Self {
        let known = self
            .pairs
            .iter()
            .any(|(from, to)| from == upstream || to == portal);
        if !known {
            self.pairs.push((upstream.to_owned(), portal.to_owned()));
        }
        self
    }

    /// Whether it maps asset origins beside the page's.
    #[must_use]
    pub fn has_assets(&self) -> bool {
        self.pairs.len() > 1
    }

    /// The page's upstream origin.
    #[must_use]
    pub fn main_upstream(&self) -> &str {
        &self.pairs[0].0
    }

    /// The upstream origins, the page's first.
    pub fn upstreams(&self) -> impl Iterator<Item = &str> {
        self.pairs.iter().map(|(upstream, _)| upstream.as_str())
    }

    /// The portal origins, the page's first.
    pub fn portals(&self) -> impl Iterator<Item = &str> {
        self.pairs.iter().map(|(_, portal)| portal.as_str())
    }

    /// `url` with a portal origin replaced by its upstream origin (a device's `Referer`).
    #[must_use]
    pub fn to_upstream_url(&self, url: &str) -> String {
        self.convert(url, true, true)
    }

    /// `url` with an upstream origin replaced by its portal origin (`Location`).
    #[must_use]
    pub fn to_portal_url(&self, url: &str) -> String {
        self.convert(url, false, true)
    }

    /// An `Origin` header for the upstream: a portal origin becomes its upstream origin. A
    /// module script on an asset listener carries the page's portal origin, which becomes the
    /// page's upstream origin - the one a dev server's CORS rule admits.
    #[must_use]
    pub fn origin_to_upstream(&self, value: &str) -> String {
        self.convert(value, true, false)
    }

    /// An `Access-Control-Allow-Origin` for the device: an upstream origin becomes its portal
    /// origin; `*` and anything else stay.
    #[must_use]
    pub fn origin_to_portal(&self, value: &str) -> String {
        self.convert(value, false, false)
    }

    /// `value` with the origin of one side of a pair replaced by the other side's: towards the
    /// upstream or towards the portal; the path kept for a URL, dropped for an origin header.
    fn convert(&self, value: &str, to_upstream: bool, keep_path: bool) -> String {
        let Some(origin) = origin_of(value) else {
            return value.to_owned();
        };
        for (upstream, portal) in &self.pairs {
            let (from, to) = if to_upstream {
                (portal, upstream)
            } else {
                (upstream, portal)
            };
            if *from == origin {
                return if keep_path {
                    rewrite_url(value, from, to)
                } else {
                    to.clone()
                };
            }
        }
        value.to_owned()
    }

    /// `body` (HTML, CSS, JavaScript or JSON) with every absolute URL of an upstream origin
    /// pointed at its portal origin, in one pass, plain (`http://host:port`), with escaped
    /// slashes (`http:\/\/host:port`, JSON's form) or with its punctuation as character
    /// references (`http&#x3A;&#x2F;&#x2F;host&#x3A;port`, hex upper or lower, or decimal - an
    /// attribute escaper's `src` and `srcset`, RB-090-75), each written back in its own form. An
    /// origin matches in any case and only whole (`:3000`, not `:30001` or
    /// `localhost:3000.example`); a `https` twin stays.
    #[must_use]
    pub fn rewrite_body(&self, body: &[u8]) -> Vec<u8> {
        self.needles().rewrite(body, body.len()).0
    }

    /// What [`Self::rewrite_body`] looks for, built once for a body that streams (RB-090-72).
    #[must_use]
    pub fn needles(&self) -> Needles {
        // Hex upper and lower read alike in lower case: one needle, the case of the portal
        // origin taken from the body.
        let forms = [
            Form::Plain,
            Form::Escaped,
            Form::Hex { upper: false },
            Form::Decimal,
        ];
        let mut needles: Vec<Needle> = self
            .pairs
            .iter()
            .flat_map(|(upstream, portal)| {
                let upstream = upstream.to_ascii_lowercase();
                forms.map(|form| Needle {
                    upstream: form.encode(&upstream),
                    portal: form.encode(portal),
                    portal_upper: matches!(form, Form::Hex { .. })
                        .then(|| Form::Hex { upper: true }.encode(portal)),
                })
            })
            .collect();
        // The longest first: `http://a:80` never shadows `http://a:8000`.
        needles.sort_by_key(|needle| std::cmp::Reverse(needle.upstream.len()));
        Needles { needles }
    }
}

/// One upstream origin in one form, in lower case, and the portal origin in the same form.
#[derive(Clone, Debug)]
struct Needle {
    upstream: Vec<u8>,
    portal: Vec<u8>,
    /// For hex references: the portal origin in upper case, for a body that writes them so.
    portal_upper: Option<Vec<u8>>,
}

/// Each upstream origin in each form ([`Form`]), with the portal origin that replaces it; the
/// longest first.
#[derive(Clone, Debug)]
pub struct Needles {
    needles: Vec<Needle>,
}

impl Needles {
    /// How far before the end of what came an origin may start and still be undecided: the
    /// longest upstream origin in any form, and a character reference after it that may still
    /// make it go on (`&#x3a;`).
    #[must_use]
    pub fn reach(&self) -> usize {
        self.needles
            .first()
            .map_or(0, |needle| needle.upstream.len() + LONGEST_REFERENCE)
    }

    /// `body` with every upstream origin that starts before `cut` replaced, and the length of
    /// `body` the output stands for (`cut`, or the end of an origin that runs past it). The
    /// caller makes sure that the byte (or character reference) after each such origin is in
    /// `body` or that `body` ends there; with `cut` at the end of `body`, the whole body.
    #[must_use]
    pub fn rewrite(&self, body: &[u8], cut: usize) -> (Vec<u8>, usize) {
        let lower = body.to_ascii_lowercase();
        let mut out = Vec::with_capacity(body.len());
        let mut copied = 0;
        let mut at = 0;
        while let Some(found) = find(&lower[at..], SCHEME) {
            let start = at + found;
            if start >= cut {
                break;
            }
            let replaced = self.needles.iter().find(|needle| {
                lower[start..].starts_with(&needle.upstream)
                    && is_whole(&lower, start + needle.upstream.len())
            });
            match replaced {
                Some(needle) => {
                    let end = start + needle.upstream.len();
                    out.extend_from_slice(&body[copied..start]);
                    let portal = match &needle.portal_upper {
                        Some(upper) if upper_hex(&body[start..end]) => upper,
                        _ => &needle.portal,
                    };
                    out.extend_from_slice(portal);
                    copied = end;
                    at = copied;
                }
                None => at = start + SCHEME.len(),
            }
        }
        let consumed = copied.max(cut);
        out.extend_from_slice(&body[copied..consumed]);
        (out, consumed)
    }

    /// Whether `rest` (lower case, at the end of what came so far) may still turn out to be an
    /// upstream origin: it is the start of one, or one whole that needs the next byte - or the
    /// rest of a character reference - to tell.
    #[must_use]
    pub fn could_start(&self, rest: &[u8]) -> bool {
        !rest.is_empty()
            && (SCHEME.starts_with(rest)
                || self.needles.iter().any(|needle| {
                    needle.upstream.starts_with(rest)
                        || rest
                            .strip_prefix(needle.upstream.as_slice())
                            .is_some_and(partial_reference)
                }))
    }
}

pub(super) fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// The most loopback origins one page reports.
const MAX_REPORTED: usize = 16;
/// The most loopback origins the log names in one run of the app.
const MAX_LOGGED: usize = 64;

/// The loopback `http` origins (`localhost`, `127.0.0.1`, `[::1]`) that `body` names, plain,
/// with escaped slashes or as character references (RB-090-75), and `map` does not map, once
/// each, in order: what a device cannot reach. Origins only, in their plain form - never a path
/// or a query.
#[must_use]
pub fn unmapped_loopback(body: &[u8], map: &OriginMap) -> Vec<String> {
    body_origins(body, map, body.len(), true).unmapped
}

/// What the body scan finds, origins only, in their plain form, each once, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BodyOrigins {
    /// Loopback origins the map does not map ([`unmapped_loopback`]).
    pub unmapped: Vec<String>,
    /// Mapped origins written in a form no needle matches - a padded character reference
    /// (`&#x003A;`), a reference in a plain URL, a default port written out (`:80`): they reach
    /// the device unrewritten (RB-090-75 review).
    pub unrewritten: Vec<String>,
}

/// What `body` names in the URLs that start before `cut`; unless `at_end`, one whose host runs
/// to the end of `body` is left out (the rest of it is still to come).
#[must_use]
pub fn body_origins(body: &[u8], map: &OriginMap, cut: usize, at_end: bool) -> BodyOrigins {
    let lower = body.to_ascii_lowercase();
    let mut found = BodyOrigins::default();
    let mut at = 0;
    while let Some(offset) = find(&lower[at..], SCHEME) {
        let start = at + offset;
        if start >= cut {
            break;
        }
        let Some((form, scheme)) = scheme_form_at(&lower, start) else {
            at = start + SCHEME.len();
            continue;
        };
        let authority = authority_at(&lower, start + scheme);
        at = authority.end.max(start + 1);
        if authority.open && !at_end {
            break;
        }
        let Some(origin) = origin_of(&format!("http://{}", authority.text)) else {
            continue;
        };
        if map.upstreams().any(|known| known == origin) {
            // A needle matches only this exact writing, ending whole.
            let rewritten = lower[start..authority.end] == form.encode(&origin)[..]
                && is_whole(&lower, authority.end);
            if !rewritten
                && found.unrewritten.len() < MAX_REPORTED
                && !found.unrewritten.contains(&origin)
            {
                found.unrewritten.push(origin);
            }
            continue;
        }
        let loopback = host_of(&origin)
            .is_some_and(|host| matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]"));
        if loopback && found.unmapped.len() < MAX_REPORTED && !found.unmapped.contains(&origin) {
            found.unmapped.push(origin);
        }
        if found.unmapped.len() >= MAX_REPORTED && found.unrewritten.len() >= MAX_REPORTED {
            break;
        }
    }
    found
}

/// The origins the body scan logged since the portal started.
static LOGGED: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// The origins logged from the devices' misses since the portal started (RB-090-72): a record of
/// their own, so a device cannot fill the body scan's.
static DEVICE_LOGGED: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// The most origins the log names from the devices' misses in one run of the portal.
pub const MAX_DEVICE_LOGGED: usize = 64;

/// Forgets what was logged: called when the portal starts, so each start logs again.
pub fn forget_logged() {
    lock(&LOGGED).clear();
    lock(&DEVICE_LOGGED).clear();
}

/// Logs, once per origin and start of the portal, each loopback origin a body served through
/// the portal names (a page, a style sheet, a script or JSON) that the portal does not forward:
/// the evidence for an `assetOrigins` entry (the origin only). Each one goes to the offer too,
/// with the page's origin, outside the lock (RB-090-71, [`offer`], which keeps its own record
/// of what it offered).
pub fn log_unmapped_loopback(body: &[u8], map: &OriginMap) {
    let found = body_origins(body, map, body.len(), true);
    report_unmapped(&found.unmapped, map);
    report_unrewritten(&found.unrewritten);
}

/// Logs, once per origin and start, each of `found`, mapped origins a body names in a form the
/// rewrite does not match (the origin only): a gap of the rewrite, never offered.
pub fn report_unrewritten(found: &[String]) {
    for origin in found {
        if first_logged(origin) {
            tracing::info!(
                %origin,
                "a portal page names a forwarded origin in a form the portal does not rewrite (a \
                 padded character reference, a default port written out); it reaches the device \
                 unrewritten"
            );
        }
    }
}

/// Logs (once per origin and start) and offers each of `found`, unmapped loopback origins a body
/// of the page `map` stands for names.
pub fn report_unmapped(found: &[String], map: &OriginMap) {
    for origin in found {
        if first_logged(origin) {
            tracing::info!(
                %origin,
                "a portal page names a loopback origin the portal does not forward; list it in \
                 the environment's assetOrigins"
            );
        }
    }
    for origin in found {
        offer(map, origin);
    }
}

/// Whether `origin`, a device's miss, is to be logged now: neither the body scan nor a device
/// logged it since the portal started, and the devices' record is not full; it is recorded there.
pub(super) fn first_logged_by_device(origin: &str) -> bool {
    if lock(&LOGGED).iter().any(|known| known == origin) {
        return false;
    }
    let mut logged = lock(&DEVICE_LOGGED);
    if logged.len() >= MAX_DEVICE_LOGGED || logged.iter().any(|known| known == origin) {
        return false;
    }
    logged.push(origin.to_owned());
    true
}

/// Whether `origin`, named by a body, is to be logged now: not logged since the portal started,
/// and the body scan's record not full; it is recorded.
pub(super) fn first_logged(origin: &str) -> bool {
    let mut logged = lock(&LOGGED);
    if logged.len() >= MAX_LOGGED || logged.iter().any(|known| known == origin) {
        return false;
    }
    logged.push(origin.to_owned());
    true
}

#[cfg(test)]
#[path = "origin_map_tests.rs"]
mod tests;

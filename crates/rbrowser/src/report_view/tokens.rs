//! The report window's addresses (RB-091-02): `rbrowser://report/<token>/<file>`, a random
//! 128-bit token per report asked for, which names the report's project, plugin and path. No
//! path is in the address, so a page cannot name a file of its own choosing: the route serves
//! only what was asked for through [`super::open`] and passed [`super::guard`] (again on every
//! load). The scheme's host differs from the shell's (`rbrowser://ui/`), so the report is another
//! origin than the shell's pages and gets no `cefQuery` (`cef_app/render.rs`); outer CDP clients
//! never see a `rbrowser:` target (`rb_cdp::target_filter`).
//!
//! [`CSP`] is the report's policy: its own inline script and style and `data:` images, nothing
//! from the network, no frame, worker, form or `<base>`.

use std::collections::VecDeque;
use std::path::PathBuf;

/// The route's host.
pub const ROUTE_HOST: &str = "report";
/// Every report address starts with this.
pub const REPORT_ORIGIN: &str = "rbrowser://report/";
/// The hex digits of a token.
pub const TOKEN_HEX: usize = 32;
/// The most reports addressable at once; the oldest address goes first.
pub const MAX_TOKENS: usize = 64;
/// The report's Content-Security-Policy.
pub const CSP: &str = "default-src 'none'; script-src 'unsafe-inline'; \
    style-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src data:; \
    connect-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; \
    object-src 'none'; manifest-src 'none'; base-uri 'none'; form-action 'none'; \
    frame-ancestors 'none'";

/// What a token names: the report `rel` below `.rbrowser/out/<plugin>/` of the project at `root`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub root: PathBuf,
    pub plugin: String,
    pub rel: String,
}

impl Entry {
    /// The report's name for the window title: its path below the plugin's folder.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.rel
    }

    /// The file name, the address's last part.
    fn file(&self) -> &str {
        self.rel.rsplit(['/', '\\']).next().unwrap_or("report.html")
    }
}

/// The live tokens, oldest first.
#[derive(Debug, Default)]
pub struct Tokens {
    live: VecDeque<(String, Entry)>,
}

/// A new random token.
fn token() -> String {
    let mut bytes = [0u8; TOKEN_HEX / 2];
    if getrandom::fill(&mut bytes).is_err() {
        // No randomness: an address no one can guess cannot be made, so none that works.
        tracing::warn!("no random token for a report address");
        return String::new();
    }
    rb_util::hex(&bytes)
}

impl Tokens {
    /// No token yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            live: VecDeque::new(),
        }
    }

    /// An address for `entry`.
    pub fn issue(&mut self, entry: Entry) -> String {
        let token = token();
        let url = format!("{REPORT_ORIGIN}{token}/{}", entry.file());
        if token.len() == TOKEN_HEX {
            if self.live.len() >= MAX_TOKENS {
                self.live.pop_front();
            }
            self.live.push_back((token, entry));
        }
        url
    }

    /// What `url` names, when its token is live.
    #[must_use]
    pub fn resolve(&self, url: &str) -> Option<&Entry> {
        let (token, file) = parse_url(url)?;
        self.live
            .iter()
            .find(|(live, entry)| *live == token && entry.file() == file)
            .map(|(_, entry)| entry)
    }

    /// `url`'s token no longer resolves.
    pub fn revoke(&mut self, url: &str) {
        if let Some((token, _)) = parse_url(url) {
            self.live.retain(|(live, _)| *live != token);
        }
    }

    /// How many tokens are live.
    #[must_use]
    pub fn len(&self) -> usize {
        self.live.len()
    }

    /// Whether none is.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }
}

/// Whether `url` is a report address (the scheme and host in any case).
#[must_use]
pub fn is_report_url(url: &str) -> bool {
    url.get(..REPORT_ORIGIN.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(REPORT_ORIGIN))
}

/// The token and file name of a report address; a fragment is dropped, a query refuses it.
#[must_use]
pub fn parse_url(url: &str) -> Option<(String, String)> {
    if !is_report_url(url) {
        return None;
    }
    let rest = &url[REPORT_ORIGIN.len()..];
    let rest = rest.split('#').next().unwrap_or(rest);
    if rest.contains('?') {
        return None;
    }
    let (token, file) = rest.split_once('/')?;
    let hex = token.len() == TOKEN_HEX && token.bytes().all(|b| b.is_ascii_hexdigit());
    let plain = !file.is_empty()
        && file
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && !file.starts_with('.');
    (hex && plain).then(|| (token.to_ascii_lowercase(), file.to_owned()))
}

#[cfg(test)]
#[path = "tokens_tests.rs"]
mod tests;

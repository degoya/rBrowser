//! How often a stored credential is sent: once per view (CEF browser), origin and realm while the
//! server keeps asking. Chromium keeps an answered credential in the session's auth cache under
//! its origin and realm and sends it on by itself, so a second realm of the origin is a first
//! challenge of its own (RB-094-10). The same challenge again within
//! [`REFUSAL_WINDOW`](super::prompt::REFUSAL_WINDOW) of the answer means the server refused the
//! credential, so it is not sent in a loop - the person is asked instead (`basic_auth::ask`). A
//! challenge later than that is a fresh one: the answer before was accepted (review w35). Setting
//! or removing a credential starts over, and `environment.auth` reloads the views.

use std::collections::HashMap;
use std::time::Instant;

use super::origin::AuthOrigin;
use super::prompt::REFUSAL_WINDOW;

/// The answers given since the credentials last changed, and when.
#[derive(Debug, Default)]
pub struct Attempts {
    answered: HashMap<(i32, AuthOrigin, String), Instant>,
}

impl Attempts {
    /// Whether the challenge of `origin` and `realm` in `browser` may be answered now; records
    /// the answer.
    pub fn take(&mut self, browser: i32, origin: &AuthOrigin, realm: &str) -> bool {
        self.take_at(browser, origin, realm, Instant::now())
    }

    /// Whether the challenge of `origin` and `realm` in `browser` may be answered at `now`: not
    /// when it was answered within [`REFUSAL_WINDOW`] - the server refused it. Records the
    /// answer.
    pub fn take_at(
        &mut self,
        browser: i32,
        origin: &AuthOrigin,
        realm: &str,
        now: Instant,
    ) -> bool {
        self.answered
            .retain(|_, at| now.saturating_duration_since(*at) <= REFUSAL_WINDOW);
        let key = (browser, origin.clone(), realm.to_owned());
        if self.answered.contains_key(&key) {
            return false;
        }
        self.answered.insert(key, now);
        true
    }

    /// A credential was set or removed: every view may try once more.
    pub fn reset(&mut self) {
        self.answered.clear();
    }
}

#[cfg(test)]
#[path = "attempts_tests.rs"]
mod tests;

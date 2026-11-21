//! How often a stored credential is sent: once per view (CEF browser), origin and realm.
//! Chromium keeps an answered credential in the session's auth cache under its origin and realm
//! and sends it on by itself, so a second realm of the origin is a first challenge of its own
//! (RB-094-10). Asking again for one realm means the server refused the credential, so that
//! challenge falls back to the person's prompt instead of sending a wrong password in a loop.
//! Setting or removing a credential starts over.

use std::collections::HashSet;

use super::origin::AuthOrigin;

/// The answers given since the credentials last changed.
#[derive(Debug, Default)]
pub struct Attempts {
    answered: HashSet<(i32, AuthOrigin, String)>,
}

impl Attempts {
    /// Whether the challenge of `origin` and `realm` in `browser` may be answered; records the
    /// answer.
    pub fn take(&mut self, browser: i32, origin: &AuthOrigin, realm: &str) -> bool {
        self.answered
            .insert((browser, origin.clone(), realm.to_owned()))
    }

    /// A credential was set or removed: every view may try once more.
    pub fn reset(&mut self) {
        self.answered.clear();
    }
}

#[cfg(test)]
#[path = "attempts_tests.rs"]
mod tests;

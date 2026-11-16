//! Cookies copied into Firefox (RB-040-05): [`FirefoxEngine::set_cookies`] sets them in the
//! jar of a session - the default user context for `default`, a named session's own user
//! context otherwise ([`super::session`]) - one `storage.setCookie` each. Values travel only in
//! the commands; a refusal is counted and logged by its BiDi error code alone, never with
//! Firefox's message (which could quote the cookie).

use crate::error::BidiError;
use crate::storage::{Cookie, set_cookie_in};

use super::FirefoxEngine;

/// How many cookies Firefox took and refused.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CookieOutcome {
    /// Set.
    pub set: usize,
    /// Refused by Firefox (an invalid domain, a prefix rule).
    pub refused: usize,
}

impl FirefoxEngine {
    /// Sets `cookies` in the jar of `session` of the running Firefox. Never starts Firefox:
    /// [`BidiError::Closed`] when none runs (its views start with what the next copy sets).
    pub async fn set_cookies(
        &self,
        session: &str,
        cookies: &[Cookie],
    ) -> Result<CookieOutcome, BidiError> {
        let browser = self.running_browser().await.ok_or(BidiError::Closed)?;
        let user_context = browser.user_context(session).await?;
        let mut outcome = CookieOutcome::default();
        for cookie in cookies {
            match browser
                .client
                .send(set_cookie_in(cookie, user_context.as_deref()))
                .await
            {
                Ok(_) => outcome.set += 1,
                Err(BidiError::Closed) => return Err(BidiError::Closed),
                Err(error) => {
                    outcome.refused += 1;
                    tracing::debug!(code = %code_of(&error), "Firefox refused a cookie");
                }
            }
        }
        Ok(outcome)
    }
}

/// The BiDi error code of `error`, or its kind: never a message Firefox wrote.
fn code_of(error: &BidiError) -> &str {
    match error {
        BidiError::Command { error, .. } => error,
        BidiError::Timeout { .. } => "timeout",
        BidiError::Protocol { .. } => "protocol",
        _ => "other",
    }
}

#[cfg(test)]
#[path = "cookies_tests.rs"]
mod tests;

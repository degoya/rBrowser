//! The viewer link of a presentation (RB-096-05): one at a time, 128 bits from the OS's random
//! source, valid for `portal.presentation.linkTtlSec` and spent by its first viewer. The token
//! travels in the link's fragment (`#t=...`), which a browser never sends: neither Cloudflare's
//! logs nor the dev server's see it; the join page posts it to the portal.
//!
//! Behind the tunnel every request comes from 127.0.0.1, so failures are counted per link and
//! across links instead of per address: [`MAX_LINK_FAILURES`] wrong tokens within
//! [`FAILURE_WINDOW`] spend the link, [`MAX_GLOBAL_FAILURES`] lock every attempt for the rest of
//! the window. Every comparison is constant-time; the `Debug` output shows no token.

use std::collections::VecDeque;
use std::fmt;
use std::time::{Duration, Instant};

use crate::portal::pairing::Secret;
pub use crate::portal::pairing::{FAILURE_WINDOW, PairError};

/// The link token's length in bytes (128 bits).
pub const LINK_BYTES: usize = 16;
/// Wrong tokens within [`FAILURE_WINDOW`] that spend the open link.
pub const MAX_LINK_FAILURES: usize = 5;
/// Wrong tokens within [`FAILURE_WINDOW`], across links, that refuse every attempt.
pub const MAX_GLOBAL_FAILURES: usize = 20;
/// The path of the join page the link opens.
pub const JOIN_PATH: &str = "/__rbrowser/join";

struct Open {
    token: Secret,
    expires: Instant,
    failures: VecDeque<Instant>,
}

/// The open link and the failures of a presentation.
pub struct ViewerLinks {
    open: Option<Open>,
    failures: VecDeque<Instant>,
    /// Every token issued, for the log mask until the presentation ends.
    issued: Vec<Secret>,
    ttl: Duration,
}

impl fmt::Debug for ViewerLinks {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ViewerLinks")
            .field("open", &self.open.is_some())
            .field("failures", &self.failures.len())
            .finish_non_exhaustive()
    }
}

impl ViewerLinks {
    /// No link; links live `ttl`.
    #[must_use]
    pub fn new(ttl: Duration) -> Self {
        Self {
            open: None,
            failures: VecDeque::new(),
            issued: Vec::new(),
            ttl,
        }
    }

    /// A fresh link token valid from `now`; the one before is void.
    ///
    /// # Errors
    ///
    /// The OS gives no randomness.
    pub fn issue(&mut self, now: Instant) -> std::io::Result<Secret> {
        let token = Secret::generate(LINK_BYTES)?;
        self.open = Some(Open {
            token: token.clone(),
            expires: now + self.ttl,
            failures: VecDeque::new(),
        });
        self.issued.push(token.clone());
        Ok(token)
    }

    /// Whether `offered` is the open link's token at `now`; the caller spends it once the viewer
    /// is admitted ([`Self::spend`]).
    ///
    /// # Errors
    ///
    /// [`PairError::RateLimited`] past the failures across links, [`PairError::Invalid`] for no
    /// link or another token (counted; the fifth spends the link), [`PairError::Expired`] (the
    /// link is spent).
    pub fn check(&mut self, offered: &str, now: Instant) -> Result<(), PairError> {
        forget_old(&mut self.failures, now);
        if self.failures.len() >= MAX_GLOBAL_FAILURES {
            return Err(PairError::RateLimited);
        }
        let Some(open) = self.open.as_mut() else {
            self.failures.push_back(now);
            return Err(PairError::Invalid);
        };
        if !open.token.matches(offered) {
            self.failures.push_back(now);
            forget_old(&mut open.failures, now);
            open.failures.push_back(now);
            if open.failures.len() >= MAX_LINK_FAILURES {
                tracing::info!("a presentation link was spent by failed attempts");
                self.open = None;
            }
            return Err(PairError::Invalid);
        }
        if now >= open.expires {
            self.open = None;
            return Err(PairError::Expired);
        }
        Ok(())
    }

    /// The open link is used: it works no more.
    pub fn spend(&mut self) {
        self.open = None;
    }

    /// Whether a link is open at `now`.
    #[must_use]
    pub fn is_open(&self, now: Instant) -> bool {
        self.open.as_ref().is_some_and(|open| now < open.expires)
    }

    /// When the open link runs out.
    #[must_use]
    pub fn expires(&self) -> Option<Instant> {
        self.open.as_ref().map(|open| open.expires)
    }

    /// The presentation ends: no link stays open; every token it issued, oldest first, for the
    /// log mask.
    pub fn revoke_all(&mut self) -> Vec<Secret> {
        self.open = None;
        self.failures.clear();
        std::mem::take(&mut self.issued)
    }
}

/// The link a viewer opens: the join page of the public origin, the token in the fragment.
#[must_use]
pub fn link_url(public_origin: &str, token: &str) -> String {
    format!("{public_origin}{JOIN_PATH}#t={token}")
}

fn forget_old(times: &mut VecDeque<Instant>, now: Instant) {
    while times
        .front()
        .is_some_and(|time| now.saturating_duration_since(*time) >= FAILURE_WINDOW)
    {
        times.pop_front();
    }
}

#[cfg(test)]
#[path = "link_tests.rs"]
mod tests;

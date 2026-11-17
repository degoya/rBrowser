//! Pairing and the paired devices (RB-080-05): one pairing token at a time (16 bytes from the
//! OS's random source, single use, short-lived), exchanged for a device session (32 bytes) that
//! the device keeps as an `HttpOnly`, `SameSite=Strict` cookie. Every comparison is
//! constant-time; neither secret has a `Display`, and `Debug` shows a placeholder, so no log
//! line, error or MCP answer carries one by accident. Failed pairings are limited per address.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::net::IpAddr;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::secret::{constant_time_eq, random_hex};

/// The pairing token's length in bytes (128 bits).
pub const PAIRING_BYTES: usize = 16;
/// The device session's length in bytes.
pub const SESSION_BYTES: usize = 32;
/// The name of the device cookie.
pub const COOKIE_NAME: &str = "__rbportal";
/// Failed pairings an address may make within [`FAILURE_WINDOW`].
pub const MAX_FAILURES: usize = 5;
/// The window of [`MAX_FAILURES`].
pub const FAILURE_WINDOW: Duration = Duration::from_secs(60);
/// The addresses the limiter remembers at most; the oldest are forgotten first.
pub const MAX_TRACKED_ADDRESSES: usize = 1024;
/// The longest device name kept (the user agent, shortened).
pub const MAX_NAME_CHARS: usize = 120;

/// A secret of the portal: a pairing token or a device session.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    fn generate(bytes: usize) -> std::io::Result<Self> {
        random_hex(bytes).map(Self)
    }

    /// The text: for the QR code shown to the person, the cookie and the log's mask only.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Whether `offered` is this secret, in time independent of where they differ.
    #[must_use]
    pub fn matches(&self, offered: &str) -> bool {
        constant_time_eq(self.0.as_bytes(), offered.as_bytes())
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Secret(<redacted>)")
    }
}

/// A paired device's id, unique for the portal's life.
pub type DeviceId = u32;

/// Why a pairing was refused; every one answers 403.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairError {
    /// No such token (wrong, already used or replaced).
    Invalid,
    /// The token's time is up; it is spent now.
    Expired,
    /// Too many failures from the address.
    RateLimited,
    /// `portal.maxDevices` are paired; the token stays valid.
    Full,
}

impl PairError {
    /// The i18n key of the page the device sees.
    #[must_use]
    pub fn message_key(self) -> &'static str {
        match self {
            Self::Invalid => "portal.page.invalid",
            Self::Expired => "portal.page.expired",
            Self::RateLimited => "portal.page.rate_limited",
            Self::Full => "portal.page.full",
        }
    }
}

/// A device in the list: nothing secret.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// The id `portal.disconnect` and `portal_disconnect` take.
    pub id: DeviceId,
    /// The user agent, shortened; empty when it sent none.
    pub name: String,
    /// The address it paired from.
    pub ip: String,
    /// When it paired, ms since the epoch.
    pub paired_at_ms: u64,
    /// Its last request, ms since the epoch.
    pub last_seen_ms: u64,
    /// Whether it ever came back with its cookie; `false` is a pairing not completed (the
    /// scanner app kept the cookie), which expires with the token time.
    pub contacted: bool,
}

struct Device {
    id: DeviceId,
    session: Secret,
    name: String,
    ip: IpAddr,
    paired_at: SystemTime,
    paired: Instant,
    last_seen: SystemTime,
    contacted: bool,
}

struct Pairing {
    token: Secret,
    expires: Instant,
}

/// Failed pairings per address.
#[derive(Default)]
pub struct RateLimiter {
    failures: HashMap<IpAddr, VecDeque<Instant>>,
    order: VecDeque<IpAddr>,
}

impl RateLimiter {
    /// Whether `ip` made [`MAX_FAILURES`] failures within the window before `now`.
    pub fn blocked(&mut self, ip: IpAddr, now: Instant) -> bool {
        self.failures.get_mut(&ip).is_some_and(|times| {
            times.retain(|time| now.saturating_duration_since(*time) < FAILURE_WINDOW);
            times.len() >= MAX_FAILURES
        })
    }

    /// Counts a failure of `ip`.
    pub fn fail(&mut self, ip: IpAddr, now: Instant) {
        if !self.failures.contains_key(&ip) {
            while self.failures.len() >= MAX_TRACKED_ADDRESSES {
                let Some(oldest) = self.order.pop_front() else {
                    break;
                };
                self.failures.remove(&oldest);
            }
            self.order.push_back(ip);
        }
        let times = self.failures.entry(ip).or_default();
        times.push_back(now);
        while times.len() > MAX_FAILURES {
            times.pop_front();
        }
    }

    /// The addresses remembered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.failures.len()
    }

    /// Whether none is remembered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.failures.is_empty()
    }
}

/// The pairing token and the paired devices of one running portal.
pub struct Pairings {
    pairing: Option<Pairing>,
    devices: Vec<Device>,
    next_id: DeviceId,
    limiter: RateLimiter,
    ttl: Duration,
    limit: usize,
}

impl fmt::Debug for Pairings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Pairings")
            .field("pairing", &self.pairing.is_some())
            .field("devices", &self.devices.len())
            .finish_non_exhaustive()
    }
}

impl Pairings {
    /// No token and no device; tokens live `ttl`, at most `limit` devices.
    #[must_use]
    pub fn new(ttl: Duration, limit: usize) -> Self {
        Self {
            pairing: None,
            devices: Vec::new(),
            next_id: 1,
            limiter: RateLimiter::default(),
            ttl,
            limit: limit.max(1),
        }
    }

    /// A fresh pairing token valid from `now`; the one before is void.
    ///
    /// # Errors
    ///
    /// The OS gives no randomness.
    pub fn issue(&mut self, now: Instant) -> std::io::Result<Secret> {
        let token = Secret::generate(PAIRING_BYTES)?;
        self.pairing = Some(Pairing {
            token: token.clone(),
            expires: now + self.ttl,
        });
        Ok(token)
    }

    /// When the current token runs out, if there is one.
    #[must_use]
    pub fn token_expires(&self) -> Option<Instant> {
        self.pairing.as_ref().map(|pairing| pairing.expires)
    }

    /// Exchanges `offered` for a device session: the device from `ip` with the user agent
    /// `user_agent` is paired and the token spent.
    ///
    /// # Errors
    ///
    /// [`PairError`]; the token is spent by an expiry, kept by a full portal.
    pub fn redeem(
        &mut self,
        offered: &str,
        ip: IpAddr,
        user_agent: &str,
        now: Instant,
        wall: SystemTime,
    ) -> Result<(DeviceId, Secret), PairError> {
        if self.limiter.blocked(ip, now) {
            return Err(PairError::RateLimited);
        }
        let matches = self
            .pairing
            .as_ref()
            .is_some_and(|pairing| pairing.token.matches(offered));
        if !matches {
            self.limiter.fail(ip, now);
            return Err(PairError::Invalid);
        }
        if self
            .pairing
            .as_ref()
            .is_some_and(|pairing| now >= pairing.expires)
        {
            self.pairing = None;
            self.limiter.fail(ip, now);
            return Err(PairError::Expired);
        }
        if self.devices.len() >= self.limit {
            return Err(PairError::Full);
        }
        let session = Secret::generate(SESSION_BYTES).map_err(|_| PairError::Invalid)?;
        self.pairing = None;
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.devices.push(Device {
            id,
            session: session.clone(),
            name: short_name(user_agent),
            ip,
            paired_at: wall,
            paired: now,
            last_seen: wall,
            contacted: false,
        });
        Ok((id, session))
    }

    /// Drops every device that never came back with its cookie within the token time; their
    /// sessions, oldest first (for the log mask).
    pub fn expire_unfinished(&mut self, now: Instant) -> Vec<Secret> {
        let ttl = self.ttl;
        let (gone, kept): (Vec<Device>, Vec<Device>) = self
            .devices
            .drain(..)
            .partition(|device| !device.contacted && now >= device.paired + ttl);
        self.devices = kept;
        gone.into_iter().map(|device| device.session).collect()
    }

    /// The device whose session is `offered`; its last contact becomes `wall`. Every session
    /// is compared, so the time tells nothing about which one nearly matched.
    pub fn authenticate(&mut self, offered: &str, wall: SystemTime) -> Option<DeviceId> {
        let mut found = None;
        for device in &mut self.devices {
            if device.session.matches(offered) && found.is_none() {
                device.last_seen = wall;
                device.contacted = true;
                found = Some(device.id);
            }
        }
        found
    }

    /// The device paired from `ip` that has come back with its cookie (RB-090-65): an asset
    /// listener's `GET` without the cookie - a module script, a font, a `fetch` on another port
    /// is cross-origin and goes without credentials - is the device's when it comes from there.
    /// A device not contacted yet (the scanner app kept the cookie) grants nothing; the grant
    /// ends with the session (disconnect, the portal's stop).
    pub fn device_at(&mut self, ip: IpAddr, wall: SystemTime) -> Option<DeviceId> {
        let device = self
            .devices
            .iter_mut()
            .find(|device| device.contacted && device.ip == ip)?;
        device.last_seen = wall;
        Some(device.id)
    }

    /// Revokes `device`'s session and hands it back (for the log mask); `None` when there is no
    /// such device.
    pub fn disconnect(&mut self, device: DeviceId) -> Option<Secret> {
        let at = self.devices.iter().position(|known| known.id == device)?;
        Some(self.devices.remove(at).session)
    }

    /// Revokes every session and the open token (the portal stops); the sessions, oldest first.
    pub fn revoke_all(&mut self) -> Vec<Secret> {
        self.pairing = None;
        self.devices
            .drain(..)
            .map(|device| device.session)
            .collect()
    }

    /// Whether `device` is paired.
    #[must_use]
    pub fn is_paired(&self, device: DeviceId) -> bool {
        self.devices.iter().any(|known| known.id == device)
    }

    /// The paired devices, oldest first.
    #[must_use]
    pub fn devices(&self) -> Vec<DeviceInfo> {
        self.devices
            .iter()
            .map(|device| DeviceInfo {
                id: device.id,
                name: device.name.clone(),
                ip: device.ip.to_string(),
                paired_at_ms: epoch_ms(device.paired_at),
                last_seen_ms: epoch_ms(device.last_seen),
                contacted: device.contacted,
            })
            .collect()
    }
}

/// The `Set-Cookie` value that hands `session` to the device.
#[must_use]
pub fn set_cookie(session: &Secret) -> String {
    format!(
        "{COOKIE_NAME}={}; Path=/; HttpOnly; SameSite=Strict",
        session.expose()
    )
}

/// The device session in a `Cookie` header.
#[must_use]
pub fn session_of(cookie: Option<&str>) -> Option<&str> {
    cookie?
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE_NAME)
        .map(|(_, value)| value.trim())
}

fn short_name(user_agent: &str) -> String {
    let clean: String = user_agent
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect();
    // Empty: the panel shows its own "unknown device" text.
    clean.trim().to_owned()
}

fn epoch_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |since| {
        u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
    })
}

#[cfg(test)]
#[path = "pairing_tests.rs"]
mod tests;

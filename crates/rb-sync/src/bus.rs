//! The page API's protocol (RB-020-08, `docs/sync-protocol.md`, "Page API and message bus"):
//! what a page learns about its view ([`PageState`], the agent's `page` command and the boot of
//! the main-world bundle), the bus messages a page sends ([`BusMessage`]) and the core's checks
//! before it relays one ([`check_message`], [`RateLimiter`]). The agent makes the same checks in
//! `agent/src/page-api/bridge.ts`; the core repeats them, because a page message reaches it
//! through a renderer it does not trust.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::value::RawValue;

/// The page API in the main world: `window.__rbrowser`.
pub const PAGE_API_GLOBAL: &str = "__rbrowser";
/// The parameter the core wraps the main-world bundle in:
/// `(function(__rbPageBoot){<bundle>})(<PageState>)`.
pub const PAGE_BOOT_PARAM: &str = "__rbPageBoot";
/// The longest bus type name.
pub const MAX_TYPE_LEN: usize = 64;
/// The highest `bus.maxPayloadBytes`: a message with its envelope stays under the binding's
/// 256 KiB ([`crate::MAX_MESSAGE_BYTES`]).
pub const MAX_PAYLOAD_LIMIT: u32 = 196_608;
/// The highest `bus.maxPerSecond`.
pub const MAX_RATE_LIMIT: u32 = 1000;
/// `bus.maxPayloadBytes` by default: 64 KiB of JSON text.
pub const DEFAULT_MAX_PAYLOAD_BYTES: u32 = 65_536;
/// `bus.maxPerSecond` by default.
pub const DEFAULT_MAX_PER_SECOND: u32 = 100;
/// The longest page origin a bus message may carry.
pub const MAX_ORIGIN_LEN: usize = 2048;

/// A page's bus message (agent -> core, `kind: "bus"`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BusMessage {
    /// The protocol version.
    pub v: u32,
    /// Whether the agent runs in the top frame; only the top frame has the page API.
    pub top: bool,
    /// As [`crate::Ready::frame`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<Vec<String>>,
    /// When, ms since the epoch.
    pub time: u64,
    /// The page's origin (`location.origin`, read by the agent, not the page).
    pub origin: String,
    /// The message type the page chose (`canvas:pan`).
    pub r#type: String,
    /// The page's payload, a JSON value.
    pub payload: Value,
}

/// The emulation a page sees in `window.__rbrowser.emulation`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageEmulation {
    /// The device pixel ratio; 0 keeps the display's.
    pub dpr: f64,
    /// Mobile viewport.
    pub mobile: bool,
    /// Touch screen.
    pub touch: bool,
    /// `portrait` or `landscape`, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<String>,
    /// `light` or `dark`, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_scheme: Option<String>,
    /// `prefers-reduced-motion`, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduced_motion: Option<bool>,
    /// The locale, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

/// What a page learns about its view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageInfo {
    /// The view id.
    pub id: u32,
    /// The breakpoint label, or the size after a free size.
    pub label: String,
    /// The emulated width, CSS pixels.
    pub width: u32,
    /// The emulated height, CSS pixels.
    pub height: u32,
    /// The view's place in the grid, from 0.
    pub index: u32,
    /// `chromium`, `firefox` or `webkit`.
    pub engine: String,
    /// The emulation.
    pub emulation: PageEmulation,
}

/// The bus switch and limits (`bus.*`; off as well when `pageApi.enabled` is off).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BusSettings {
    /// Whether messages are relayed.
    pub enabled: bool,
    /// The largest payload, bytes of its JSON text.
    pub max_payload_bytes: u32,
    /// Messages per second and view; more are dropped.
    pub max_per_second: u32,
}

impl Default for BusSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_payload_bytes: DEFAULT_MAX_PAYLOAD_BYTES,
            max_per_second: DEFAULT_MAX_PER_SECOND,
        }
    }
}

impl BusSettings {
    /// The settings with each limit held to its range (1 to [`MAX_PAYLOAD_LIMIT`], 1 to
    /// [`MAX_RATE_LIMIT`]); the agent refuses settings outside them.
    pub fn clamped(self) -> Self {
        Self {
            enabled: self.enabled,
            max_payload_bytes: self.max_payload_bytes.clamp(1, MAX_PAYLOAD_LIMIT),
            max_per_second: self.max_per_second.clamp(1, MAX_RATE_LIMIT),
        }
    }
}

/// The page API's state of a view: sent with the agent's `page` command, and the boot of the
/// main-world bundle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageState {
    /// The view.
    pub info: PageInfo,
    /// The bus.
    pub bus: BusSettings,
}

/// Why the core does not relay a bus message. Never quotes the payload.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BusError {
    /// The bus or the page API is off.
    #[error("the message bus is off")]
    Off,
    /// Not from the top frame.
    #[error("bus messages come from the top frame only")]
    NotTop,
    /// Not a bus type.
    #[error("invalid bus message type")]
    Type,
    /// The payload is too large.
    #[error("bus payload of {size} bytes is over {limit}")]
    TooLarge {
        /// Bytes of the payload's JSON text.
        size: usize,
        /// `bus.maxPayloadBytes`.
        limit: u32,
    },
    /// An opaque or oversized origin: nothing to compare the receivers against.
    #[error("bus messages need the page's origin")]
    Origin,
    /// Over `bus.maxPerSecond`.
    #[error("too many bus messages from this view")]
    RateLimited,
}

/// Whether `text` is a bus type: 1 to 64 of `A-Z a-z 0-9 _ . : / -`, starting with a letter or
/// digit.
pub fn is_bus_type(text: &str) -> bool {
    let mut chars = text.chars();
    text.len() <= MAX_TYPE_LEN
        && chars
            .next()
            .is_some_and(|first| first.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || "_.:/-".contains(c))
}

/// A bus payload serialised once (RB-080-27): its JSON text, measured for the size check and
/// written into every view's command as it is - never serialised again per view. Reading one
/// back (the contract tests; the core never parses its own commands) goes through a `Value`.
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct BusPayload(Box<RawValue>);

impl<'de> Deserialize<'de> for BusPayload {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Self::of(&value).map_err(serde::de::Error::custom)
    }
}

impl BusPayload {
    /// `payload` as JSON text.
    ///
    /// # Errors
    ///
    /// When `payload` cannot be serialised (a map with non-string keys).
    pub fn of<T: Serialize + ?Sized>(payload: &T) -> Result<Self, serde_json::Error> {
        serde_json::value::to_raw_value(payload).map(Self)
    }

    /// The JSON text.
    pub fn get(&self) -> &str {
        self.0.get()
    }

    /// The size of the JSON text, in bytes.
    pub fn len(&self) -> usize {
        self.0.get().len()
    }

    /// Whether the text is empty - it never is: `null` is four bytes.
    pub fn is_empty(&self) -> bool {
        self.0.get().is_empty()
    }
}

impl PartialEq for BusPayload {
    fn eq(&self, other: &Self) -> bool {
        self.0.get() == other.0.get()
    }
}

/// Checks a type and payload against `settings` (a page's message, MCP's `bus_send`); the
/// payload serialised once, for the deliveries.
pub fn check_payload<T: Serialize + ?Sized>(
    r#type: &str,
    payload: &T,
    settings: &BusSettings,
) -> Result<BusPayload, BusError> {
    if !settings.enabled {
        return Err(BusError::Off);
    }
    if !is_bus_type(r#type) {
        return Err(BusError::Type);
    }
    let limit = settings.clamped().max_payload_bytes;
    let payload = BusPayload::of(payload).map_err(|_| BusError::TooLarge {
        size: usize::MAX,
        limit,
    })?;
    let size = payload.len();
    if size > limit as usize {
        return Err(BusError::TooLarge { size, limit });
    }
    Ok(payload)
}

/// Checks a page's bus message before the core relays it: the bus on, the top frame, the
/// type, the size and a real origin. The rate is the caller's ([`RateLimiter`] per view). The
/// payload comes back serialised once, for the deliveries.
pub fn check_message(message: &BusMessage, settings: &BusSettings) -> Result<BusPayload, BusError> {
    if !settings.enabled {
        return Err(BusError::Off);
    }
    if !message.top {
        return Err(BusError::NotTop);
    }
    let origin = message.origin.as_str();
    if origin.is_empty() || origin == "null" || origin.len() > MAX_ORIGIN_LEN {
        return Err(BusError::Origin);
    }
    check_payload(&message.r#type, &message.payload, settings)
}

/// A token bucket: `per_second` messages a second, at most one second's worth stored. Times are
/// ms of any monotonic clock.
#[derive(Clone, Debug)]
pub struct RateLimiter {
    rate: f64,
    tokens: f64,
    last_ms: u64,
}

impl RateLimiter {
    /// Full, at `now_ms`.
    pub fn new(per_second: u32, now_ms: u64) -> Self {
        let rate = f64::from(per_second);
        Self {
            rate,
            tokens: rate,
            last_ms: now_ms,
        }
    }

    /// A new rate; what is stored is capped to it.
    pub fn set_rate(&mut self, per_second: u32, now_ms: u64) {
        self.refill(now_ms);
        self.rate = f64::from(per_second);
        self.tokens = self.tokens.min(self.rate);
    }

    /// Whether one more message may go at `now_ms`.
    pub fn take(&mut self, now_ms: u64) -> bool {
        self.refill(now_ms);
        if self.tokens < 1.0 {
            return false;
        }
        self.tokens -= 1.0;
        true
    }

    fn refill(&mut self, now_ms: u64) {
        let elapsed = now_ms.saturating_sub(self.last_ms);
        self.last_ms = self.last_ms.max(now_ms);
        // A second or more fills the bucket anyway.
        let elapsed = f64::from(u32::try_from(elapsed.min(1_000)).unwrap_or(1_000));
        self.tokens = (self.tokens + elapsed * self.rate / 1000.0).min(self.rate);
    }
}

#[cfg(test)]
#[path = "bus_tests.rs"]
mod tests;

//! The user-agent suffix ` rBrowser/<version>` (RB-020-08, `pageApi.uaSuffix`), so servers and
//! analytics can tell rBrowser apart. It rides on the emulation the host already sends
//! (`host/attach.rs`): the view's emulated user agent (RB-010-05) or, without one, the engine's
//! own from `Browser.getVersion`, with the suffix, through `Emulation.setUserAgentOverride`. The
//! engine's own user agent keeps its client hints: the call carries Chromium's brands, which an
//! override without them would drop.

use rb_engine::Emulation;
use serde_json::{Value, json};

/// What the suffix adds: `rBrowser/<version>`.
pub const UA_PRODUCT: &str = concat!("rBrowser/", env!("CARGO_PKG_VERSION"));
/// The GREASE brand Chromium lists beside its own.
const GREASE_BRAND: (&str, &str, &str) = ("Not_A Brand", "24", "24.0.0.0");

/// The engine's own user agent, from `Browser.getVersion`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineUserAgent {
    /// `userAgent`.
    pub user_agent: String,
    /// The Chromium version of `product` (`Chrome/131.0.6778.86`), when it names one.
    pub version: Option<String>,
}

/// The engine's user agent from the answer to `Browser.getVersion`.
pub fn engine_user_agent(answer: &Value) -> Option<EngineUserAgent> {
    let user_agent = answer["userAgent"]
        .as_str()
        .filter(|text| !text.is_empty())?;
    let version = answer["product"]
        .as_str()
        .and_then(|product| product.split_once('/'))
        .map(|(_, version)| version.to_owned())
        .filter(|version| !version.is_empty());
    Some(EngineUserAgent {
        user_agent: user_agent.to_owned(),
        version,
    })
}

/// `user_agent` ending in ` rBrowser/<version>` (once).
pub fn with_suffix(user_agent: &str) -> String {
    if user_agent.ends_with(UA_PRODUCT) {
        user_agent.to_owned()
    } else {
        format!("{user_agent} {UA_PRODUCT}")
    }
}

/// `emulation` with the suffixed user agent: its own, else the engine's. Without either it
/// stays as it is (the engine's user agent could not be read).
pub fn suffixed(emulation: &Emulation, engine: Option<&EngineUserAgent>) -> Emulation {
    let base = emulation
        .user_agent
        .as_deref()
        .or_else(|| engine.map(|engine| engine.user_agent.as_str()));
    let mut next = emulation.clone();
    if let Some(base) = base {
        next.user_agent = Some(with_suffix(base));
    }
    next
}

/// The parameters of `Emulation.setUserAgentOverride` with Chromium's brands in the client
/// hints, when the user agent is the engine's own (`original` emulates none) and its version is
/// known; unchanged otherwise.
pub fn with_brands(mut params: Value, original: &Emulation, engine: &EngineUserAgent) -> Value {
    let (None, Some(version)) = (&original.user_agent, &engine.version) else {
        return params;
    };
    let Some(metadata) = params
        .get_mut("userAgentMetadata")
        .and_then(Value::as_object_mut)
    else {
        return params;
    };
    let major = version.split('.').next().unwrap_or(version);
    let (grease, grease_major, grease_full) = GREASE_BRAND;
    metadata.insert(
        "brands".to_owned(),
        json!([
            { "brand": "Chromium", "version": major },
            { "brand": grease, "version": grease_major },
        ]),
    );
    metadata.insert(
        "fullVersionList".to_owned(),
        json!([
            { "brand": "Chromium", "version": version },
            { "brand": grease, "version": grease_full },
        ]),
    );
    metadata.insert("fullVersion".to_owned(), json!(version));
    params
}

#[cfg(test)]
#[path = "ua_tests.rs"]
mod tests;

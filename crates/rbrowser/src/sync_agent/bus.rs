//! The message bus in the core (RB-020-08): a page's bus message, reported by its agent, goes to
//! the same page in every other view - never back to the sender ([`relay_targets`]), only after
//! [`super::PageApi::accept`] (switch, top frame, type, size, origin, rate) and only into pages
//! of the sender's origin (the receiving agent compares). MCP's `bus_send` ([`bus_send`]) puts a
//! message on the bus from outside, for agents and flows; RB-030-01 registers the tool.

use rb_engine::{EngineError, Target, ViewHost, ViewId};
use rb_sync::{BusMessage, BusPayload};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::host::Host;

/// The MCP tool that sends a bus message (RB-030-01 registers it).
pub const BUS_SEND_TOOL: &str = "bus_send";
/// The tool's description for the MCP tool list.
pub const BUS_SEND_DESCRIPTION: &str = "Sends a message on the page API's message bus \
     (window.__rbrowser.bus) to the pages of `target` (\"all\" or a view id): their \
     bus.on(type) handlers get `payload` with `from: null`. `type` is 1 to 64 of A-Z a-z 0-9 \
     _ . : / -; `payload` any JSON value up to bus.maxPayloadBytes. Answers the views whose page \
     took it; a page with the bus or page API off, or without the sync agent, does not.";

/// The arguments of `bus_send`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BusSendArgs {
    /// `"all"` (the default) or a view id.
    #[serde(default = "all_views")]
    pub target: Target,
    /// The message type.
    pub r#type: String,
    /// The payload; `null` when left out.
    #[serde(default)]
    pub payload: Value,
}

fn all_views() -> Target {
    Target::All
}

/// The answer of `bus_send`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BusSendAnswer {
    /// The views whose page took the message.
    pub delivered: Vec<ViewId>,
    /// The views of `target` whose page did not.
    pub skipped: Vec<ViewId>,
}

/// Every view of `views` but `from`, in order.
pub fn relay_targets(views: &[ViewId], from: ViewId) -> Vec<ViewId> {
    views.iter().copied().filter(|view| *view != from).collect()
}

/// The views `message` of `from` goes to when the core takes it ([`relay_targets`]) and its
/// payload serialised once for them; `None` when it does not. [`super::relay::BusRelay`]
/// delivers it, each view on its own.
pub fn accepted_targets(
    host: &Host,
    from: ViewId,
    message: &BusMessage,
) -> Option<(Vec<ViewId>, BusPayload)> {
    let page = host.page_api();
    let settings = page.switches(&host.config()).bus_in_effect();
    let payload = match page.accept(from, message, &settings) {
        Ok(payload) => payload,
        Err(error) => {
            // The reason only, never the payload.
            tracing::debug!(view = %from, %error, "bus message not relayed");
            return None;
        }
    };
    let views: Vec<ViewId> = host.views().iter().map(|info| info.id).collect();
    Some((relay_targets(&views, from), payload))
}

/// MCP's `bus_send`: the message from the core to the pages of `args.target`, of any origin.
pub async fn bus_send(host: &Host, args: BusSendArgs) -> Result<BusSendAnswer, EngineError> {
    let page = host.page_api();
    let settings = page.switches(&host.config()).bus_in_effect();
    let payload = rb_sync::bus::check_payload(&args.r#type, &args.payload, &settings)
        .map_err(|error| EngineError::Invalid(error.to_string()))?;
    let targets = host.target_views(Some(args.target))?;
    let delivered = page
        .deliver(&targets, None, None, &args.r#type, &payload)
        .await;
    let skipped = targets
        .into_iter()
        .filter(|view| !delivered.contains(view))
        .collect();
    Ok(BusSendAnswer { delivered, skipped })
}

#[cfg(test)]
#[path = "bus_tests.rs"]
mod tests;

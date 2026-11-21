//! How a plugin granted `load-test` asks the core for the load test's work (RB-096-07), the API
//! client's pattern (`crate::api_client::protocol`) with keys of its own:
//!
//! - Every call of every plugin loses the [`OFFER_KEY`] and [`RESULT_KEY`] a caller sent
//!   ([`offered`]): no MCP client, panel or command argument forges an answer or an origin.
//! - A plugin granted `load-test` finds `"$loadTest": "<origin>"` in its arguments - `panel`,
//!   `tool` (MCP and every other tool call) or `command`.
//! - It may answer `{"$loadTest": {"op": "...", ...}}` ([`requested`], [`Op`]); the core runs the
//!   op if the origin may ([`permitted`]) and calls the same tool or command once more with the
//!   op's answer under [`RESULT_KEY`] and without the offer ([`with_result`]).
//!
//! The origins (a review condition of the job): only the panel - a person in front of it -
//! saves, confirms a host, previews and starts a run, and opens the API client panel
//! (RB-096-09); a command (a key, the activity bar's Stop) may stop it; a tool call (MCP) only
//! reads the reports - it can neither start, change nor stop a run, nor confirm a host.

use rb_plugin_types::{Failure, FailureKind, codes};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::scenario::Scenario;
pub use crate::api_client::protocol::Origin;

/// The host's offer in a call's arguments (the origin), and the plugin's request in its answer.
pub const OFFER_KEY: &str = "$loadTest";
/// The argument the op's answer arrives under.
pub const RESULT_KEY: &str = "$loadResult";

/// What the person saw and clicked Start on: the run starts only when it is still this.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Confirm {
    /// The target hosts, sorted.
    pub targets: Vec<String>,
    pub users: u32,
    /// The rate the run is held to.
    pub rps: f64,
    pub duration_s: u64,
}

/// What a plugin asks for.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub enum Op {
    /// The panel's state: trust, saved requests, scenarios, ceilings, hosts, the run, reports.
    State,
    /// One scenario.
    Load { id: String },
    /// Stores `scenario` as `id`, or as a new file named after its title.
    Save {
        #[serde(default)]
        id: Option<String>,
        scenario: Scenario,
    },
    /// Deletes the scenario `id`.
    Delete { id: String },
    /// What a run of the scenario `id` would do: targets and their standing, users, rate,
    /// duration - the summary the person confirms.
    Preview { id: String },
    /// The person confirms that `host` is theirs or that they may test it.
    Allow { host: String, confirmed: bool },
    /// Starts the scenario `id`, when it still is what the person confirmed.
    Start { id: String, confirm: Confirm },
    /// Stops the running run.
    Stop,
    /// The running or last run's numbers.
    Status,
    /// The reports, newest first.
    Reports,
    /// One report, the newest without `id`.
    Report {
        #[serde(default)]
        id: Option<String>,
    },
    /// Two reports side by side.
    Compare { a: String, b: String },
    /// Opens the API client panel, where the saved requests are made (RB-096-09): the panel's
    /// hint while there are none.
    OpenApiClient,
}

impl Op {
    /// The op's id, for a failure.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            Self::State => "state",
            Self::Load { .. } => "load",
            Self::Save { .. } => "save",
            Self::Delete { .. } => "delete",
            Self::Preview { .. } => "preview",
            Self::Allow { .. } => "allow",
            Self::Start { .. } => "start",
            Self::Stop => "stop",
            Self::Status => "status",
            Self::Reports => "reports",
            Self::Report { .. } => "report",
            Self::Compare { .. } => "compare",
            Self::OpenApiClient => "openApiClient",
        }
    }
}

fn origin_of(id: &str) -> Option<Origin> {
    [Origin::Panel, Origin::Tool, Origin::Command]
        .into_iter()
        .find(|origin| origin.id() == id)
}

/// `args` without the keys a caller put there and, for a plugin `granted` the load test, with
/// the offer of `origin`; `null` becomes an object only for such a plugin.
#[must_use]
pub fn offered(args: &Value, granted: bool, origin: Origin) -> Value {
    let offer = || Value::String(origin.id().to_owned());
    match args {
        Value::Object(map) => {
            let mut map = map.clone();
            map.remove(OFFER_KEY);
            map.remove(RESULT_KEY);
            if granted {
                map.insert(OFFER_KEY.to_owned(), offer());
            }
            Value::Object(map)
        }
        Value::Null if granted => {
            let mut map = Map::new();
            map.insert(OFFER_KEY.to_owned(), offer());
            Value::Object(map)
        }
        other => other.clone(),
    }
}

/// The origin `args` offer, when they offer the load test.
#[must_use]
pub fn offer_of(args: &Value) -> Option<Origin> {
    args.get(OFFER_KEY)?.as_str().and_then(origin_of)
}

/// The op a plugin's `answer` asks for: `None` for an answer that asks nothing; `Some(Err)` -
/// `plugins.error.invalid` - for one that does not read.
#[must_use]
pub fn requested(answer: &str) -> Option<Result<Op, Failure>> {
    let value: Value = serde_json::from_str(answer).ok()?;
    let request = value.get(OFFER_KEY)?;
    Some(Op::deserialize(request).map_err(|error| {
        Failure::coded(FailureKind::BadData, codes::INVALID).with_param(
            "reason",
            rb_util::text::cut(&format!("load test op: {error}"), 200),
        )
    }))
}

/// Whether `origin` may run `op`.
///
/// # Errors
///
/// `plugins.error.capability_denied` (capability `load-test`, the op).
pub fn permitted(op: &Op, origin: Origin) -> Result<(), Failure> {
    let reading = matches!(op, Op::Reports | Op::Report { .. } | Op::Compare { .. });
    let allowed = match origin {
        Origin::Panel => true,
        Origin::Command => reading || matches!(op, Op::Stop),
        Origin::Tool => reading,
    };
    if allowed {
        Ok(())
    } else {
        Err(
            Failure::coded(FailureKind::AccessDenied, codes::CAPABILITY_DENIED)
                .with_param("capability", "load-test")
                .with_param("op", op.id()),
        )
    }
}

/// `args` for the second call: the op's `result` under [`RESULT_KEY`], the offer gone.
#[must_use]
pub fn with_result(args: &Value, result: Value) -> Value {
    let mut map = match args {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };
    map.remove(OFFER_KEY);
    map.insert(RESULT_KEY.to_owned(), result);
    Value::Object(map)
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;

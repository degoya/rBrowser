//! How a plugin granted `api-client` asks the core for the API client's work (RB-096-01), the
//! consent probe's pattern (`crate::plugins::consent::args`):
//!
//! - Every call of every plugin loses the [`OFFER_KEY`] and [`RESULT_KEY`] a caller sent
//!   ([`offered`]): no MCP client, panel or command argument can forge an answer or an origin.
//! - A plugin granted `api-client` finds `"$apiClient": "<origin>"` in its arguments - `panel`
//!   for its panel's calls, `tool` for MCP's and every other tool call, `command` for a command.
//! - It may answer `{"$apiClient": {"op": "...", ...}}` ([`requested`], [`Op`]); the core runs the
//!   op if the origin may ([`permitted`]) and calls the same tool or command once more with the
//!   op's answer under [`RESULT_KEY`] and without the offer ([`with_result`]).
//!
//! The origins: the panel may do everything; a tool or a command (MCP) may list and read the
//! collections and the history, run a saved request, run a saved collection, read its report,
//! benchmark a saved request (RB-096-04), write a saved request as code (RB-096-03) and take
//! captured requests into the collections (RB-096-08; their secrets' values never kept, nothing
//! opened) - never a free request, never a save of its own, a delete, an export, an import or a
//! host grant; nor a collection made, renamed, duplicated or deleted, a request moved or
//! duplicated, the environment file or a private value written (RB-096-11): a person's in the
//! panel.

use std::collections::BTreeMap;

use rb_api_model::ApiRequest;
use rb_plugin_types::{Failure, FailureKind, codes};
use serde::Deserialize;
use serde_json::{Map, Value};

/// The host's offer in a call's arguments (the origin), and the plugin's request in its answer.
pub const OFFER_KEY: &str = "$apiClient";
/// The argument the op's answer arrives under.
pub const RESULT_KEY: &str = "$apiResult";

/// Who called the plugin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Its own panel (`PluginRuntime::panel_call_tool`): a person in front of it.
    Panel,
    /// A tool call from anywhere else: MCP above all.
    Tool,
    /// One of its commands (a key, a menu).
    Command,
}

impl Origin {
    /// The offer's text.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::Tool => "tool",
            Self::Command => "command",
        }
    }

    fn of(id: &str) -> Option<Self> {
        [Self::Panel, Self::Tool, Self::Command]
            .into_iter()
            .find(|origin| origin.id() == id)
    }
}

/// What a plugin asks for.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub enum Op {
    /// The panel's state: trust, environments, variables (names), collections, allowed hosts.
    State,
    /// The collections: every request's id, title, method and URL template.
    List,
    /// One saved request, as the model and as its file's text.
    Load { id: String },
    /// Stores `request` as `id`, or as a new file in `folder` named after its title.
    Save {
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        folder: Option<String>,
        request: ApiRequest,
    },
    /// Deletes the saved request `id`.
    Delete { id: String },
    /// Sends `request` as it is (the panel's draft).
    Send {
        request: ApiRequest,
        #[serde(default)]
        environment: Option<String>,
        /// The saved request it was loaded from, for the history.
        #[serde(default)]
        id: Option<String>,
    },
    /// Sends the saved request `id`.
    Run {
        id: String,
        #[serde(default)]
        environment: Option<String>,
    },
    /// Allows the project to send to `host` from now on: a person's answer in the panel.
    Allow { host: String },
    /// The last sends.
    History,
    /// Runs every saved request of `folder` (all of them without one) and judges their checks
    /// (RB-096-04): the report, its failures as findings.
    RunCollection {
        #[serde(default)]
        folder: Option<String>,
        #[serde(default)]
        environment: Option<String>,
    },
    /// The last collection run's report.
    Report,
    /// Benchmarks the saved request `id` in up to two environments (the views' without one).
    Bench {
        id: String,
        #[serde(default)]
        environments: Vec<String>,
    },
    /// A request as code (RB-096-03): the saved `id`, or the panel's draft `request`.
    Code {
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        request: Option<ApiRequest>,
        language: rb_api_export::Language,
        #[serde(default)]
        environment: Option<String>,
    },
    /// Writes the requests of `folder` (every one without it) as `format` below
    /// `.rbrowser/out/api-client/exports/` (RB-096-03): a person's choice in the panel.
    Export {
        #[serde(default)]
        folder: Option<String>,
        format: rb_api_export::Format,
    },
    /// Reads the collections of another tool (RB-096-02) and stores them below `.rbrowser/api/`;
    /// `preview` reads them only. The panel's alone: a person picked the files.
    Import {
        files: Vec<ImportFile>,
        /// A format's id (`postman`, `openApi`, ...); none to tell it by the files.
        #[serde(default)]
        format: Option<String>,
        /// The folder below `.rbrowser/api/`; none for one named after the collection.
        #[serde(default)]
        folder: Option<String>,
        /// Whether the environments go into `http-client.env.json`.
        #[serde(default = "yes")]
        environments: bool,
        #[serde(default)]
        preview: bool,
    },
    /// Takes requests the views captured into the API client (RB-096-08,
    /// [`super::capture_ops`]): `save` stores each below `.rbrowser/api/<host>/`, `open` hands the
    /// first to the panel's editor (a person's, the panel's alone); neither answers them only.
    Capture {
        requests: Vec<CaptureRef>,
        #[serde(default)]
        save: bool,
        #[serde(default)]
        open: bool,
    },
    /// Makes the collection `folder` below `.rbrowser/api/` (RB-096-11, [`super::folders`]).
    CreateFolder { folder: String },
    /// Renames the collection `folder` to `to`, never over another.
    RenameFolder { folder: String, to: String },
    /// Copies the collection `folder` to a free `<folder>-copy`.
    DuplicateFolder { folder: String },
    /// Deletes the collection `folder` with its requests (the panel asked the person first).
    DeleteFolder { folder: String },
    /// Copies the saved request `id` beside itself, titled `name`.
    Duplicate {
        id: String,
        #[serde(default)]
        name: Option<String>,
    },
    /// Moves the saved request `id` into `folder` (`""` the top), never over another.
    Move { id: String, folder: String },
    /// The Variables tab: the environment file, the private names, the environments' URLs.
    Variables,
    /// Writes the environment file and patches the private values ([`super::variables`]).
    SaveVariables {
        file: serde_json::Value,
        /// The file as the panel read it: refused when it changed since.
        #[serde(default)]
        base: serde_json::Value,
        #[serde(default)]
        private: super::variables::PrivatePatch,
        #[serde(default)]
        renamed: BTreeMap<String, String>,
        /// Environments deleted: their private values dropped before the renames.
        #[serde(default)]
        deleted: Vec<String>,
    },
    /// Sets the private value `name` - `{{$secret name}}` - in `environment` (`$shared` without).
    SetSecret {
        name: String,
        value: SecretText,
        #[serde(default)]
        environment: Option<String>,
    },
    /// Which `{{$secret name}}`s of `names` have no value in `environment`.
    Secrets {
        names: Vec<String>,
        #[serde(default)]
        environment: Option<String>,
    },
}

/// A private value on its way in (RB-096-11): never in a `Debug` output, so never in a log.
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct SecretText(pub String);

impl std::fmt::Debug for SecretText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretText(***)")
    }
}

/// A request a view captured: the view and the host's request id (the network panel's).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureRef {
    pub view: u32,
    pub id: String,
}

const fn yes() -> bool {
    true
}

/// A file of an import, or the pasted text (`name` empty).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportFile {
    #[serde(default)]
    pub name: String,
    pub text: String,
}

impl Op {
    /// The op's id, for a failure.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            Self::State => "state",
            Self::List => "list",
            Self::Load { .. } => "load",
            Self::Save { .. } => "save",
            Self::Delete { .. } => "delete",
            Self::Send { .. } => "send",
            Self::Run { .. } => "run",
            Self::Allow { .. } => "allow",
            Self::History => "history",
            Self::RunCollection { .. } => "runCollection",
            Self::Report => "report",
            Self::Bench { .. } => "bench",
            Self::Code { .. } => "code",
            Self::Export { .. } => "export",
            Self::Import { .. } => "import",
            Self::Capture { .. } => "capture",
            Self::CreateFolder { .. } => "createFolder",
            Self::RenameFolder { .. } => "renameFolder",
            Self::DuplicateFolder { .. } => "duplicateFolder",
            Self::DeleteFolder { .. } => "deleteFolder",
            Self::Duplicate { .. } => "duplicate",
            Self::Move { .. } => "move",
            Self::Variables => "variables",
            Self::SaveVariables { .. } => "saveVariables",
            Self::SetSecret { .. } => "setSecret",
            Self::Secrets { .. } => "secrets",
        }
    }
}

/// `args` without the keys a caller put there and, for a plugin `granted` the API client, with
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

/// The origin `args` offer, when they offer the API client.
#[must_use]
pub fn offer_of(args: &Value) -> Option<Origin> {
    args.get(OFFER_KEY)?.as_str().and_then(Origin::of)
}

/// The op a plugin's `answer` asks for: `None` for an answer that asks nothing (no object, no
/// [`OFFER_KEY`]); `Some(Err)` - `plugins.error.invalid` - for one that does not read.
#[must_use]
pub fn requested(answer: &str) -> Option<Result<Op, Failure>> {
    let value: Value = serde_json::from_str(answer).ok()?;
    let request = value.get(OFFER_KEY)?;
    Some(Op::deserialize(request).map_err(|error| {
        Failure::coded(FailureKind::BadData, codes::INVALID).with_param(
            "reason",
            rb_util::text::cut(&format!("api client op: {error}"), 200),
        )
    }))
}

/// Whether `origin` may run `op`.
///
/// # Errors
///
/// `plugins.error.capability_denied` (capability `api-client`, the op) for an op only the panel
/// runs.
pub fn permitted(op: &Op, origin: Origin) -> Result<(), Failure> {
    let anyone = matches!(
        op,
        Op::List
            | Op::Load { .. }
            | Op::Run { .. }
            | Op::History
            | Op::RunCollection { .. }
            | Op::Report
            | Op::Bench { .. }
            | Op::Code { request: None, .. }
            | Op::Capture { open: false, .. }
    );
    if anyone || origin == Origin::Panel {
        Ok(())
    } else {
        Err(
            Failure::coded(FailureKind::AccessDenied, codes::CAPABILITY_DENIED)
                .with_param("capability", "api-client")
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

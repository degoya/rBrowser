//! The API client's core (RB-096-01): the first-party plugin `api-client` (its panel and its
//! MCP tools) asks, the core does the work - reads and writes the collections, puts the
//! variables in, holds every send to the hosts a person allowed and sends from Rust, without
//! CORS, timed. The request model and its `.http` file are `rb_api_model`.
//!
//! | Part | Where |
//! | --- | --- |
//! | The capability `api-client`: the offer, the op the plugin asks for, what each origin may | [`protocol`] |
//! | The plugin call with the op in between (around the consent probe's) | [`call`] |
//! | The ops over the project, its trust, environments, grants and settings | [`ops`] |
//! | The collections below `.rbrowser/api/`, the environment file, body files; no link followed | [`store`] |
//! | The collections as folders: made, renamed, duplicated, deleted; a request moved or duplicated (RB-096-11) | [`folders`] |
//! | The panel's collection, variable and private value ops (RB-096-11) | [`manage`] |
//! | The hosts allowed (`api-client.allowedHosts` in `config.local.json`) | [`grants`] |
//! | Variables: environments, private values, `RB_SECRET_*`, `$timestamp`, `$guid` | [`resolve`] |
//! | The Variables tab: the environment file written, the private values patched, `$secret`s without a value (RB-096-11) | [`variables`] |
//! | The request as bytes, the send, redirects held to the grants, the answer masked | [`send`], [`timing`] |
//! | The last sends (`.rbrowser/out/api-client/history.json`) | [`history`] |
//! | One request delivered: checked, resolved, held to the grants, sent, redirects followed | [`deliver`] |
//! | A collection run with its checks and report (`.rbrowser/out/api-client/report.json`), its failures as findings (RB-096-04) | [`collection`], [`findings`] |
//! | The benchmark of a saved request: few repeats, little concurrency, one or two environments (RB-096-04) | [`bench`] |
//! | A request as code, a folder as a Postman collection, `.http` file or curl script (RB-096-03) | [`export`] |
//! | The import of other tools' collections (`rb_api_import`), the panel's alone | [`import`] |
//! | Requests the views captured taken in: secrets as variables, a file per request in a folder per host, the editor's draft (RB-096-08) | [`capture`], [`capture_ops`] |
//! | The window's host: the context of an op, the grant written, the capture gathered | [`window`] |
//!
//! Secrets: a value from `RB_SECRET_*` or from `api-client.secrets` (`config.local.json`) is put
//! in by the core alone, registered with the log mask and masked in every answer, failure
//! parameter and history entry; the plugin, its panel and MCP see the templates and the names.

pub mod bench;
pub mod call;
pub mod capture;
pub mod capture_ops;
pub mod collection;
pub mod deliver;
pub mod export;
#[cfg(test)]
mod fakes;
pub mod findings;
pub mod folders;
pub mod grants;
pub mod history;
pub mod import;
pub mod manage;
pub mod ops;
pub mod protocol;
pub mod resolve;
pub mod send;
pub mod store;
pub mod timing;
pub mod variables;
pub mod window;

use rb_plugin_types::{Failure, FailureKind, codes};

/// The plugin's id: the prefix of its settings keys and texts.
pub const PLUGIN_ID: &str = "api-client";
/// The settings key of the private values.
pub const SECRETS_KEY: &str = "api-client.secrets";
/// The settings key of the history's size.
pub const HISTORY_SIZE_KEY: &str = "api-client.historySize";
/// The settings key of the largest answer body read.
pub const MAX_RESPONSE_KEY: &str = "api-client.maxResponseBytes";
/// The settings key of the benchmark's repeats per environment (RB-096-04).
pub const BENCH_REPEATS_KEY: &str = "api-client.benchRepeats";
/// The settings key of the benchmark's sends at once (RB-096-04).
pub const BENCH_CONCURRENCY_KEY: &str = "api-client.benchConcurrency";
/// The default history size.
pub const DEFAULT_HISTORY_SIZE: usize = 50;
/// The default and the largest answer body read: the panel's answer must stay below the
/// plugins' 4 MiB message limit with the body as base64.
pub const DEFAULT_MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
/// The largest `api-client.maxResponseBytes`.
pub const MAX_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;
/// The largest file a body or a multipart part sends.
pub const MAX_BODY_FILE_BYTES: u64 = 32 * 1024 * 1024;

/// The failure `plugin.api-client.errors.<name>`: the plugin's own text, so its panel shows it.
#[must_use]
pub fn error(kind: FailureKind, name: &str) -> Failure {
    Failure::coded(kind, &format!("plugin.{PLUGIN_ID}.errors.{name}"))
}

/// A failure's code for the history and a list: its code, or the host's generic one.
#[must_use]
pub fn failure_code(failure: &Failure) -> String {
    failure.code().unwrap_or(codes::HOST_FAILED).to_owned()
}

/// Starts the window's host when one of `plugins` is granted `api-client` and none runs yet (at
/// the start and after a live change).
pub fn follow(core: &crate::control::Core, plugins: &[crate::plugins::runtime::Plugin]) {
    let wanted = plugins.iter().any(|plugin| plugin.component.api_client());
    if wanted && call::host().is_none() {
        window::start(core);
    }
}

//! The load test (RB-096-07): the first-party plugin `load-test` (its panel and its MCP tools)
//! asks, the core does the work - deliberately loading a server the person owns with many
//! virtual users, from saved requests of the API client (RB-096-01), to find its limits.
//!
//! | Part | Where |
//! | --- | --- |
//! | The scenario file below `.rbrowser/api/load/`: steps, weights, think times, per-user values, profile, abort rules | [`scenario`], [`store`] |
//! | Users over the profile, the ceilings of the settings (no ceiling in code) | [`profile`] |
//! | The shared rate, a user's next step and pause | [`pace`], [`pick`] |
//! | The numbers per second and of the run, the abort rules | [`stats`], [`histogram`], [`abort`] |
//! | Where a run may send: own origins, this machine, hosts the person confirmed | [`gate`] |
//! | The summary the person confirms and the run built from it | [`plan`] |
//! | The run: async users on the core's runtime, Stop at every wait | [`engine`] |
//! | One request: the API client's `deliver`, an honest sender of its own | [`shot`], [`sender`] |
//! | The live view and the report (JSON, HTML, two compared) | [`snapshot`], [`report`], [`report_html`] |
//! | The capability `load-test`: the offer, the op, what each origin may | [`protocol`], [`call`] |
//! | The ops, and the window's host: context, confirmed hosts, the one run, the activity bar | [`ops`], [`window`] |
//!
//! The safeguards, each a review condition of the job and each tested where it is enforced:
//! targets only the trusted project's own origins and this machine, any other host only after
//! the person confirmed it ([`gate`], held on every hop by `crate::api_client::deliver`); a run
//! starts only from the panel with the summary the person saw ([`protocol::permitted`],
//! [`plan::launch`]); MCP only reads reports; the ceilings are settings only ([`profile`]);
//! Stop and closing rBrowser end every connection at once ([`engine`], [`sender`],
//! [`window::stop_all`]); every request names itself and hides no one ([`sender::identified`]),
//! and goes out without a proxy.

pub mod abort;
pub mod call;
pub mod engine;
pub mod gate;
pub mod histogram;
pub mod ops;
pub mod pace;
pub mod pick;
pub mod plan;
pub mod profile;
pub mod protocol;
pub mod report;
pub mod report_html;
pub mod scenario;
pub mod sender;
pub mod shot;
pub mod snapshot;
pub mod stats;
pub mod store;
pub mod window;

use rb_plugin_types::{Failure, FailureKind};

/// The plugin's id: the prefix of its settings keys and texts.
pub const PLUGIN_ID: &str = "load-test";

/// The failure `plugin.load-test.errors.<name>`: the plugin's own text, so its panel shows it.
#[must_use]
pub fn error(kind: FailureKind, name: &str) -> Failure {
    Failure::coded(kind, &format!("plugin.{PLUGIN_ID}.errors.{name}"))
}

/// Starts the window's host when one of `plugins` is granted `load-test` and none runs yet (at
/// the start and after a live change).
pub fn follow(core: &crate::control::Core, plugins: &[crate::plugins::runtime::Plugin]) {
    let wanted = plugins.iter().any(|plugin| plugin.component.load_test());
    if wanted && call::host().is_none() {
        window::start(core);
    }
}

/// Ends a running load test because rBrowser closes: every connection at once.
pub fn on_closing() {
    if window::stop_all(engine::End::Closed) {
        tracing::info!("the running load test ended: the window closes");
    }
}

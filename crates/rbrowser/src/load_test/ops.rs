//! The ops of the load test (RB-096-07, [`super::protocol::Op`]) over one [`Context`] - the
//! window's project, trust, environments, ceilings and confirmed hosts, taken once per op - and
//! the run as it stands ([`RunState`]). Every op needs an open project in a trusted workspace:
//! the scenarios and requests are the project's files, and a cloned project's run is not even
//! planned before a person trusted it. Starting a run is two steps for the person: `preview`
//! gives the summary, `start` echoes what they confirmed ([`super::plan::launch`]).

use rb_plugin_types::{Failure, FailureKind};
use serde_json::{Value, json};

use super::error;
use super::gate::{LOCAL_HOSTS, Standing};
use super::plan::{Context, Launch, launch, plan, summary_json};
use super::protocol::Op;
use super::report::{Reports, compare};
use super::store::Scenarios;
use crate::api_client::grants::{normalized, with_host};
use crate::api_client::resolve::EnvReader;
use crate::api_client::store::Store;

/// The API client plugin, whose panel `openApiClient` opens (RB-096-09).
pub const API_CLIENT_PLUGIN: &str = "api-client";
/// The API client's command that shows its panel (`[[commands]] id = "panel"`).
pub const API_CLIENT_PANEL_COMMAND: &str = "plugin.api-client.panel";

/// Whether `openApiClient` may answer: the API client's panel command is `registered` only while
/// that plugin is on and loaded; without it the op is refused with `api_client_missing`, which the
/// panel says, instead of answering `opening` for a panel that never comes.
///
/// # Errors
///
/// `api_client_missing` when the command is not registered.
pub fn api_client_reachable(registered: bool) -> Result<(), Failure> {
    if registered {
        Ok(())
    } else {
        Err(error(FailureKind::NotFound, "api_client_missing"))
    }
}

/// The run as an op finds it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunState {
    /// Whether one runs now.
    pub running: bool,
    /// The running or the last run's numbers, as the panel shows them.
    pub snapshot: Option<Value>,
}

/// An op's answer and what the window does with it.
pub struct Outcome {
    pub answer: Value,
    /// The confirmed hosts to keep, when the op changed them.
    pub confirmed: Option<Vec<String>>,
    /// The run to start.
    pub launch: Option<Launch>,
    /// Stop the running run.
    pub stop: bool,
    /// Open the API client panel (RB-096-09).
    pub open_api_client: bool,
}

impl Outcome {
    fn answer(answer: Value) -> Self {
        Self {
            answer,
            confirmed: None,
            launch: None,
            stop: false,
            open_api_client: false,
        }
    }
}

fn state(ctx: &Context, root: &std::path::Path, run: &RunState) -> Result<Value, Failure> {
    let requests = Store::new(root).list()?;
    let gate = ctx.gate();
    Ok(json!({
        "trusted": ctx.api.trusted,
        "environment": ctx.api.environment,
        "environments": ctx.api.environments.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        "requests": requests,
        "scenarios": Scenarios::new(root).list()?,
        "limits": {
            "maxUsers": ctx.limits.max_users,
            "maxRps": ctx.limits.max_rps,
            "maxDurationS": ctx.limits.max_duration_s,
        },
        "hosts": { "own": gate.own(), "local": LOCAL_HOSTS, "confirmed": ctx.confirmed },
        "running": run.running,
        "run": run.snapshot,
        "reports": Reports::new(root).list()?,
    }))
}

fn allow(ctx: &Context, host: &str, confirmed: bool) -> Result<Outcome, Failure> {
    if !confirmed {
        return Err(error(FailureKind::BadData, "not_confirmed"));
    }
    let host = normalized(host).ok_or_else(|| error(FailureKind::BadData, "bad_host"))?;
    if ctx.gate().standing(&host) != Standing::Unconfirmed {
        return Ok(Outcome::answer(
            json!({ "allowed": host, "allowedHosts": ctx.confirmed }),
        ));
    }
    let list = with_host(&ctx.confirmed, &host)
        .ok_or_else(|| error(FailureKind::Limit, "too_many_hosts"))?;
    Ok(Outcome {
        answer: json!({ "allowed": host, "allowedHosts": list }),
        confirmed: Some(list),
        launch: None,
        stop: false,
        open_api_client: false,
    })
}

/// Runs `op` (already permitted for its caller, [`super::protocol::permitted`]) with the run as
/// `run` finds it, process variables read by `env`.
///
/// # Errors
///
/// `no_project`, `untrusted`, `already_running`, `not_running`, and the op's own.
pub fn run(
    op: Op,
    ctx: &Context,
    run: &RunState,
    env: &'static EnvReader,
) -> Result<Outcome, Failure> {
    let root = ctx
        .api
        .root
        .clone()
        .ok_or_else(|| error(FailureKind::NotFound, "no_project"))?;
    if !ctx.api.trusted {
        return Err(error(FailureKind::AccessDenied, "untrusted"));
    }
    let scenarios = Scenarios::new(&root);
    let reports = Reports::new(&root);
    Ok(match op {
        Op::State => Outcome::answer(state(ctx, &root, run)?),
        Op::Load { id } => Outcome::answer(json!({ "id": id, "scenario": scenarios.read(&id)? })),
        Op::Save { id, scenario } => {
            let id = scenarios.save(id.as_deref(), &scenario)?;
            Outcome::answer(json!({ "id": id }))
        }
        Op::Delete { id } => {
            scenarios.delete(&id)?;
            Outcome::answer(json!({ "deleted": id }))
        }
        Op::Preview { id } => Outcome::answer(summary_json(&plan(ctx, &root, &id, env)?.summary)),
        Op::Allow { host, confirmed } => allow(ctx, &host, confirmed)?,
        Op::Start { id, confirm } => {
            if run.running {
                return Err(error(FailureKind::Limit, "already_running"));
            }
            let planned = plan(ctx, &root, &id, env)?;
            let launch = launch(ctx, &root, planned, &confirm, env)?;
            Outcome {
                answer: json!({ "started": true, "summary": summary_json(&launch.summary) }),
                confirmed: None,
                launch: Some(launch),
                stop: false,
                open_api_client: false,
            }
        }
        Op::Stop => {
            if !run.running {
                return Err(error(FailureKind::NotFound, "not_running"));
            }
            Outcome {
                answer: json!({ "stopping": true }),
                confirmed: None,
                launch: None,
                stop: true,
                open_api_client: false,
            }
        }
        Op::Status => Outcome::answer(json!({ "running": run.running, "run": run.snapshot })),
        Op::Reports => Outcome::answer(json!({ "reports": reports.list()? })),
        Op::Report { id } => Outcome::answer(reports.read(id.as_deref())?),
        Op::Compare { a, b } => {
            Outcome::answer(compare(&reports.read(Some(&a))?, &reports.read(Some(&b))?))
        }
        Op::OpenApiClient => Outcome {
            open_api_client: true,
            ..Outcome::answer(json!({ "opening": API_CLIENT_PLUGIN }))
        },
    })
}

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;

//! A run planned from its scenario (RB-096-07): the scenario read and held to its rules and to
//! the ceilings of the settings, each step's saved request read, the target hosts found by
//! putting in the variables as a send would (for each user's values), each target's standing
//! at the gate - and from it the summary the person confirms ([`Summary::confirm`]) and, on
//! Start, the run itself ([`launch`]): every hop held to [`Gate::allowed`], the run's own
//! sender, the plan of the engine and what its report says.

use rb_plugin_types::{Failure, FailureKind};
use serde::Serialize;
use serde_json::Value;

use super::engine::Plan;
use super::error;
use super::gate::{Gate, Standing};
use super::profile::{Limits, check_limits, rate_of};
use super::protocol::Confirm;
use super::report::Meta;
use super::scenario::{Scenario, personalize};
use super::sender::LoadSender;
use super::shot::{Live, Target};
use super::store::Scenarios;
use crate::api_client::grants::host_of;
use crate::api_client::ops::{Context as ApiContext, environment_of, layers};
use crate::api_client::resolve::{EnvReader, Variables, resolve};
use crate::api_client::store::Store;

/// The most users whose values are tried for the targets.
const USERS_TRIED: u32 = 100;

/// What the load test's ops work on.
#[derive(Clone, Debug)]
pub struct Context {
    /// The API client's context: project, trust, environments, private values, settings.
    pub api: ApiContext,
    /// The project's `startUrl`.
    pub start_url: Option<String>,
    pub limits: Limits,
    /// `load-test.allowedHosts`.
    pub confirmed: Vec<String>,
}

impl Context {
    /// The gate of the project: its own origins, the confirmed hosts.
    #[must_use]
    pub fn gate(&self) -> Gate {
        let own: Vec<String> = self
            .start_url
            .iter()
            .cloned()
            .chain(self.api.environments.iter().map(|(_, url)| url.clone()))
            .collect();
        Gate::new(&own, &self.confirmed)
    }
}

/// A target and its standing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetHost {
    pub host: String,
    pub standing: Standing,
}

/// What a run would do, as the person confirms it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub scenario: String,
    pub name: String,
    pub environment: Option<String>,
    pub targets: Vec<TargetHost>,
    /// The targets the person has not confirmed yet: no run before they are.
    pub unconfirmed: Vec<String>,
    pub users: u32,
    pub rps: f64,
    pub duration_s: u64,
    pub ramp_up_s: u32,
    pub hold_s: u32,
    pub ramp_down_s: u32,
    pub steps: usize,
    /// What Start sends back.
    pub confirm: Confirm,
}

/// A run planned.
pub struct Planned {
    pub summary: Summary,
    pub scenario: Scenario,
    pub targets: Vec<Target>,
    pub hosts: Vec<String>,
}

fn unreadable_request(id: &str, failure: &Failure) -> Failure {
    error(FailureKind::BadData, "request_unreadable")
        .with_param("id", id)
        .with_param("reason", crate::api_client::failure_code(failure))
}

/// Plans the scenario `id` of the project at `root`, process variables read by `env`.
///
/// # Errors
///
/// The scenario's (`not_found`, `unreadable`, `bad_id`), `invalid_scenario` with the field,
/// `over_limit` with what, the wanted value, the limit and the setting, `request_unreadable`
/// with the request, `variables_unreadable`, `unresolved` with the names, `bad_target` for a
/// request that is no `http(s)` URL.
pub fn plan(
    ctx: &Context,
    root: &std::path::Path,
    id: &str,
    env: &EnvReader,
) -> Result<Planned, Failure> {
    let scenario = Scenarios::new(root).read(id)?;
    scenario.check().map_err(|field| {
        error(FailureKind::BadData, "invalid_scenario").with_param("field", field)
    })?;
    check_limits(&scenario.profile, &ctx.limits).map_err(|over| {
        error(FailureKind::Limit, "over_limit")
            .with_param("what", over.what)
            .with_param("wanted", over.wanted)
            .with_param("limit", over.limit)
            .with_param("setting", over.setting)
    })?;
    let store = Store::new(root);
    let environment = environment_of(&ctx.api, scenario.environment.clone());
    let mut targets = Vec::new();
    for step in &scenario.steps {
        let (request, _) = store
            .read(&step.request)
            .map_err(|failure| unreadable_request(&step.request, &failure))?;
        targets.push(Target {
            id: step.request.clone(),
            request,
        });
    }
    let variables = Variables {
        layers: layers(&ctx.api, &store, environment.clone()).map_err(|failure| {
            error(FailureKind::BadData, "variables_unreadable")
                .with_param("reason", crate::api_client::failure_code(&failure))
        })?,
        env,
        now: ctx.api.now,
    };
    let tried = scenario
        .variables
        .values()
        .map(|values| u32::try_from(values.len()).unwrap_or(USERS_TRIED))
        .max()
        .unwrap_or(1)
        .clamp(1, USERS_TRIED);
    let mut hosts = Vec::new();
    for target in &targets {
        for user in 0..tried {
            let personal = personalize(&target.request, &scenario.values_of(user), user, 0);
            let (resolved, _secrets) = resolve(&personal, &variables).map_err(|missing| {
                error(FailureKind::BadData, "unresolved")
                    .with_param("names", missing.names.join(", "))
            })?;
            let host = host_of(&resolved.url).ok_or_else(|| {
                error(FailureKind::BadData, "bad_target").with_param("id", &target.id)
            })?;
            hosts.push(host);
        }
    }
    hosts.sort();
    hosts.dedup();
    let gate = ctx.gate();
    let rps = rate_of(&scenario.profile, &ctx.limits);
    let profile = &scenario.profile;
    let summary = Summary {
        scenario: id.to_owned(),
        name: scenario.name.clone(),
        environment: environment.clone(),
        targets: hosts
            .iter()
            .map(|host| TargetHost {
                host: host.clone(),
                standing: gate.standing(host),
            })
            .collect(),
        unconfirmed: gate.unconfirmed(&hosts),
        users: profile.users,
        rps,
        duration_s: profile.duration_s(),
        ramp_up_s: profile.ramp_up_s,
        hold_s: profile.hold_s,
        ramp_down_s: profile.ramp_down_s,
        steps: scenario.steps.len(),
        confirm: Confirm {
            targets: hosts.clone(),
            users: profile.users,
            rps,
            duration_s: profile.duration_s(),
        },
    };
    Ok(Planned {
        summary,
        scenario,
        targets,
        hosts,
    })
}

/// A run ready to go.
pub struct Launch {
    pub plan: Plan,
    pub live: Live,
    pub meta: Meta,
    pub summary: Summary,
}

/// The run of `planned`, when the person `confirm`ed what it does and every target may be one.
///
/// # Errors
///
/// `host_not_confirmed` with the first host the person has not confirmed, `summary_changed`
/// when the scenario, the settings or the hosts changed since the person saw the summary.
pub fn launch(
    ctx: &Context,
    root: &std::path::Path,
    planned: Planned,
    confirm: &Confirm,
    env: &'static EnvReader,
) -> Result<Launch, Failure> {
    if let Some(host) = planned.summary.unconfirmed.first() {
        return Err(error(FailureKind::AccessDenied, "host_not_confirmed").with_param("host", host));
    }
    if planned.summary.confirm != *confirm {
        return Err(error(FailureKind::BadData, "summary_changed"));
    }
    let Planned {
        summary,
        scenario,
        targets,
        hosts,
    } = planned;
    let gate = ctx.gate();
    let mut api = ctx.api.clone();
    api.allowed_hosts = gate.allowed(&hosts);
    let profile = scenario.profile.clone();
    let plan = Plan {
        profile: profile.clone(),
        rps: Some(summary.rps),
        order: scenario.order,
        weights: scenario.steps.iter().map(|step| step.weight).collect(),
        thinks: scenario.steps.iter().map(|step| step.think_ms).collect(),
        abort: scenario.abort.clone(),
    };
    let meta = Meta {
        scenario: summary.scenario.clone(),
        name: scenario.name.clone(),
        started_at: ctx.api.now,
        profile,
        rps_limit: summary.rps,
        environment: summary.environment.clone(),
        targets: summary
            .targets
            .iter()
            .map(|target| (target.host.clone(), target.standing))
            .collect(),
        steps: scenario
            .steps
            .iter()
            .map(|step| step.request.clone())
            .collect(),
    };
    let live = Live {
        ctx: api,
        store: Store::new(root),
        sender: LoadSender::new(scenario.profile.users),
        steps: targets,
        environment: summary.environment.clone(),
        scenario,
        env,
    };
    Ok(Launch {
        plan,
        live,
        meta,
        summary,
    })
}

/// `summary` as the panel and the start's answer have it.
#[must_use]
pub fn summary_json(summary: &Summary) -> Value {
    serde_json::to_value(summary).unwrap_or(Value::Null)
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;

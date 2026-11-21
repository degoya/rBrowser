//! The ops of the API client (RB-096-01, [`super::protocol::Op`]) over one [`Context`] - the
//! window's project, trust, environments, grants and settings, taken once per op - and a
//! [`Sender`]: the real one in the app, a fake in tests. Every op needs an open project in a
//! trusted workspace: the collections are the project's files, and a cloned project's requests
//! run only after a person trusted it. A send is held to the grants before anything goes out,
//! and so is every redirect's host; a host not granted answers `host_not_allowed` with the host,
//! which the panel turns into its question.

use std::path::PathBuf;

use rb_api_model::{ApiRequest, EnvFile, Layers};
use rb_plugin_types::{Failure, FailureKind, Finding};
use serde_json::{Value, json};

use super::deliver::{Courier, deliver};
use super::grants;
use super::history::{self, Entry};
use super::protocol::Op;
use super::resolve::EnvReader;
use super::send::{self, Exchange, Prepared};
use super::store::Store;
use super::{bench, collection, error, failure_code, findings};

/// The settings an op reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// `api-client.historySize`.
    pub history_size: usize,
    /// `api-client.maxResponseBytes`.
    pub max_response_bytes: u64,
    /// `api-client.benchRepeats` (RB-096-04).
    pub bench_repeats: u32,
    /// `api-client.benchConcurrency` (RB-096-04).
    pub bench_concurrency: u32,
    /// `api-client.codeShell` and `api-client.codeVariables` (RB-096-03).
    pub code: super::export::CodeSettings,
}

/// What an op works on.
#[derive(Clone, Debug)]
pub struct Context {
    pub root: Option<PathBuf>,
    pub trusted: bool,
    /// The rBrowser environment the views show, by id.
    pub environment: Option<String>,
    /// The project's rBrowser environments: id and URL.
    pub environments: Vec<(String, String)>,
    /// `api-client.allowedHosts`.
    pub allowed_hosts: Vec<String>,
    /// `api-client.secrets`: the private values, as the environment file has them.
    pub secrets: Value,
    pub settings: Settings,
    /// Seconds since the epoch.
    pub now: u64,
}

/// Sends a prepared request.
pub trait Sender: Send + Sync {
    /// The answer of `prepared`.
    fn send(
        &self,
        prepared: &Prepared,
        limit: u64,
        secrets: &[String],
    ) -> Result<Exchange, Failure>;
}

/// An op's answer, the grants to keep when it changed them, the findings to show when it ran a
/// collection (RB-096-04; an empty list clears the plugin's earlier ones), and the private
/// values to keep when it changed them (RB-096-11; never in the answer) and the environment file
/// to write after them (`env_file`, [`super::variables::commit`]).
#[derive(Clone, PartialEq)]
pub struct Outcome {
    pub answer: Value,
    pub allowed_hosts: Option<Vec<String>>,
    pub findings: Option<Vec<Finding>>,
    pub secrets: Option<Value>,
    pub env_file: Option<Value>,
}

impl std::fmt::Debug for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Outcome")
            .field("answer", &self.answer)
            .field("allowed_hosts", &self.allowed_hosts)
            .field("findings", &self.findings)
            .field("secrets", &self.secrets.as_ref().map(|_| "***"))
            .field("env_file", &self.env_file.is_some())
            .finish()
    }
}

impl Outcome {
    pub(super) fn answer(answer: Value) -> Self {
        Self {
            answer,
            allowed_hosts: None,
            findings: None,
            secrets: None,
            env_file: None,
        }
    }
}

/// Runs `op` (already permitted for its caller, [`super::protocol::permitted`]), process
/// variables read by `env`.
///
/// # Errors
///
/// `no_project`, `untrusted`, and the op's own.
pub fn run(
    op: Op,
    ctx: &Context,
    sender: &dyn Sender,
    env: &EnvReader,
) -> Result<Outcome, Failure> {
    let root = ctx
        .root
        .clone()
        .ok_or_else(|| error(FailureKind::NotFound, "no_project"))?;
    if !ctx.trusted {
        return Err(error(FailureKind::AccessDenied, "untrusted"));
    }
    let store = Store::new(&root);
    Ok(match op {
        Op::State => Outcome::answer(state(ctx, &store)?),
        Op::List => Outcome::answer(json!({ "requests": store.list()? })),
        Op::Load { id } => {
            let (request, text) = store.read(&id)?;
            Outcome::answer(json!({ "id": id, "request": request, "text": text }))
        }
        Op::Save {
            id,
            folder,
            request,
        } => {
            let id = store.save(id.as_deref(), folder.as_deref(), &request)?;
            Outcome::answer(json!({ "id": id }))
        }
        Op::Delete { id } => {
            store.delete(&id)?;
            Outcome::answer(json!({ "deleted": id }))
        }
        Op::Send {
            request,
            environment,
            id,
        } => Outcome::answer(send_request(
            ctx,
            &store,
            &request,
            id.as_deref(),
            environment,
            sender,
            env,
        )?),
        Op::Run { id, environment } => {
            let (request, _) = store.read(&id)?;
            Outcome::answer(send_request(
                ctx,
                &store,
                &request,
                Some(&id),
                environment,
                sender,
                env,
            )?)
        }
        Op::Allow { host } => {
            let host =
                grants::normalized(&host).ok_or_else(|| error(FailureKind::BadData, "bad_host"))?;
            let hosts = grants::with_host(&ctx.allowed_hosts, &host)
                .ok_or_else(|| error(FailureKind::Limit, "too_many_hosts"))?;
            Outcome {
                answer: json!({ "allowed": host, "allowedHosts": hosts }),
                allowed_hosts: Some(hosts),
                findings: None,
                secrets: None,
                env_file: None,
            }
        }
        Op::History => Outcome::answer(json!({ "entries": history::read(&root) })),
        Op::RunCollection {
            folder,
            environment,
        } => {
            let courier = courier_of(ctx, &store, sender, env);
            let environment = environment_of(ctx, environment);
            let report = collection::run(courier, folder.as_deref(), environment)?;
            if let Err(problem) = collection::keep(&root, &report) {
                tracing::debug!(kind = %problem.kind(), "the API client's report was not written");
            }
            Outcome {
                findings: Some(findings::of(&report)),
                answer: json!(report),
                allowed_hosts: None,
                secrets: None,
                env_file: None,
            }
        }
        Op::Report => Outcome::answer(json!({ "report": collection::last(&root) })),
        Op::Bench { id, environments } => {
            let courier = courier_of(ctx, &store, sender, env);
            let environments = if environments.is_empty() {
                vec![environment_of(ctx, None)]
            } else {
                environments
                    .into_iter()
                    .map(|name| environment_of(ctx, Some(name)))
                    .collect()
            };
            let settings = ctx.settings;
            let result = bench::run(
                courier,
                &id,
                environments,
                settings.bench_repeats,
                settings.bench_concurrency,
            )?;
            Outcome::answer(json!(result))
        }
        Op::Code {
            id,
            request,
            language,
            environment,
        } => Outcome::answer(super::export::code(
            ctx,
            &store,
            id.as_deref(),
            request,
            language,
            environment,
        )?),
        Op::Export { folder, format } => Outcome::answer(super::export::export(
            ctx,
            &store,
            &root,
            folder.as_deref(),
            format,
        )?),
        Op::Import {
            files,
            format,
            folder,
            environments,
            preview,
        } => Outcome::answer(super::import::run(
            &store,
            super::import::Wanted {
                files,
                format,
                folder,
                environments,
                preview,
            },
        )?),
        // The window takes the views' capture (`super::window`, `super::capture_ops`): an op
        // context holds none.
        Op::Capture { .. } => return Err(error(FailureKind::NotSupported, "capture_unavailable")),
        // The collections, the variables and the private values (RB-096-11).
        other => super::manage::run(other, ctx, &store, env)?,
    })
}

fn courier_of<'a>(
    ctx: &'a Context,
    store: &'a Store,
    sender: &'a dyn Sender,
    env: &'a EnvReader,
) -> Courier<'a> {
    Courier {
        ctx,
        store,
        sender,
        env,
    }
}

/// The environment in effect: the op's, else the views'.
pub fn environment_of(ctx: &Context, wanted: Option<String>) -> Option<String> {
    wanted
        .filter(|name| !name.trim().is_empty())
        .or_else(|| ctx.environment.clone())
}

/// The variable layers of `environment`: the project's environment file, the private values,
/// the environment's base URL (shared with the load test, RB-096-07).
///
/// # Errors
///
/// `secrets_unreadable`, and [`Store::env_file`]'s.
pub fn layers(
    ctx: &Context,
    store: &Store,
    environment: Option<String>,
) -> Result<Layers, Failure> {
    let private = EnvFile::from_json(&ctx.secrets).map_err(|reason| {
        error(FailureKind::BadData, "secrets_unreadable")
            .with_param("reason", rb_util::text::cut(&reason, 200))
    })?;
    let base_url = environment.as_ref().and_then(|name| {
        ctx.environments
            .iter()
            .find(|(id, _)| id == name)
            .map(|(_, url)| url.trim_end_matches('/').to_owned())
    });
    Ok(Layers {
        public: store.env_file()?,
        private,
        environment,
        base_url,
    })
}

fn state(ctx: &Context, store: &Store) -> Result<Value, Failure> {
    let public = store.env_file();
    let mut names: Vec<String> = ctx.environments.iter().map(|(id, _)| id.clone()).collect();
    if let Ok(file) = &public {
        names.extend(file.names());
    }
    names.sort();
    names.dedup();
    let environment = environment_of(ctx, None);
    let variables = layers(ctx, store, environment.clone())
        .map(|layers| {
            layers
                .names()
                .into_iter()
                .map(|(name, secret)| json!({ "name": name, "secret": secret }))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(json!({
        "environment": environment,
        "environments": names,
        "variables": variables,
        "requests": store.list()?,
        "folders": store.folders()?,
        "allowedHosts": ctx.allowed_hosts,
        "envProblem": public.err().map(|failure| failure_code(&failure)),
    }))
}

fn send_request(
    ctx: &Context,
    store: &Store,
    request: &ApiRequest,
    id: Option<&str>,
    environment: Option<String>,
    sender: &dyn Sender,
    env: &EnvReader,
) -> Result<Value, Failure> {
    let environment = environment_of(ctx, environment);
    let courier = courier_of(ctx, store, sender, env);
    let sent = deliver(courier, request, id, environment.clone())?;
    let result = &sent.result;
    let entry = Entry {
        at: ctx.now,
        id: id.map(str::to_owned),
        method: request.method.clone(),
        url: request.url.clone(),
        environment: environment.clone(),
        status: result.as_ref().ok().map(|exchange| exchange.status),
        total_ms: result.as_ref().ok().map(|exchange| exchange.phases.total),
        error: result.as_ref().err().map(failure_code),
    };
    if let Some(root) = ctx.root.as_deref()
        && let Err(problem) = history::append(root, entry, ctx.settings.history_size)
    {
        tracing::debug!(kind = %problem.kind(), "the API client's history was not written");
    }
    let verdicts = sent.verdicts(request);
    let exchange = sent.result?;
    let mut answer = send::answer(&sent.prepared, &exchange, &sent.secrets);
    answer["redirects"] = json!(sent.redirects);
    answer["environment"] = json!(environment);
    if !request.checks.is_empty() {
        answer["checks"] = json!(verdicts);
    }
    Ok(answer)
}

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;

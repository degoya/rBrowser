//! Runs the check plugins: each describes its rules, then runs those switched on in every view
//! of the target - at most `checks.parallel` runs at once - and the findings are held to the
//! policy and merged. A plugin that fails is named in the run's failures - and in the log, with
//! its reason; the run goes on. A view whose engine the plugin cannot serve is skipped, no
//! failure (RB-080-98). Every plugin x view job's state goes to the run's sink as it
//! changes, with the findings so far whenever a plugin is through (RB-080-89).

use std::sync::{Arc, Mutex, PoisonError};
use std::{collections::BTreeMap, time::Duration};

use rb_plugin_types::{
    CheckContext, CheckFailure, CheckSkip, CheckTarget, CheckedView, FINDINGS_VERSION, Failure,
    Finding, FindingsFile, FindingsSummary, MessageParams, RuleInfo, SKIP_ENGINE, ViewId, codes,
};
use tokio::{sync::Semaphore, task::JoinSet};

use super::{
    CheckSource,
    jobs::{Ended, Job, run_jobs},
    merge::{Report, merge},
    policy::{ChecksPolicy, ChecksSettings},
    progress::{JobProgress, JobState, RunControl, RunOutcome, SkipReason},
};

/// One view of the target, as the application knows it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RunView {
    pub id: ViewId,
    /// The breakpoint id it shows; `None` for a free size.
    pub breakpoint: Option<String>,
    /// The URL it shows.
    pub url: Option<String>,
}

/// What to run.
#[derive(Clone, Debug, Default)]
pub struct RunRequest {
    /// [`super::run_id`] of the start.
    pub run_id: String,
    /// Unix seconds.
    pub started_at: u64,
    pub target: CheckTarget,
    /// The target's views; empty runs once without a view (the CLI against a URL).
    pub views: Vec<RunView>,
    /// The URL asked for; the views' own when `None`.
    pub url: Option<String>,
    /// Only these plugins; every given source when `None`.
    pub plugins: Option<Vec<String>>,
    /// Arguments for every plugin, JSON text.
    pub args: String,
}

/// The runner, with the settings of one run.
pub struct CheckRunner {
    policy: ChecksPolicy,
    parallel: usize,
}

/// Runs `jobs` with at most `limit` at once; the answers in the jobs' order, `None` for a job
/// that panicked - so an answer never moves to another job's place.
async fn bounded<T: Send + 'static>(limit: usize, jobs: Vec<Job<T>>) -> Vec<Option<T>> {
    let count = jobs.len();
    let gate = Arc::new(Semaphore::new(limit.max(1)));
    let mut set = JoinSet::new();
    for (index, job) in jobs.into_iter().enumerate() {
        let gate = Arc::clone(&gate);
        set.spawn(async move {
            let _permit = gate.acquire_owned().await;
            (index, job.await)
        });
    }
    let mut answers: Vec<Option<T>> = std::iter::repeat_with(|| None).take(count).collect();
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok((index, answer)) => {
                if let Some(slot) = answers.get_mut(index) {
                    *slot = Some(answer);
                }
            }
            Err(error) => tracing::warn!(%error, "a check job ended without an answer"),
        }
    }
    answers
}

/// The failure of a job the host could not run to its end (it panicked).
fn host_failed(plugin: &str, view: Option<ViewId>) -> CheckFailure {
    CheckFailure {
        plugin: plugin.to_owned(),
        view,
        code: Some(codes::HOST_FAILED.to_owned()),
        params: MessageParams::new(),
    }
}

/// `failure` with the parameters its sentence needs (`{service}`, RB-080-98).
fn failure_of(plugin: &str, view: Option<ViewId>, failure: &Failure) -> CheckFailure {
    CheckFailure {
        plugin: plugin.to_owned(),
        view,
        code: failure.code().map(str::to_owned),
        params: failure.params.clone(),
    }
}

/// One plugin with the rules it runs.
struct Planned {
    source: Arc<dyn CheckSource>,
    rules: BTreeMap<String, RuleInfo>,
}

impl CheckRunner {
    /// A runner holding `settings` (parallel limit, rule overrides, ignore patterns).
    #[must_use]
    pub fn new(settings: &ChecksSettings) -> Self {
        Self {
            policy: ChecksPolicy::new(settings),
            parallel: settings.parallel(),
        }
    }

    /// Runs `sources` (the enabled check plugins) as `request` says.
    pub async fn run(&self, sources: &[Arc<dyn CheckSource>], request: RunRequest) -> FindingsFile {
        self.run_with(sources, request, &RunControl::default())
            .await
            .file
    }

    /// Runs `sources` as `request` says, telling `control`'s sink every change of a plugin x
    /// view job and the findings so far whenever a plugin is through; `control`'s cancel stops
    /// it with what the finished jobs found (RB-080-89).
    pub async fn run_with(
        &self,
        sources: &[Arc<dyn CheckSource>],
        request: RunRequest,
        control: &RunControl,
    ) -> RunOutcome {
        let mut failures = Vec::new();
        let mut skipped: Vec<CheckSkip> = Vec::new();
        let selected = select(sources, request.plugins.as_deref(), &mut failures);
        let plugins: Vec<String> = selected
            .iter()
            .map(|source| source.plugin_id().to_owned())
            .collect();
        let views: Vec<Option<&RunView>> = if request.views.is_empty() {
            vec![None]
        } else {
            request.views.iter().map(Some).collect()
        };

        let (planned, mut table) = self.plan(selected, &mut failures).await;
        let mut jobs = Vec::new();
        let mut contexts = Vec::new();
        for (plan, view) in planned
            .iter()
            .flat_map(|plan| views.iter().map(move |view| (plan, *view)))
        {
            let ctx = CheckContext {
                view: view.map(|view| view.id),
                url: request
                    .url
                    .clone()
                    .or_else(|| view.and_then(|view| view.url.clone())),
                rules: plan.rules.keys().cloned().collect(),
                args: request.args.clone(),
            };
            let source = Arc::clone(&plan.source);
            let job_ctx = ctx.clone();
            jobs.push(Box::pin(async move { source.run(job_ctx).await }) as Job<_>);
            table.push(JobProgress::new(
                plan.source.plugin_id(),
                ctx.view,
                JobState::Queued,
            ));
            contexts.push((plan, view, ctx));
        }
        let first_job = table.len() - contexts.len();
        let tell = |table: &[JobProgress], partial: Option<&FindingsFile>| {
            if let Some(sink) = &control.sink {
                sink.update(table, partial);
            }
        };
        tell(&table, None);

        let shell = Shell {
            request: &request,
            plugins: &plugins,
        };
        let table = Mutex::new(table);
        let mut reports: Vec<Report> = Vec::new();
        run_jobs(
            self.parallel,
            jobs,
            &control.cancel,
            |index| {
                let mut table = table.lock().unwrap_or_else(PoisonError::into_inner);
                table[first_job + index].state = JobState::Running;
                tell(&table, None);
            },
            |index, ended: Ended<Result<Vec<Finding>, Failure>>| {
                let (plan, view, ctx) = &contexts[index];
                let mut table = table.lock().unwrap_or_else(PoisonError::into_inner);
                let row = &mut table[first_job + index];
                match ended {
                    Ended::Answered(Ok(found), elapsed) => {
                        let kept: Vec<Report> = found
                            .into_iter()
                            .filter_map(|finding| self.report(plan, *view, ctx, finding))
                            .collect();
                        row.state = JobState::Done {
                            findings: kept.len(),
                        };
                        row.elapsed_ms = Some(millis(elapsed));
                        reports.extend(kept);
                    }
                    Ended::Answered(Err(failure), elapsed) => {
                        let plugin = plan.source.plugin_id();
                        row.state = JobState::ended_by(&failure, ctx.view);
                        row.elapsed_ms = Some(millis(elapsed));
                        if let JobState::Skipped {
                            engine: Some(engine),
                            ..
                        } = &row.state
                        {
                            skipped.push(CheckSkip {
                                plugin: plugin.to_owned(),
                                view: ctx.view,
                                reason: SKIP_ENGINE.to_owned(),
                                engine: Some(engine.clone()),
                            });
                            tracing::info!(
                                plugin,
                                view = ?ctx.view,
                                engine,
                                "a check plugin cannot serve the view's engine; skipped"
                            );
                        } else {
                            tracing::warn!(
                                plugin,
                                view = ?ctx.view,
                                code = failure.code().unwrap_or("-"),
                                params = ?failure.params,
                                "a check plugin failed; its findings are missing"
                            );
                            failures.push(failure_of(plugin, ctx.view, &failure));
                        }
                    }
                    Ended::Panicked => {
                        let plugin = plan.source.plugin_id();
                        tracing::warn!(
                            plugin,
                            view = ?ctx.view,
                            code = codes::HOST_FAILED,
                            "a check job panicked; the plugin's findings in this view are missing"
                        );
                        row.state = JobState::host_failed();
                        failures.push(host_failed(plugin, ctx.view));
                    }
                    Ended::Cancelled => {
                        row.state = JobState::skipped(SkipReason::Cancelled);
                    }
                }
                let plugin = plan.source.plugin_id();
                let through = table
                    .iter()
                    .filter(|job| job.plugin == plugin)
                    .all(|job| job.state.ended());
                if through && !control.cancel.is_cancelled() {
                    let partial = shell.file(reports.clone(), failures.clone(), skipped.clone());
                    tell(&table, Some(&partial));
                } else {
                    tell(&table, None);
                }
            },
        )
        .await;
        RunOutcome {
            file: shell.file(reports, failures, skipped),
            jobs: table.into_inner().unwrap_or_else(PoisonError::into_inner),
            cancelled: control.cancel.is_cancelled(),
        }
    }

    /// Asks each plugin for its rules and keeps those the policy runs; a plugin that fails to
    /// answer or runs no rule gets one row of its own in the table (no view).
    async fn plan(
        &self,
        selected: Vec<Arc<dyn CheckSource>>,
        failures: &mut Vec<CheckFailure>,
    ) -> (Vec<Planned>, Vec<JobProgress>) {
        let described: Vec<Option<Result<Vec<RuleInfo>, Failure>>> = bounded(
            self.parallel,
            selected
                .iter()
                .map(|source| {
                    let source = Arc::clone(source);
                    Box::pin(async move { source.describe().await }) as Job<_>
                })
                .collect(),
        )
        .await;
        let mut planned = Vec::new();
        let mut table = Vec::new();
        for (source, rules) in selected.into_iter().zip(described) {
            let plugin = source.plugin_id().to_owned();
            match rules {
                Some(Ok(rules)) => {
                    let rules: BTreeMap<String, RuleInfo> = rules
                        .into_iter()
                        .filter(|rule| self.policy.enabled(&qualified(&source, &rule.id)))
                        .map(|rule| (rule.id.clone(), rule))
                        .collect();
                    if rules.is_empty() {
                        let state = JobState::skipped(SkipReason::RulesOff);
                        table.push(JobProgress::new(&plugin, None, state));
                    } else {
                        planned.push(Planned { source, rules });
                    }
                }
                Some(Err(failure)) => {
                    tracing::warn!(
                        plugin,
                        code = failure.code().unwrap_or("-"),
                        %failure,
                        "a check plugin did not describe its rules; it does not run"
                    );
                    let state = JobState::Failed {
                        code: failure.code().map(str::to_owned),
                        params: failure.params.clone(),
                    };
                    table.push(JobProgress::new(&plugin, None, state));
                    failures.push(failure_of(&plugin, None, &failure));
                }
                None => {
                    tracing::warn!(
                        plugin,
                        code = codes::HOST_FAILED,
                        "describing a check plugin's rules panicked; it does not run"
                    );
                    table.push(JobProgress::new(&plugin, None, JobState::host_failed()));
                    failures.push(host_failed(&plugin, None));
                }
            }
        }
        (planned, table)
    }

    /// One finding held to its plugin's rules and the policy; `None` drops it.
    fn report(
        &self,
        plan: &Planned,
        view: Option<&RunView>,
        ctx: &CheckContext,
        mut finding: Finding,
    ) -> Option<Report> {
        let rule = plan.rules.get(&finding.rule)?;
        let rule_id = qualified(&plan.source, &rule.id);
        let severity = self.policy.severity(&rule_id, finding.severity)?;
        if finding.url.is_none() {
            finding.url.clone_from(&ctx.url);
        }
        if self.policy.ignored(
            &rule_id,
            finding.url.as_deref(),
            finding.selector.as_deref(),
        ) {
            return None;
        }
        Some(Report {
            plugin: plan.source.plugin_id().to_owned(),
            rule_id,
            shared_id: rule.shared_id.clone(),
            help_url: rule.help_url.clone(),
            severity,
            view: view.map(|view| view.id).or(finding.view),
            breakpoint: view.and_then(|view| view.breakpoint.clone()),
            finding,
        })
    }
}

/// What every file of a run shares: the request and the plugins it selected.
struct Shell<'a> {
    request: &'a RunRequest,
    plugins: &'a [String],
}

impl Shell<'_> {
    /// The run's file with `reports` merged and `failures` named.
    fn file(
        &self,
        reports: Vec<Report>,
        failures: Vec<CheckFailure>,
        skipped: Vec<CheckSkip>,
    ) -> FindingsFile {
        let request = self.request;
        let findings = merge(reports);
        FindingsFile {
            version: FINDINGS_VERSION,
            run_id: request.run_id.clone(),
            started_at: request.started_at,
            target: request.target,
            url: request.url.clone(),
            plugins: self.plugins.to_vec(),
            views: request
                .views
                .iter()
                .map(|view| CheckedView {
                    id: view.id,
                    breakpoint: view.breakpoint.clone(),
                })
                .collect(),
            summary: FindingsSummary::of(&findings),
            findings,
            failures,
            skipped,
        }
    }
}

fn millis(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

fn qualified(source: &Arc<dyn CheckSource>, rule: &str) -> String {
    format!("{}.{rule}", source.plugin_id())
}

/// The sources `asked` names (all without a list); a name no source has is a failure.
fn select(
    sources: &[Arc<dyn CheckSource>],
    asked: Option<&[String]>,
    failures: &mut Vec<CheckFailure>,
) -> Vec<Arc<dyn CheckSource>> {
    let Some(asked) = asked else {
        return sources.to_vec();
    };
    for name in asked {
        if !sources.iter().any(|source| source.plugin_id() == name) {
            failures.push(CheckFailure {
                plugin: name.clone(),
                view: None,
                code: Some(codes::NOT_FOUND.to_owned()),
                params: MessageParams::new(),
            });
        }
    }
    sources
        .iter()
        .filter(|source| asked.iter().any(|name| name == source.plugin_id()))
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;

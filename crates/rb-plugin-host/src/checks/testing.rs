//! A check source in memory for the runner's tests: fixed rules and findings, a failure on
//! demand, and what it saw and how many ran at once.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use async_trait::async_trait;
use rb_plugin_types::{CheckContext, Failure, FailureKind, Finding, RuleInfo, Severity, codes};

use super::CheckSource;

/// Shared by several fakes: how many run now, and the most that ran at once.
#[derive(Debug, Default)]
pub(crate) struct Gauge {
    running: AtomicUsize,
    pub(crate) peak: AtomicUsize,
}

pub(crate) struct FakeSource {
    pub(crate) id: String,
    pub(crate) rules: Vec<RuleInfo>,
    pub(crate) findings: Vec<Finding>,
    pub(crate) fail: bool,
    /// The views it fails in, each with its failure.
    pub(crate) fail_in: Vec<(u32, Failure)>,
    /// The failure it answers in every view.
    pub(crate) fail_with: Option<Failure>,
    /// `describe` panics (a host fault in the job, not the plugin's failure).
    pub(crate) panic_describe: bool,
    /// `run` panics.
    pub(crate) panic_run: bool,
    /// The views it finds anything in; every view when empty.
    pub(crate) only_views: Vec<u32>,
    pub(crate) delay: Duration,
    pub(crate) gauge: Arc<Gauge>,
    pub(crate) seen: Mutex<Vec<CheckContext>>,
}

pub(crate) fn rule(id: &str, shared: Option<&str>) -> RuleInfo {
    RuleInfo {
        id: id.to_owned(),
        title: format!("plugin.x.rules.{id}"),
        severity: Severity::Warning,
        shared_id: shared.map(str::to_owned),
        help_url: Some(format!("https://help.test/{id}")),
    }
}

pub(crate) fn finding(rule: &str, severity: Severity, selector: &str) -> Finding {
    Finding {
        rule: rule.to_owned(),
        severity,
        message: format!("plugin.x.findings.{rule}"),
        params: vec![("count".to_owned(), "2".to_owned())],
        selector: Some(selector.to_owned()),
        path: Some(format!("html > body > {selector}")),
        ..Finding::default()
    }
}

impl FakeSource {
    pub(crate) fn new(id: &str, rules: Vec<RuleInfo>, findings: Vec<Finding>) -> Self {
        Self {
            id: id.to_owned(),
            rules,
            findings,
            fail: false,
            fail_in: Vec::new(),
            fail_with: None,
            panic_describe: false,
            panic_run: false,
            only_views: Vec::new(),
            delay: Duration::ZERO,
            gauge: Arc::new(Gauge::default()),
            seen: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn contexts(&self) -> Vec<CheckContext> {
        self.seen
            .lock()
            .map(|seen| seen.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl CheckSource for FakeSource {
    fn plugin_id(&self) -> &str {
        &self.id
    }

    async fn describe(&self) -> Result<Vec<RuleInfo>, Failure> {
        assert!(!self.panic_describe, "the fake's describe panics");
        Ok(self.rules.clone())
    }

    async fn run(&self, ctx: CheckContext) -> Result<Vec<Finding>, Failure> {
        let now = self.gauge.running.fetch_add(1, Ordering::SeqCst) + 1;
        self.gauge.peak.fetch_max(now, Ordering::SeqCst);
        tokio::time::sleep(self.delay).await;
        self.gauge.running.fetch_sub(1, Ordering::SeqCst);
        assert!(!self.panic_run, "the fake's run panics");
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(ctx.clone());
        }
        if self.fail {
            return Err(Failure::coded(FailureKind::Other, codes::TIMED_OUT));
        }
        if let Some(failure) = &self.fail_with {
            return Err(failure.clone());
        }
        if let Some((_, failure)) = self
            .fail_in
            .iter()
            .find(|(view, _)| ctx.view == Some(*view))
        {
            return Err(failure.clone());
        }
        if !self.only_views.is_empty()
            && !ctx.view.is_some_and(|view| self.only_views.contains(&view))
        {
            return Ok(Vec::new());
        }
        // Everything it has, whatever `ctx.rules` says: the runner holds a plugin to its rules.
        Ok(self.findings.clone())
    }
}

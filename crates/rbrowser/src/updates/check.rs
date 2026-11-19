//! The update check of the app (RB-090-14): one at a time, whoever asks - the start, a person,
//! MCP, an install that needs an offer first; a caller that waited for a running check takes its
//! outcome instead of starting another ([`Serial`]). An expired manifest is nothing to offer,
//! logged, never a notice; a refused one (forged, replayed, misplaced) always is. A newer offer
//! replaces a verified download of an older one, whose file goes.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rb_ipc::NotificationLevel;
use rb_update::auto_install::AutoMode;
use rb_update::{Gate, HttpFetcher, Sources, Target, Utc};
use serde_json::{Value, json};

use super::flow::{
    Origin, VERSION, channel, facts, fail, install_kind, show, start_download, status,
};
use super::notices;
use super::state::{self, Phase};
use crate::control::Core;

/// Runs one piece of work at a time; a caller whose ticket a finished run made stale takes
/// `joined` instead of running again.
pub(super) struct Serial {
    lock: Mutex<()>,
    finished: AtomicU64,
}

impl Serial {
    pub(super) const fn new() -> Self {
        Self {
            lock: Mutex::new(()),
            finished: AtomicU64::new(0),
        }
    }

    /// Taken before waiting: which runs had finished when the caller asked.
    pub(super) fn ticket(&self) -> u64 {
        self.finished.load(Ordering::SeqCst)
    }

    /// `work` unless a run finished since `ticket` was taken; then `joined`.
    pub(super) fn run<T>(
        &self,
        ticket: u64,
        work: impl FnOnce() -> T,
        joined: impl FnOnce() -> T,
    ) -> T {
        let _one = rb_util::lock(&self.lock);
        if self.finished.load(Ordering::SeqCst) != ticket {
            return joined();
        }
        let outcome = work();
        self.finished.fetch_add(1, Ordering::SeqCst);
        outcome
    }
}

static CHECKS: Serial = Serial::new();

/// `update.check`: the check on a blocking thread; its start and its outcome are notices.
pub(super) fn check_in_background(core: &Core) -> Value {
    let Some(core) = core.arc() else {
        return json!({ "checking": false });
    };
    if state::with(|state| state.phase.is_busy()) {
        return json!({ "checking": false, "busy": true });
    }
    // The answer can take seconds: the person sees the check begin (RB-090-85).
    show(&core, notices::checking());
    let runtime = core.runtime().clone();
    runtime.spawn_blocking(move || {
        check_now(&core, Origin::Person);
    });
    json!({ "checking": true })
}

/// One check, blocking, or the outcome of the one running; answers the status after it.
pub(super) fn check_now(core: &Arc<Core>, origin: Origin) -> Value {
    let ticket = CHECKS.ticket();
    CHECKS.run(
        ticket,
        || run(core, origin),
        || status(&core.user_settings()),
    )
}

fn run(core: &Arc<Core>, origin: Origin) -> Value {
    let settings = core.user_settings();
    // A download or a hand-over in progress keeps its offer; nothing to check meanwhile.
    if state::with(|state| matches!(state.phase, Phase::Downloading | Phase::Installing)) {
        return status(&settings);
    }
    let Some(data) = rb_config::local_dir() else {
        fail(core, "update.failed", origin == Origin::Person);
        return status(&settings);
    };
    state::with(|state| {
        state.phase = Phase::Checking;
        state.problem = None;
    });
    let channel = channel(&settings);
    let gate = Gate {
        feature_built: true,
        enabled: settings.updates.enabled,
    };
    let fetcher = HttpFetcher::new();
    let report = match rb_update::check_channel(
        gate,
        &fetcher,
        &Sources::official(),
        channel,
        &data,
        Utc::now(),
    ) {
        Ok(report) => report,
        Err(error) => {
            fail(core, error.code(), origin != Origin::Start);
            return status(&settings);
        }
    };
    let offer = rb_update::newest_offer(VERSION, channel, &report.manifests, &Target::current());
    let problem = report.problem;
    let Some(offer) = offer else {
        match problem {
            // Offline at the start is no news; a refused manifest always is.
            Some(problem) if !problem.is_expired() => fail(
                core,
                problem.code(),
                origin != Origin::Start || problem.is_integrity(),
            ),
            expired => {
                if let Some(expired) = expired {
                    tracing::info!(%expired, "the channel's manifest expired; nothing is offered");
                }
                state::with(|state| state.phase = Phase::UpToDate);
                if origin == Origin::Person {
                    show(
                        core,
                        notices::plain(
                            NotificationLevel::Success,
                            "update.notice.upToDate",
                            &[("version", VERSION.to_owned())],
                        ),
                    );
                }
            }
        }
        return status(&settings);
    };
    if let Some(problem) = &problem {
        tracing::warn!(
            code = problem.code(),
            "an update manifest was not taken beside the offer"
        );
    }
    tracing::info!(offered = %offer.version, "a newer rBrowser is published");
    let (kept, superseded) = state::with(|state| {
        let same = state
            .offer
            .as_ref()
            .is_some_and(|known| known.version == offer.version);
        let superseded = if same { None } else { state.ready.take() };
        state.phase = if same && state.ready.is_some() {
            Phase::Ready
        } else {
            Phase::Available
        };
        state.offer = Some(offer.clone());
        (same && state.ready.is_some(), superseded)
    });
    if let Some(old) = superseded {
        match std::fs::remove_file(&old) {
            Ok(()) => tracing::info!("the download of the superseded update is removed"),
            Err(error) => tracing::warn!(%error, "the superseded update download stays"),
        }
    }
    show(core, notices::offer(&offer, install_kind()));
    let facts = facts(&settings, Some(&offer.version));
    if !kept
        && facts.auto_install == AutoMode::OnQuit
        && facts.kind.installs_itself()
        && !facts.failed_before
    {
        start_download(core, &offer, false);
    }
    status(&settings)
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;

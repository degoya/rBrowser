//! The real [`Shot`] of a run (RB-096-07): each request is one delivery of the API client's one
//! send path (`crate::api_client::deliver`) - checked, the user's values and the environment's
//! variables put in, every hop held to the run's hosts ([`super::gate::Gate::allowed`]), sent by
//! the run's own [`LoadSender`] - on a blocking thread of the runtime, never the UI thread. A
//! request that never went out (a host not allowed, a variable without a value) is a `blocked`
//! failure; an answer whose checks fail is a `check` failure.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use rb_api_model::ApiRequest;
use rb_plugin_types::Failure;

use super::engine::Shot;
use super::scenario::{Scenario, personalize};
use super::sender::LoadSender;
use super::stats::{Failed, Hit};
use crate::api_client::deliver::{Courier, deliver};
use crate::api_client::ops::Context;
use crate::api_client::resolve::EnvReader;
use crate::api_client::store::Store;

/// One step's saved request.
pub struct Target {
    pub id: String,
    pub request: ApiRequest,
}

/// What a run sends with.
pub struct Live {
    /// The API client's context, its hosts the run's ([`super::gate::Gate::allowed`]).
    pub ctx: Context,
    pub store: Store,
    pub sender: LoadSender,
    pub steps: Vec<Target>,
    pub scenario: Scenario,
    /// The environment the requests run in.
    pub environment: Option<String>,
    pub env: &'static EnvReader,
}

/// The kind of a send's failure without an answer.
#[must_use]
pub fn failed_of(failure: &Failure) -> Failed {
    match failure.param("kind") {
        Some("timeout") => Failed::Timeout,
        Some("connection") => Failed::Connection,
        _ => Failed::Other,
    }
}

impl Live {
    /// Step `step` for `user` in round `iteration`, sent and judged. Blocks.
    #[must_use]
    pub fn fire_blocking(&self, step: usize, user: u32, iteration: u64) -> Hit {
        let started = Instant::now();
        let blocked = |ms: f64| Hit {
            step,
            ms,
            status: None,
            failed: Some(Failed::Blocked),
        };
        let Some(target) = self.steps.get(step) else {
            return blocked(0.0);
        };
        let values = self.scenario.values_of(user);
        let request = personalize(&target.request, &values, user, iteration);
        let courier = Courier {
            ctx: &self.ctx,
            store: &self.store,
            sender: &self.sender,
            env: self.env,
        };
        let sent = deliver(
            courier,
            &request,
            Some(target.id.as_str()),
            self.environment.clone(),
        );
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let Ok(sent) = sent else {
            return blocked(ms);
        };
        match &sent.result {
            Err(failure) => Hit {
                step,
                ms,
                status: None,
                failed: Some(failed_of(failure)),
            },
            Ok(exchange) => {
                let failed = Failed::of_status(exchange.status).or_else(|| {
                    sent.verdicts(&request)
                        .iter()
                        .any(|verdict| !verdict.passed)
                        .then_some(Failed::Check)
                });
                Hit {
                    step,
                    ms,
                    status: Some(exchange.status),
                    failed,
                }
            }
        }
    }
}

/// [`Live`] as the engine's shot: each fire on a blocking thread.
pub struct LiveShot(pub Arc<Live>);

#[async_trait]
impl Shot for LiveShot {
    async fn fire(&self, step: usize, user: u32, iteration: u64) -> Hit {
        let live = Arc::clone(&self.0);
        tokio::task::spawn_blocking(move || live.fire_blocking(step, user, iteration))
            .await
            .unwrap_or(Hit {
                step,
                ms: 0.0,
                status: None,
                failed: Some(Failed::Other),
            })
    }

    fn halt(&self) {
        self.0.sender.halt();
    }
}

#[cfg(test)]
#[path = "shot_tests.rs"]
mod tests;

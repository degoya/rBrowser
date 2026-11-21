//! The benchmark of one saved request (RB-096-04, owner 2026-11-21): sent `api-client.benchRepeats`
//! times in each of one or two environments - staging against production - with at most
//! `api-client.benchConcurrency` sends at once, and the phases (DNS, connect, TLS, first byte,
//! total) summed up as min, median, p95 and max ([`rb_api_model::stats`]). Deliberately no load
//! test (that is RB-096-07): few repeats, little concurrency, only granted hosts - every send is a
//! [`deliver`], held to the grants like any other. A request that cannot go out in an
//! environment (a host not granted, a variable missing) fails the benchmark before anything is
//! repeated, so the panel can ask for the host.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use rb_api_model::ApiRequest;
use rb_api_model::stats::Summary;
use rb_plugin_types::{Failure, FailureKind};
use serde::Serialize;

use super::deliver::{Courier, deliver};
use super::error;
use super::send::Exchange;

/// The most repeats any setting asks for.
pub const MAX_REPEATS: u32 = 100;
/// The default repeats.
pub const DEFAULT_REPEATS: u32 = 10;
/// The most sends at once any setting asks for.
pub const MAX_CONCURRENCY: u32 = 4;
/// The most environments compared.
pub const MAX_ENVIRONMENTS: usize = 2;

/// The phases of one series, summed up; a phase no send measured (TLS over `http`) is `None`.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseSummaries {
    pub dns: Option<Summary>,
    pub connect: Option<Summary>,
    pub tls: Option<Summary>,
    pub first_byte: Option<Summary>,
    pub total: Option<Summary>,
}

/// One environment's series.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Series {
    pub environment: Option<String>,
    pub sent: u32,
    /// Answers by status.
    pub statuses: BTreeMap<u16, u32>,
    /// Sends without an answer, by the failure's code.
    pub errors: BTreeMap<String, u32>,
    pub phases: PhaseSummaries,
}

/// The difference of the second series to the first, in milliseconds of the total.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub median_delta: f64,
    pub p95_delta: f64,
}

/// A benchmark's result.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bench {
    pub id: String,
    pub repeats: u32,
    pub concurrency: u32,
    pub series: Vec<Series>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comparison: Option<Comparison>,
}

/// The samples of one series.
#[derive(Default)]
struct Samples {
    dns: Vec<f64>,
    connect: Vec<f64>,
    tls: Vec<f64>,
    first_byte: Vec<f64>,
    total: Vec<f64>,
    statuses: BTreeMap<u16, u32>,
    errors: BTreeMap<String, u32>,
}

impl Samples {
    fn take(&mut self, result: Result<Exchange, Failure>) {
        match result {
            Ok(exchange) => {
                let phases = exchange.phases;
                self.dns.extend(phases.dns);
                self.connect.extend(phases.connect);
                self.tls.extend(phases.tls);
                self.first_byte.push(phases.first_byte);
                self.total.push(phases.total);
                *self.statuses.entry(exchange.status).or_default() += 1;
            }
            Err(failure) => {
                *self
                    .errors
                    .entry(super::failure_code(&failure))
                    .or_default() += 1;
            }
        }
    }

    fn series(self, environment: Option<String>, sent: u32) -> Series {
        Series {
            environment,
            sent,
            statuses: self.statuses,
            errors: self.errors,
            phases: PhaseSummaries {
                dns: Summary::of(&self.dns),
                connect: Summary::of(&self.connect),
                tls: Summary::of(&self.tls),
                first_byte: Summary::of(&self.first_byte),
                total: Summary::of(&self.total),
            },
        }
    }
}

fn series(
    courier: Courier<'_>,
    request: &ApiRequest,
    id: &str,
    environment: Option<String>,
    repeats: u32,
    concurrency: u32,
) -> Result<Series, Failure> {
    // The first send tells whether the request goes out at all.
    let first = deliver(courier, request, Some(id), environment.clone())?;
    let samples = Mutex::new(Samples::default());
    rb_util::lock(&samples).take(first.result);
    let next = AtomicU32::new(1);
    std::thread::scope(|scope| {
        for _ in 0..concurrency.clamp(1, MAX_CONCURRENCY) {
            scope.spawn(|| {
                while next.fetch_add(1, Ordering::Relaxed) < repeats {
                    let result = deliver(courier, request, Some(id), environment.clone())
                        .and_then(|sent| sent.result);
                    rb_util::lock(&samples).take(result);
                }
            });
        }
    });
    let samples = samples
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Ok(samples.series(environment, repeats))
}

/// Benchmarks the saved request `id` in `environments` (one or two; the views' environment for
/// none), `repeats` times each (held to 1..=[`MAX_REPEATS`]), `concurrency` at once (held to
/// 1..=[`MAX_CONCURRENCY`]).
///
/// # Errors
///
/// `too_many_environments`; the store's failure to read `id`; the first delivery's failure in
/// an environment (the request did not go out there).
pub fn run(
    courier: Courier<'_>,
    id: &str,
    environments: Vec<Option<String>>,
    repeats: u32,
    concurrency: u32,
) -> Result<Bench, Failure> {
    if environments.len() > MAX_ENVIRONMENTS {
        return Err(error(FailureKind::BadData, "too_many_environments")
            .with_param("most", MAX_ENVIRONMENTS));
    }
    let environments = if environments.is_empty() {
        vec![None]
    } else {
        environments
    };
    let (request, _) = courier.store.read(id)?;
    let repeats = repeats.clamp(1, MAX_REPEATS);
    let concurrency = concurrency.clamp(1, MAX_CONCURRENCY);
    let mut out = Vec::new();
    for environment in environments {
        out.push(series(
            courier,
            &request,
            id,
            environment,
            repeats,
            concurrency,
        )?);
    }
    let comparison = match out.as_slice() {
        [first, second] => match (&first.phases.total, &second.phases.total) {
            (Some(a), Some(b)) => Some(Comparison {
                median_delta: b.median - a.median,
                p95_delta: b.p95 - a.p95,
            }),
            _ => None,
        },
        _ => None,
    };
    Ok(Bench {
        id: id.to_owned(),
        repeats,
        concurrency,
        series: out,
        comparison,
    })
}

#[cfg(test)]
#[path = "bench_tests.rs"]
mod tests;

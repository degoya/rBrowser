//! One request delivered (RB-096-01, shared by RB-096-04): checked, its variables put in, its
//! host held to the grants, sent, every redirect followed only to a granted host. The send of
//! the panel, a collection run and the benchmark go through [`deliver`]; the load test of
//! RB-096-07 is meant to as well. It writes no history: that is the caller's.
//!
//! The outer `Err` is a request that never went out (invalid, a variable missing, a host not
//! granted, a body file that does not read); [`Sent::result`] holds the send's own outcome.

use rb_api_model::ApiRequest;
use rb_api_model::verdict::{self, Observed, Verdict};
use rb_plugin_types::{Failure, FailureKind};

use super::error;
use super::grants;
use super::ops::{Context, Sender, layers};
use super::resolve::{EnvReader, Variables, resolve};
use super::send::{self, Exchange, Prepared};
use super::store::Store;

/// A request that went out. No `Debug`: it holds the secret values.
pub struct Sent {
    /// The last hop sent.
    pub prepared: Prepared,
    /// Its answer, or why there was none.
    pub result: Result<Exchange, Failure>,
    /// The redirects followed, masked.
    pub redirects: Vec<String>,
    /// The secret values put in: what every text leaving the core is masked by.
    pub secrets: Vec<String>,
}

impl Sent {
    /// The verdicts of `request`'s checks on the answer, every found value masked; none
    /// without an answer.
    #[must_use]
    pub fn verdicts(&self, request: &ApiRequest) -> Vec<Verdict> {
        let Ok(exchange) = &self.result else {
            return Vec::new();
        };
        let observed = Observed {
            status: exchange.status,
            headers: &exchange.headers,
            body: &exchange.body,
            total_ms: exchange.phases.total,
        };
        verdict::judge(&request.checks, &observed)
            .into_iter()
            .map(|mut verdict| {
                verdict.actual = verdict
                    .actual
                    .map(|actual| rb_api_model::mask::mask(&actual, &self.secrets));
                verdict
            })
            .collect()
    }
}

/// What a delivery works with: the op's context, the collections, the sender, the process
/// variables.
#[derive(Clone, Copy)]
pub struct Courier<'a> {
    pub ctx: &'a Context,
    pub store: &'a Store,
    pub sender: &'a dyn Sender,
    pub env: &'a EnvReader,
}

/// Delivers `request` (saved as `id`, for its body files) in `environment`.
///
/// # Errors
///
/// `invalid_request`, `unresolved` with the names, `secrets_unreadable`, `host_not_allowed`
/// with the host (masked), and [`send::prepare`]'s: the request did not go out.
pub fn deliver(
    courier: Courier<'_>,
    request: &ApiRequest,
    id: Option<&str>,
    environment: Option<String>,
) -> Result<Sent, Failure> {
    let Courier {
        ctx,
        store,
        sender,
        env,
    } = courier;
    request.check().map_err(|invalid| {
        error(FailureKind::BadData, "invalid_request").with_param("reason", invalid.reason())
    })?;
    let variables = Variables {
        layers: layers(ctx, store, environment)?,
        env,
        now: ctx.now,
    };
    let (resolved, mut secrets) = resolve(request, &variables).map_err(|missing| {
        error(FailureKind::BadData, "unresolved").with_param("names", missing.names.join(", "))
    })?;
    let derived = send::derived_secrets(&resolved, &secrets);
    secrets.extend(derived);
    for secret in &secrets {
        rb_diag::redact::register_secret(secret);
    }
    let held = |url: &str| -> Result<(), Failure> {
        let host = grants::host_of(url).ok_or_else(|| {
            error(FailureKind::BadData, "invalid_request")
                .with_param("reason", "only http and https")
        })?;
        if grants::allowed(&ctx.allowed_hosts, &host) {
            Ok(())
        } else {
            Err(error(FailureKind::AccessDenied, "host_not_allowed")
                .with_param("host", rb_api_model::mask::mask(&host, &secrets)))
        }
    };
    held(&resolved.url)?;
    let limit = ctx.settings.max_response_bytes;
    let file = |path: &str| store.body_file(id, path, super::MAX_BODY_FILE_BYTES);
    let mut prepared = send::prepare(&resolved, &file)?;
    send::mark_secret_headers(&mut prepared, &secrets);
    let mut redirects = Vec::new();
    let result = loop {
        let exchange = match sender.send(&prepared, limit, &secrets) {
            Ok(exchange) => exchange,
            Err(failure) => break Err(failure),
        };
        let Some(next) = send::next_hop(&prepared, &exchange) else {
            break Ok(exchange);
        };
        if redirects.len() >= send::MAX_REDIRECTS as usize || held(&next.url).is_err() {
            // The redirect's answer is the send's: its Location names where it led.
            break Ok(exchange);
        }
        redirects.push(rb_api_model::mask::mask(&next.url, &secrets));
        prepared = next;
    };
    Ok(Sent {
        prepared,
        result,
        redirects,
        secrets,
    })
}

//! How a load test sends (RB-096-07): from the core on blocking threads, through one ureq agent
//! per run - its own connection pool, apart from the views, the API client and every other
//! client of the app - and never through a proxy: the run's connections go straight from this
//! machine to the target, so the target sees who loads it (no proxy chain, no rotating address).
//! Every request says what it is: its `User-Agent` is [`USER_AGENT`] whatever the saved request
//! names, and the headers that pass on another client's address (`X-Forwarded-For`,
//! `Forwarded`, `X-Real-IP` and kin) are not sent ([`identified`]). [`LoadSender::halt`] shuts
//! the agent down: every connection closes at once - a request waiting on a read returns
//! straight away - and nothing more goes out. On Windows a socket closed from another thread does
//! not wake a blocking read, so there each send runs on a thread of its own and the halt answers
//! the waiting caller at once; the abandoned thread ends with the request's own timeout.

use std::io::Read as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rb_plugin_types::{Failure, FailureKind};

use super::error;
use crate::api_client::ops::Sender;
use crate::api_client::send::{Exchange, Prepared};
use crate::api_client::timing::Phases;

/// The `User-Agent` of every request of a run.
pub const USER_AGENT: &str = concat!(
    "rBrowser/",
    env!("CARGO_PKG_VERSION"),
    " (rBrowser load test)"
);

/// Headers a run never sends: its own name is the agent's, and no other client's address is
/// passed on.
const NOT_SENT: [&str; 9] = [
    "user-agent",
    "forwarded",
    "x-forwarded-for",
    "x-forwarded-host",
    "x-real-ip",
    "x-client-ip",
    "true-client-ip",
    "cf-connecting-ip",
    "x-originating-ip",
];

/// `headers` as a run sends them: [`NOT_SENT`] left out, [`USER_AGENT`] added.
#[must_use]
pub fn identified(headers: &[(String, String)]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = headers
        .iter()
        .filter(|(name, _)| {
            !NOT_SENT
                .iter()
                .any(|hidden| name.eq_ignore_ascii_case(hidden))
        })
        .cloned()
        .collect();
    out.push(("User-Agent".to_owned(), USER_AGENT.to_owned()));
    out
}

/// The kind of a send that got no answer: `timeout`, `connection` or `other` (the parameter
/// `kind` of `send_failed`).
#[must_use]
pub fn kind_of(failure: &ureq::Error) -> &'static str {
    use std::io::ErrorKind;
    match failure {
        ureq::Error::Timeout(_) => "timeout",
        ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => "connection",
        ureq::Error::Io(io) => match io.kind() {
            ErrorKind::TimedOut => "timeout",
            ErrorKind::ConnectionRefused
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::NotConnected
            | ErrorKind::BrokenPipe
            | ErrorKind::UnexpectedEof => "connection",
            _ => "other",
        },
        _ => "other",
    }
}

/// The sender of one run.
pub struct LoadSender {
    /// The agent of requests that check the server's certificate.
    checked: ureq::Agent,
    /// The agent of those that do not (`# @rb-insecure`).
    unchecked: ureq::Agent,
    /// Set by [`LoadSender::halt`]: every send after it, and on Windows every send waiting for
    /// its answer, fails at once.
    halted: Arc<AtomicBool>,
}

fn agent(users: u32, verify_tls: bool) -> ureq::Agent {
    let idle = usize::try_from(users.max(1)).unwrap_or(usize::MAX);
    let tls = ureq::tls::TlsConfig::builder()
        .disable_verification(!verify_tls)
        .build();
    let config = ureq::Agent::config_builder()
        // Redirects are followed by the API client's send path, each host held to the gate.
        .max_redirects(0)
        .http_status_as_error(false)
        // Straight to the target: no proxy, not even the environment's.
        .proxy(None)
        .tls_config(tls)
        .user_agent(USER_AGENT)
        .max_idle_connections(idle.saturating_mul(2))
        .max_idle_connections_per_host(idle)
        .build();
    ureq::Agent::new_with_config(config)
}

impl LoadSender {
    /// A sender whose pool keeps a connection per user of `users`.
    #[must_use]
    pub fn new(users: u32) -> Self {
        Self {
            checked: agent(users, true),
            unchecked: agent(users, false),
            halted: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Ends every connection at once; every send after it fails.
    pub fn halt(&self) {
        self.halted.store(true, Ordering::SeqCst);
        self.checked.clone().shutdown();
        self.unchecked.clone().shutdown();
    }
}

fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.0
}

/// The failure of a send the halt ended.
fn halted_failure() -> Failure {
    error(FailureKind::Io, "send_failed")
        .with_param("kind", "connection")
        .with_param("reason", "the run was stopped")
}

impl Sender for LoadSender {
    fn send(
        &self,
        prepared: &Prepared,
        limit: u64,
        secrets: &[String],
    ) -> Result<Exchange, Failure> {
        if self.halted.load(Ordering::SeqCst) {
            return Err(halted_failure());
        }
        let agent = if prepared.options.verify_tls {
            &self.checked
        } else {
            &self.unchecked
        };
        if cfg!(windows) {
            let (agent, prepared, secrets) = (agent.clone(), prepared.clone(), secrets.to_vec());
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _gone = tx.send(exchange(&agent, &prepared, limit, &secrets));
            });
            loop {
                match rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(answer) => return answer,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if self.halted.load(Ordering::SeqCst) {
                            return Err(halted_failure());
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        return Err(halted_failure());
                    }
                }
            }
        }
        exchange(agent, prepared, limit, secrets)
    }
}

/// One request through `agent`, its answer read up to `limit` bytes.
fn exchange(
    agent: &ureq::Agent,
    prepared: &Prepared,
    limit: u64,
    secrets: &[String],
) -> Result<Exchange, Failure> {
    {
        let failed = |kind: &str, reason: String| {
            error(FailureKind::Io, "send_failed")
                .with_param("kind", kind)
                .with_param(
                    "reason",
                    rb_util::text::cut(&rb_api_model::mask::mask(&reason, secrets), 300),
                )
        };
        let mut builder = ureq::http::Request::builder()
            .method(prepared.method.as_str())
            .uri(prepared.url.as_str());
        for (name, value) in identified(&prepared.headers) {
            builder = builder.header(name.as_str(), value.as_str());
        }
        let request = builder
            .body(prepared.body.clone())
            .map_err(|e| failed("other", e.to_string()))?;
        let request = agent
            .configure_request(request)
            .timeout_global(Some(Duration::from_millis(prepared.options.timeout_ms)))
            .build();
        let started = Instant::now();
        let mut response = agent
            .run(request)
            .map_err(|e| failed(kind_of(&e), e.to_string()))?;
        let first_byte = ms(started);
        let status = response.status();
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                )
            })
            .collect();
        let mut body = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(limit.saturating_add(1))
            .read_to_end(&mut body)
            .map_err(|e| failed("connection", e.to_string()))?;
        let truncated = u64::try_from(body.len()).unwrap_or(u64::MAX) > limit;
        body.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        Ok(Exchange {
            status: status.as_u16(),
            status_text: status.canonical_reason().unwrap_or_default().to_owned(),
            headers,
            body,
            truncated,
            phases: Phases {
                dns: None,
                connect: None,
                tls: None,
                first_byte,
                total: ms(started),
            },
        })
    }
}

#[cfg(test)]
#[path = "sender_tests.rs"]
mod tests;

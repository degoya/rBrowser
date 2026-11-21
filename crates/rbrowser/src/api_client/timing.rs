//! The phases of one exchange (RB-096-01): a resolver and two connector links that note the
//! time when the name is resolved, the TCP connection stands and TLS is up, around ureq's own
//! connectors (an HTTP proxy from the environment, TCP, rustls). One agent per send, so no
//! pooled connection hides a phase; with redirects the last connection's phases are shown.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use ureq::config::Config;
use ureq::http::Uri;
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{
    ConnectProxyConnector, ConnectionDetails, Connector, NextTimeout, RustlsConnector,
    TcpConnector, Transport,
};

use crate::cdp::lock;

/// When each phase ended.
#[derive(Clone, Copy, Debug, Default)]
pub struct Times {
    pub started: Option<Instant>,
    pub resolve_started: Option<Instant>,
    pub resolved: Option<Instant>,
    pub connected: Option<Instant>,
    pub tls: Option<Instant>,
}

/// The phases in milliseconds; a phase that did not happen (a pooled connection, no TLS) is
/// `None`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Phases {
    pub dns: Option<f64>,
    pub connect: Option<f64>,
    pub tls: Option<f64>,
    pub first_byte: f64,
    pub total: f64,
}

fn ms(from: Instant, to: Instant) -> f64 {
    to.saturating_duration_since(from).as_secs_f64() * 1000.0
}

impl Times {
    /// The phases, the headers in at `headers`, the body read at `done`.
    #[must_use]
    pub fn phases(&self, headers: Instant, done: Instant) -> Phases {
        let started = self.started.unwrap_or(headers);
        let span = |from: Option<Instant>, to: Option<Instant>| Some(ms(from?, to?));
        let ready = self.tls.or(self.connected).unwrap_or(started);
        Phases {
            dns: span(self.resolve_started, self.resolved),
            connect: span(self.resolved, self.connected),
            tls: span(self.connected, self.tls),
            first_byte: ms(ready.max(started), headers),
            total: ms(started, done),
        }
    }
}

/// The times of one send, shared by its resolver and connectors.
#[derive(Debug, Default)]
pub struct Marks {
    times: Mutex<Times>,
}

impl Marks {
    /// The times so far.
    #[must_use]
    pub fn times(&self) -> Times {
        *lock(&self.times)
    }

    /// The send starts now.
    pub fn start(&self) {
        lock(&self.times).started = Some(Instant::now());
    }

    fn note(&self, step: Step) {
        let now = Some(Instant::now());
        let mut times = lock(&self.times);
        match step {
            Step::ResolveStarted => times.resolve_started = now,
            Step::Resolved => times.resolved = now,
            Step::Connected => times.connected = now,
            Step::Tls => times.tls = now,
        }
    }
}

/// A phase's end.
#[derive(Clone, Copy, Debug)]
enum Step {
    ResolveStarted,
    Resolved,
    Connected,
    Tls,
}

/// The system resolver, timed.
#[derive(Debug)]
pub struct TimedResolver {
    inner: DefaultResolver,
    marks: Arc<Marks>,
}

impl TimedResolver {
    #[must_use]
    pub fn new(marks: Arc<Marks>) -> Self {
        Self {
            inner: DefaultResolver::default(),
            marks,
        }
    }
}

impl Resolver for TimedResolver {
    fn resolve(
        &self,
        uri: &Uri,
        config: &Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        self.marks.note(Step::ResolveStarted);
        let resolved = self.inner.resolve(uri, config, timeout);
        self.marks.note(Step::Resolved);
        resolved
    }
}

/// A link of the connector chain that notes when the transport before it stands.
#[derive(Debug)]
pub struct Mark {
    marks: Arc<Marks>,
    step: Step,
}

impl<In: Transport> Connector<In> for Mark {
    type Out = In;

    fn connect(
        &self,
        details: &ConnectionDetails,
        chained: Option<In>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        let tls_wanted = matches!(self.step, Step::Tls);
        if chained.is_some() && (!tls_wanted || details.needs_tls()) {
            self.marks.note(self.step);
        }
        Ok(chained)
    }
}

/// The connector chain of a send: CONNECT proxy, TCP, noted, rustls, noted.
pub fn connector(marks: &Arc<Marks>) -> impl Connector {
    ().chain(ConnectProxyConnector::default())
        .chain(TcpConnector::default())
        .chain(Mark {
            marks: Arc::clone(marks),
            step: Step::Connected,
        })
        .chain(RustlsConnector::default())
        .chain(Mark {
            marks: Arc::clone(marks),
            step: Step::Tls,
        })
}

#[cfg(test)]
#[path = "timing_tests.rs"]
mod tests;

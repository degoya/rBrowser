//! Test doubles of the API client's runs (RB-096-04): a sender that answers by URL and counts
//! what it got, an exchange, and a context over a temporary project.

use std::sync::Mutex;

use rb_plugin_types::{Failure, FailureKind};
use serde_json::json;

use super::ops::{Context, Sender, Settings};
use super::send::{Exchange, Prepared};
use super::timing::Phases;

/// Answers a send with the first answer whose URL prefix the request's URL starts with, else
/// with `send_failed`; notes every request.
pub struct UrlSender {
    pub answers: Vec<(String, Exchange)>,
    pub sent: Mutex<Vec<Prepared>>,
}

impl UrlSender {
    pub fn new(answers: Vec<(&str, Exchange)>) -> Self {
        Self {
            answers: answers
                .into_iter()
                .map(|(prefix, exchange)| (prefix.to_owned(), exchange))
                .collect(),
            sent: Mutex::new(Vec::new()),
        }
    }

    pub fn urls(&self) -> Vec<String> {
        self.sent
            .lock()
            .expect("sent")
            .iter()
            .map(|prepared| prepared.url.clone())
            .collect()
    }
}

impl Sender for UrlSender {
    fn send(
        &self,
        prepared: &Prepared,
        _limit: u64,
        _secrets: &[String],
    ) -> Result<Exchange, Failure> {
        self.sent.lock().expect("sent").push(prepared.clone());
        self.answers
            .iter()
            .find(|(prefix, _)| prepared.url.starts_with(prefix.as_str()))
            .map(|(_, exchange)| exchange.clone())
            .ok_or_else(|| super::error(FailureKind::Io, "send_failed"))
    }
}

/// An answer of `status` with `headers` and `body`, `total` ms long.
pub fn answer(status: u16, headers: &[(&str, &str)], body: &str, total: f64) -> Exchange {
    Exchange {
        status,
        status_text: String::new(),
        headers: headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        body: body.as_bytes().to_vec(),
        truncated: false,
        phases: Phases {
            dns: Some(1.0),
            connect: Some(2.0),
            tls: None,
            first_byte: total / 2.0,
            total,
        },
    }
}

/// No process variables.
pub fn no_env(_name: &str) -> Option<String> {
    None
}

/// A trusted project at `root` with the environments `staging` and `production`, both hosts
/// granted, a private `token` in staging.
pub fn context(root: &std::path::Path) -> Context {
    Context {
        root: Some(root.to_path_buf()),
        trusted: true,
        environment: Some("staging".into()),
        environments: vec![
            ("staging".into(), "https://staging.example.com".into()),
            ("production".into(), "https://api.example.com".into()),
        ],
        allowed_hosts: vec!["staging.example.com".into(), "api.example.com".into()],
        secrets: json!({ "staging": { "token": "tok-secret-1" } }),
        settings: Settings {
            history_size: 10,
            max_response_bytes: 4096,
            bench_repeats: 5,
            bench_concurrency: 1,
            code: crate::api_client::export::CodeSettings::default(),
        },
        now: 1_700_000_000,
    }
}
